mod aireq;
mod deesser;
mod distortion;
mod denoiser;
mod dynamic;
mod filters;
mod fixeq;
mod output;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use clap::Parser;
use hound::{SampleFormat, WavSpec, WavWriter};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use denoiser::common::{get_preset, DEFAULT_PRESET, PRESETS};
use denoiser::denoiser::{
    analyze_audio, process_stereo_lr, SpectralSubtractionDenoiser, SAMPLE_RATE,
};

use aireq::StereoAirEq;
use deesser::StereoDeEsser;
use distortion::{StereoChannel9, StereoTapeGlue, StereoTapeHysteresis};
use dynamic::{analyze_gain, apply_gain, linear_to_db, StereoButterComp2, StereoLimiter, DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB};
use filters::{HighPassSlope, StereoFilterChain};
use fixeq::FixEq;
use output::{measure_integrated_lufs, DEFAULT_TARGET_LUFS};

#[derive(Parser)]
#[command(name = "poddyclip")]
#[command(about = "Spectral Subtraction Denoiser", long_about = None)]
#[command(after_help = r#"Preset Modes:
  1 = Gentle     - Minimal processing, preserves everything
  2 = Light      - Subtle noise reduction
  3 = Moderate   - Balanced (default)
  4 = Strong     - Noticeable noise reduction
  5 = Aggressive - Maximum removal, may affect speech quality"#)]
struct Args {
    /// Input audio file (WAV, MP3, etc.)
    input: PathBuf,

    /// Output WAV file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Denoising strength 1-5
    #[arg(short, long, default_value_t = DEFAULT_PRESET, value_parser = clap::value_parser!(u8).range(1..=5))]
    preset: u8,

    /// High-pass filter slope: 12 or 24 dB/octave (filters applied before denoising)
    #[arg(long, default_value_t = 24, value_parser = clap::value_parser!(u8).range(12..=24))]
    hp_slope: u8,

    /// Disable filters (skip HP @ 80Hz and LP @ 15.5kHz)
    #[arg(long)]
    no_filters: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("Loading: {}", args.input.display());
    let (samples, input_sr) = load_audio(&args.input)?;

    let channels = samples.len();
    let num_samples = samples.first().map(|c| c.len()).unwrap_or(0);
    let is_stereo = channels == 2;

    println!("Sample rate: {} Hz", input_sr);
    println!("Channels: {}", if is_stereo { "stereo" } else { "mono" });
    println!("Duration: {:.2}s", num_samples as f32 / input_sr as f32);

    if input_sr != SAMPLE_RATE {
        eprintln!(
            "WARNING: Sample rate {} != {}. Results may vary.",
            input_sr, SAMPLE_RATE
        );
    }

    // =========================================================================
    // Cleanup Filters - Apply HP/LP before gain analysis
    // =========================================================================

    let mut samples = samples;

