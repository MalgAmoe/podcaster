mod denoiser;

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

use denoiser::{
    analyze_audio_simple, compute_minimum_statistics, get_preset, match_rms, process_stereo,
    process_stereo_lr, recommend_preset, recommend_thresholds, SpectralSubtractionDenoiser,
    DEFAULT_PRESET, PRESETS, SAMPLE_RATE,
};

#[derive(Parser)]
#[command(name = "poddyclip")]
#[command(about = "Spectral Subtraction Denoiser", long_about = None)]
#[command(after_help = r#"Preset Modes:
  1 = Gentle     - Minimal processing, preserves everything
  2 = Light      - Subtle noise reduction
  3 = Moderate   - Balanced (default)
  4 = Strong     - Noticeable noise reduction
  5 = Aggressive - Maximum removal, may affect speech quality

Stereo Modes:
  ms = Mid/Side processing (default) - Better for centered content
  lr = Left/Right independent - Better for wide stereo imaging"#)]
struct Args {
    /// Input audio file (WAV, MP3, etc.)
    input: PathBuf,

    /// Output WAV file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Denoising strength 1-5
    #[arg(short, long, default_value_t = DEFAULT_PRESET, value_parser = clap::value_parser!(u8).range(1..=5))]
    preset: u8,

    /// Disable RMS level matching
    #[arg(long)]
    no_level_match: bool,

    /// Generate output for all 5 presets
    #[arg(long)]
    all_presets: bool,

    /// Stereo processing mode: 'ms' (Mid/Side) or 'lr' (Left/Right independent)
    #[arg(long, default_value = "lr", value_parser = ["ms", "lr"])]
    stereo_mode: String,
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
    // Pass 1: Analysis
    // =========================================================================

    println!("\n[Pass 1] Analyzing audio...");

    // Compute noise floor using minimum statistics
    let noise_floor = if is_stereo {
        // Use left channel for analysis
        compute_minimum_statistics(&samples[0], input_sr)
    } else {
        compute_minimum_statistics(&samples[0], input_sr)
    };

    // Analyze and get recommendations
    let analysis = analyze_audio_simple(&samples[0], input_sr, &noise_floor);

    // Display analysis results
    println!("  ✓ Analysis complete");
    println!();
    println!("  Audio characteristics:");
    println!("    SNR: {:.1} dB", analysis.overall_snr_db);
    println!("    Stationarity: {:.2}", analysis.stationarity_score);
    println!("    Speech density: {:.0}%", analysis.speech_density * 100.0);
    println!("    Dominant noise freq: {:.0} Hz", analysis.dominant_freq_hz);
    println!();

    // Propose recommendations (NOT applied automatically)
    let recommended_preset = recommend_preset(&analysis);
    let (sfm_speech, sfm_noise, spike_thresh) = recommend_thresholds(&analysis);

    println!("  Recommendations:");
    println!(
        "    Suggested preset: {} ({})",
        recommended_preset,
        PRESETS[recommended_preset - 1].name
    );
    println!(
        "    Suggested SFM thresholds: speech={:.2}, noise={:.2}",
        sfm_speech, sfm_noise
    );
    println!("    Suggested spike threshold: {:.1}", spike_thresh);

    // If user chose default preset, show they could try the recommendation
    if args.preset == DEFAULT_PRESET && recommended_preset != DEFAULT_PRESET as usize {
        println!();
        println!("  Tip: Try --preset {} for this audio", recommended_preset);
    }

    println!();

    // =========================================================================
    // Pass 2: Processing
    // =========================================================================

    let presets_to_run: Vec<usize> = if args.all_presets {
        vec![1, 2, 3, 4, 5]
    } else {
        vec![args.preset as usize]
    };

    for preset in presets_to_run {
        let preset_info = get_preset(preset).expect("Invalid preset");
        let preset_name = preset_info.name.to_lowercase();

        let output_path = if let Some(ref out) = args.output {
            if args.all_presets {
                let stem = out
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                PathBuf::from(format!("{}_{preset}_{preset_name}.wav", stem))
            } else {
                out.clone()
            }
        } else {
            let stem = args
                .input
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("audio");
            PathBuf::from(format!("{stem}_denoised_{preset}_{preset_name}.wav"))
        };

        println!(
            "\nProcessing with preset {} ({})...",
            preset, PRESETS[preset - 1].name
        );

        let output_samples = if is_stereo {
            let left = &samples[0];
            let right = &samples[1];

            // Choose stereo processing mode based on user preference
            let (mut left_out, mut right_out) = if args.stereo_mode == "lr" {
                // L/R independent processing
                process_stereo_lr(left, right, input_sr, preset, Some(&noise_floor))
            } else {
                // M/S (Mid/Side) processing (default)
                process_stereo(left, right, input_sr, preset, Some(&noise_floor))
            };

            if !args.no_level_match {
                let left_len = left_out.len().min(left.len());
                let right_len = right_out.len().min(right.len());
                match_rms(&left[..left_len], &mut left_out[..left_len]);
                match_rms(&right[..right_len], &mut right_out[..right_len]);
            }

            vec![left_out, right_out]
        } else {
            let mut denoiser = SpectralSubtractionDenoiser::new(input_sr, preset);
            denoiser.init_with_noise_floor(&noise_floor);
            let mut output = denoiser.process(&samples[0]);

            if !args.no_level_match {
                let len = output.len().min(samples[0].len());
                match_rms(&samples[0][..len], &mut output[..len]);
            }

            vec![output]
        };

        println!("Saving: {}", output_path.display());
        save_wav(&output_path, &output_samples, input_sr)?;
    }

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