    if !args.no_filters {
        let hp_slope = if args.hp_slope == 24 {
            HighPassSlope::Slope24dB
        } else {
            HighPassSlope::Slope12dB
        };

        println!(
            "\n[Cleanup Filters] HP: 80Hz @ {} dB/oct, LP: 15.5kHz @ 12 dB/oct",
            args.hp_slope
        );

        let mut stereo_filters = StereoFilterChain::new(input_sr as f32, hp_slope);

        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            stereo_filters.process_stereo(&mut left[0], &mut right[0]);
        } else {
            stereo_filters.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // Input Gain - Normalize to target RMS (after filtering)
    // =========================================================================

    println!("\n[Input Gain]");
    let (input_rms, input_peak) = if is_stereo {
        dynamic::autogain::calculate_rms_and_peak_stereo(&samples[0], &samples[1])
    } else {
        dynamic::autogain::calculate_rms_and_peak(&samples[0])
    };
    let input_rms_db = linear_to_db(input_rms);
    let input_peak_db = linear_to_db(input_peak);
    let gain_db = analyze_gain(&samples, DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB);

    // Check if gain was limited by peak
    let gain_for_rms = DEFAULT_TARGET_RMS_DB - input_rms_db;
    let peak_limited = gain_db < gain_for_rms && gain_for_rms > 0.0;

    println!("  Input RMS: {:.1} dBFS, Peak: {:.1} dBFS", input_rms_db, input_peak_db);
    println!("  Target RMS: {:.1} dBFS, Peak ceiling: {:.1} dBFS", DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB);
    if peak_limited {
        println!("  Applying: {:+.1} dB gain (limited by peak, would need {:+.1} dB for target RMS)", gain_db, gain_for_rms);
    } else {
        println!("  Applying: {:+.1} dB gain", gain_db);
    }

    apply_gain(&mut samples, gain_db);

    // =========================================================================
    // Pass 1: Analysis (on filtered + gain-normalized audio)
    // =========================================================================

    println!("\n[Pass 1] Analyzing audio...");

    // Perform single-pass analysis (computes noise floor + all metrics)
    let result = analyze_audio(&samples[0], input_sr);
    let noise_floor = result.noise_floor;
    let analysis = result.analysis;

    // Display analysis results
    println!("  ✓ Analysis complete");
    println!();
    println!("  Audio characteristics:");
    println!("    SNR: {:.1} dB", analysis.overall_snr_db);
    println!("    Stationarity: {:.2}", analysis.stationarity_score);
    println!(
        "    Speech density: {:.0}%",
        analysis.speech_density * 100.0
    );
    println!(
        "    Dominant noise freq: {:.0} Hz",
        analysis.dominant_freq_hz
    );
    println!();

    // =========================================================================
    // Pass 2: Processing
    // =========================================================================

    let preset = args.preset.into();

    let preset_info = get_preset(preset).expect("Invalid preset");
    let preset_name = preset_info.name.to_lowercase();

    let output_path = if let Some(ref out) = args.output {
        out.clone()
    } else {
        let stem = args
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        PathBuf::from(format!("{stem}_denoised_{preset}_{preset_name}.wav"))
    };

    println!(
        "\n[Pass 2] Processing with preset {} ({})...",
        preset,
        PRESETS[preset - 1].name
    );

    // Audio is already filtered and gain-normalized from earlier stages
    let denoised_samples = if is_stereo {
        let left = &samples[0];
        let right = &samples[1];

        // L/R independent processing
        let (left_out, right_out) =
            process_stereo_lr(left, right, input_sr, preset, Some(&noise_floor));

        vec![left_out, right_out]
    } else {
        let mut denoiser = SpectralSubtractionDenoiser::new(input_sr, preset);
        denoiser.init_with_noise_floor(&noise_floor);
        let output = denoiser.process(&samples[0]);

        vec![output]
    };

    // Apply FixEq (post-denoiser dynamic EQ)
    let mut fixeq = FixEq::new(input_sr as f32);
    let analysis = fixeq.configure(&denoised_samples, preset).clone();

    println!(
        "  Mud analysis: {:.0}Hz (energy: {:.1}dB, confidence: {:.0}%)",
        analysis.mud.center_freq,
        analysis.mud.energy_db,
        analysis.mud.confidence * 100.0
    );
    println!(
        "  Applying de-mud @ {:.0}Hz (strength: {:.0}%)...",
        analysis.mud.center_freq,
        fixeq.get_demud_strength() * 100.0
    );
    println!(
        "  Correction A: {:.0}Hz (energy: {:.1}dB, confidence: {:.0}%, strength: {:.0}%)",
        analysis.correction_a.center_freq,
        analysis.correction_a.energy_db,
        analysis.correction_a.confidence * 100.0,
        fixeq.get_correction_a_strength() * 100.0
    );
    println!(
        "  Correction B: {:.0}Hz (energy: {:.1}dB, confidence: {:.0}%, strength: {:.0}%)",
        analysis.correction_b.center_freq,
        analysis.correction_b.energy_db,
        analysis.correction_b.confidence * 100.0,
        fixeq.get_correction_b_strength() * 100.0
    );

    let mut output_samples = if is_stereo {
        let (mut left, mut right) = {
            let mut iter = denoised_samples.into_iter();
            (iter.next().unwrap(), iter.next().unwrap())
        };
        fixeq.process_stereo(&mut left, &mut right);
        vec![left, right]
    } else {
        let mut mono = denoised_samples.into_iter().next().unwrap();
        fixeq.process_mono(&mut mono);
        vec![mono]
    };

    // Apply De-Esser (after FixEq)
    let mut deesser = StereoDeEsser::new(input_sr as f32);
    let sibilance = deesser.configure(&output_samples).clone();
    let q = deesser::analysis::calculate_deesser_q(sibilance.bandwidth_hz, sibilance.center_freq);
    let deesser_strength = deesser.get_strength();

    println!(
        "  De-esser: {:.0}Hz (bandwidth: {:.0}Hz, Q: {:.1}, energy: {:.1}dB, confidence: {:.0}%, strength: {:.0}%)",
        sibilance.center_freq,
        sibilance.bandwidth_hz,
        q,
        sibilance.energy_db,
        sibilance.confidence * 100.0,
        deesser_strength * 100.0
    );

    if is_stereo {
        let (left, right) = output_samples.split_at_mut(1);
        deesser.process_stereo(&mut left[0], &mut right[0]);
    } else {
        deesser.process_mono(&mut output_samples[0]);
    }

    // Apply Channel9 (Neve transformer emulation)
    let drive = 0.2; // Fixed 40% - subtle warmth without emphasizing problems
    println!(
        "  Applying Neve transformer (drive: {:.0}%)...",
        drive * 200.0
    );
    let mut channel9 = StereoChannel9::new(input_sr as f32);
    channel9.set_drive(drive);
    if is_stereo {
        let (left, right) = output_samples.split_at_mut(1);
        channel9.process_stereo(&mut left[0], &mut right[0]);
    } else {
        channel9.process_mono(&mut output_samples[0]);
    }

    // Apply ButterComp2 (smooth leveling)
    let compress = 0.8;
    println!(
        "  Applying ButterComp2 (compress: {:.0}%)...",
        compress * 100.0
    );
    let mut compressor = StereoButterComp2::new(input_sr as f32);
    compressor.set_compress(compress);
    if is_stereo {
        let (left, right) = output_samples.split_at_mut(1);
        compressor.process_stereo(&mut left[0], &mut right[0]);
    } else {
        compressor.process_mono(&mut output_samples[0]);
    }

    // Apply TapeGlue (subtle tape saturation)
    // let warmth = 0.22; // Subtle
    // println!(
    //     "  Applying TapeGlue (warmth: {:.0}%)...",
    //     warmth * 100.0
    // );
    // let mut tape_glue = StereoTapeGlue::new(input_sr as f64);
    // tape_glue.set_warmth(warmth);
    // if is_stereo {
    //     let (left, right) = output_samples.split_at_mut(1);
    //     tape_glue.process_stereo(&mut left[0], &mut right[0]);
    // } else {
    //     tape_glue.left.process_mono(&mut output_samples[0]);
    // }

    // Apply TapeHysteresis (full Jiles-Atherton physics model)
    // let drive = 0.3; // Subtle - full model is more intense
    // println!(
    //     "  Applying TapeHysteresis (drive: {:.0}%)...",
    //     drive * 100.0
    // );
    // let mut tape_hyst = StereoTapeHysteresis::new(input_sr as f64);
    // tape_hyst.set_drive(drive);
    // if is_stereo {
    //     let (left, right) = output_samples.split_at_mut(1);
    //     tape_hyst.process_stereo(&mut left[0], &mut right[0]);
    // } else {
    //     tape_hyst.left.process_mono(&mut output_samples[0]);
    // }

    // Apply Air EQ (high shelf + LP rolloff)
    println!("  Applying Air EQ (shelf: 10kHz +2dB, LP: 16kHz)...");
    let mut aireq = StereoAirEq::new(input_sr as f32);
    if is_stereo {
        let (left, right) = output_samples.split_at_mut(1);
        aireq.process_stereo(&mut left[0], &mut right[0]);
    } else {
        aireq.process_mono(&mut output_samples[0]);
    }

    // =========================================================================
    // Output Normalization - LUFS + Limiting
    // =========================================================================

    println!("\n[Output Normalization]");

    // Measure integrated LUFS
    let lufs = measure_integrated_lufs(&output_samples, input_sr);
    println!("  Integrated LUFS: {:.1}", lufs);

    // Calculate gain to reach target LUFS
    let lufs_gain_db = DEFAULT_TARGET_LUFS - lufs;
    println!(
        "  Target: {:.1} LUFS, applying {:+.1} dB",
        DEFAULT_TARGET_LUFS, lufs_gain_db
    );

    // Apply gain
    apply_gain(&mut output_samples, lufs_gain_db);

    // Apply true peak limiter at -1 dBTP
    let mut limiter = StereoLimiter::new(-1.0, 5.0, 100.0, input_sr as f32);
    let stats = if is_stereo {
        let (left, right) = output_samples.split_at_mut(1);
        limiter.process_stereo(&mut left[0], &mut right[0])
    } else {
        limiter.process_mono(&mut output_samples[0])
    };
    println!(
        "  Limiter: ceiling -1.0 dBTP, max GR: {:.1} dB, peak out: {:.1} dBFS",
        stats.max_reduction_db, stats.peak_output_db
    );

    println!("Saving: {}", output_path.display());
    save_wav(&output_path, &output_samples, input_sr)?;

    println!("\nDone.");
    Ok(())
}

fn load_audio(input_path: &Path) -> Result<(Vec<Vec<f32>>, u32)> {
    let file = std::fs::File::open(input_path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = input_path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    )?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| anyhow::anyhow!("No audio track found"))?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())?;

    let mut all_samples: Vec<Vec<f32>> = vec![Vec::new(); channels];

    loop {
        match format.next_packet() {
            Ok(packet) => {
                let decoded = decoder.decode(&packet)?;
                let spec = *decoded.spec();
                let duration = decoded.capacity() as u64;

                let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
                sample_buf.copy_interleaved_ref(decoded);

                let samples = sample_buf.samples();

                for (i, sample) in samples.iter().enumerate() {
                    all_samples[i % channels].push(*sample);
                }
            }
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(e.into()),
        }
    }

    if all_samples.is_empty() || all_samples[0].is_empty() {
        bail!("No audio samples found in file");
    }

    Ok((all_samples, sample_rate))
}

fn save_wav(path: &Path, samples: &[Vec<f32>], sample_rate: u32) -> Result<()> {
    let channels = samples.len() as u16;
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut writer = WavWriter::create(path, spec)?;

    // Get minimum length across all channels
    let num_samples = samples.iter().map(|c| c.len()).min().unwrap_or(0);

    // Write interleaved samples
    for i in 0..num_samples {
        for channel in samples {
            // Clip and convert to i16
            let sample = channel[i].clamp(-1.0, 1.0);
            let sample_i16 = (sample * 32767.0) as i16;
            writer.write_sample(sample_i16)?;
        }
    }

    writer.finalize()?;
    Ok(())
}
