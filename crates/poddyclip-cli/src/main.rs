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

use poddyclip::analysis;
use poddyclip::analysis::lufs::{measure_integrated_lufs, DEFAULT_TARGET_LUFS};
use poddyclip::denoiser::{
    analyze_audio, detect_tonal_peaks, get_gate_preset_name, get_preset, PeakAttenuator,
    PeakAttenuatorParams, RealtimeDenoiser, SpectralGate, DEFAULT_PRESET, PRESETS,
};
use poddyclip::dynamics::autogain::{
    analyze_gain, apply_gain, linear_to_db, DEFAULT_TARGET_PEAK_DB, DEFAULT_TARGET_RMS_DB,
};
use poddyclip::dynamics::limiter::Limiter;
use poddyclip::dynamics::{ButterComp2, StereoFetCompressor, StereoVcaPeakComp};
use poddyclip::eq::deesser::StereoDeEsser;
use poddyclip::eq::{FilterChain, FixEq, HighPassSlope, RadioVoiceProcessor, StereoEnhanceEq};
use poddyclip::repair::Declicker;
use poddyclip::saturation::Channel9;
use poddyclip::traits::{Stereo, StereoProcessor};

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

    /// Remove clicks and pops (offline processing, runs before other stages)
    #[arg(long)]
    declick: bool,

    /// FET-style compression (1176-inspired, fast attack, program-dependent release)
    #[arg(long)]
    fet: bool,

    /// Radio Voice EQ - automatic broadcast-style EQ
    #[arg(long)]
    radio: bool,

    /// Radio Voice EQ amount (0.0-1.0, default 1.0)
    #[arg(long, default_value_t = 1.0)]
    radio_amount: f32,

    /// De-reverb strength 1-5 (0 = disabled)
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=5))]
    dereverb: u8,

    /// Spectral gate strength 1-5 (0 = off) - handles intermittent noise
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=5))]
    spectral_gate: u8,

    /// Enable tonal peak attenuation (hum, whine removal)
    #[arg(long)]
    depeak: bool,

    /// Maximum peak attenuation in dB (with --depeak)
    #[arg(long, default_value_t = 18.0)]
    depeak_max_db: f32,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // =========================================================================
    // LOAD
    // =========================================================================
    println!("Loading: {}", args.input.display());
    let (mut samples, sample_rate) = load_audio(&args.input)?;
    let is_stereo = samples.len() >= 2;

    println!("Sample rate: {} Hz", sample_rate);
    println!("Channels: {}", if is_stereo { "stereo" } else { "mono" });
    println!(
        "Duration: {:.2}s",
        samples[0].len() as f32 / sample_rate as f32
    );

    // =========================================================================
    // INPUT STAGE
    // =========================================================================

    // Cleanup filters
    if !args.no_filters {
        let slope = if args.hp_slope == 24 {
            HighPassSlope::Slope24dB
        } else {
            HighPassSlope::Slope12dB
        };
        println!("\n[Filters] HP 80Hz @ {}dB/oct, LP 15.5kHz", args.hp_slope);

        let mut filters = Stereo::from_pair(
            FilterChain::new(sample_rate as f32, slope),
            FilterChain::new(sample_rate as f32, slope),
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            filters.process_stereo(&mut left[0], &mut right[0]);
        } else {
            filters.process_mono(&mut samples[0]);
        }
    }

    // Input gain
    println!("\n[Input Gain]");
    let (rms, peak) = if is_stereo {
        poddyclip::dynamics::autogain::calculate_rms_and_peak_stereo(&samples[0], &samples[1])
    } else {
        poddyclip::dynamics::autogain::calculate_rms_and_peak(&samples[0])
    };
    let gain_db = analyze_gain(&samples, DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB);
    println!(
        "  Input: RMS {:.1}dB, Peak {:.1}dB",
        linear_to_db(rms),
        linear_to_db(peak)
    );
    println!("  Applying: {:+.1}dB", gain_db);
    apply_gain(&mut samples, gain_db);

    // =========================================================================
    // REPAIR (OFFLINE)
    // =========================================================================
    if args.declick {
        println!("\n[Declick]");
        let declicker = Declicker::new(sample_rate);
        println!(
            "  Tuned for {}Hz: frame={}samples, order={}, threshold={}",
            sample_rate,
            declicker.detector.frame_size,
            declicker.detector.order,
            declicker.detector.threshold_k
        );
        for (i, channel) in samples.iter_mut().enumerate() {
            let clicks_found = declicker.detector.detect_f32(channel).len();
            let repaired = declicker.process_f32(channel);
            println!("  Channel {}: {} clicks repaired", i, clicks_found);
            *channel = repaired;
        }
    }

    // =========================================================================
    // DE-REVERB (optional, runs BEFORE denoiser)
    // =========================================================================
    if args.dereverb > 0 {
        println!("\n[DeReverb]");

        // Analyze reverb characteristics on original audio
        let mono_for_reverb = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let spectrum = analysis::SpectralAnalysis::new(&mono_for_reverb, sample_rate);
        let cepstral = analysis::CepstralAnalysis::from_spectrum(&spectrum);
        let reverb_analysis = analysis::ReverbAnalysis::from_analyses(&spectrum, &cepstral);

        println!(
            "  RT60: {:.0}ms, DRR: {:.1}dB ({})",
            reverb_analysis.rt60_avg_ms,
            reverb_analysis.drr_db,
            reverb_analysis.severity()
        );

        // Process with dereverb
        let dereverb_preset = args.dereverb;
        println!(
            "  Preset: {} ({})",
            dereverb_preset,
            poddyclip::dereverb::get_preset_name(dereverb_preset)
        );

        let mut dereverb = poddyclip::dereverb::DeReverbProcessor::new_with_preset(
            sample_rate,
            dereverb_preset,
        )
        .expect("Invalid dereverb preset");
        dereverb.init_with_analysis(&reverb_analysis);

        if is_stereo {
            let mut left_dereverb = dereverb.clone();
            samples[0] = dereverb.process(&samples[0]);
            samples[1] = left_dereverb.process(&samples[1]);
        } else {
            samples[0] = dereverb.process(&samples[0]);
        }
        println!("    Max GR: {:.1}dB", dereverb.get_max_gain_reduction_db());
    }

    // =========================================================================
    // DENOISE
    // =========================================================================
    println!("\n[Denoise]");
    let preset: usize = args.preset.into();

    // Analyze noise floor (on filtered + gain-normalized audio)
    let result = analyze_audio(&samples[0], sample_rate);
    println!(
        "  SNR: {:.1}dB, Speech: {:.0}%",
        result.analysis.overall_snr_db,
        result.analysis.speech_density * 100.0
    );

    // Apply denoiser
    println!(
        "  Preset: {} ({})",
        preset,
        get_preset(preset).unwrap().name
    );
    if is_stereo {
        let mut left_denoiser =
            RealtimeDenoiser::new_with_preset(sample_rate, preset).expect("Invalid preset");
        let mut right_denoiser =
            RealtimeDenoiser::new_with_preset(sample_rate, preset).expect("Invalid preset");
        left_denoiser.init_with_noise_floor(&result.noise_floor);
        right_denoiser.init_with_noise_floor(&result.noise_floor);
        samples[0] = left_denoiser.process(&samples[0]);
        samples[1] = right_denoiser.process(&samples[1]);
    } else {
        let mut denoiser =
            RealtimeDenoiser::new_with_preset(sample_rate, preset).expect("Invalid preset");
        denoiser.init_with_noise_floor(&result.noise_floor);
        samples[0] = denoiser.process(&samples[0]);
    }

    // =========================================================================
    // SPECTRAL GATE (optional, after denoiser)
    // =========================================================================
    if args.spectral_gate > 0 {
        println!("\n[Spectral Gate]");
        println!(
            "  Preset: {} ({})",
            args.spectral_gate,
            get_gate_preset_name(args.spectral_gate)
        );

        if is_stereo {
            let mut left_gate =
                SpectralGate::new_with_preset(sample_rate, args.spectral_gate).expect("Invalid preset");
            let mut right_gate =
                SpectralGate::new_with_preset(sample_rate, args.spectral_gate).expect("Invalid preset");
            left_gate.init_noise_floor(&result.noise_floor);
            right_gate.init_noise_floor(&result.noise_floor);
            samples[0] = left_gate.process(&samples[0]);
            samples[1] = right_gate.process(&samples[1]);
            println!("    Max GR: {:.1}dB", left_gate.get_max_gain_reduction_db());
        } else {
            let mut gate =
                SpectralGate::new_with_preset(sample_rate, args.spectral_gate).expect("Invalid preset");
            gate.init_noise_floor(&result.noise_floor);
            samples[0] = gate.process(&samples[0]);
            println!("    Max GR: {:.1}dB", gate.get_max_gain_reduction_db());
        }
    }

    // =========================================================================
    // PEAK ATTENUATION (optional, detect and remove tonal noise)
    // =========================================================================
    if args.depeak {
        println!("\n[Peak Attenuation]");

        // Analyze for tonal peaks using first second of audio
        let analyze_samples = (sample_rate as usize).min(samples[0].len());
        let params = PeakAttenuatorParams::default();
        let peak_profile = detect_tonal_peaks(&samples[0][..analyze_samples], sample_rate, &params);

        if peak_profile.peak_bins.is_empty() {
            println!("  No tonal peaks detected");
        } else {
            let peak_freqs = peak_profile.get_frequencies(sample_rate);
            println!("  Detected {} tonal peaks:", peak_profile.peak_bins.len());
            for (i, freq) in peak_freqs.iter().take(5).enumerate() {
                println!(
                    "    {}: {:.0}Hz ({:.1}dB prominence)",
                    i + 1,
                    freq,
                    peak_profile.prominences_db[i]
                );
            }
            if peak_freqs.len() > 5 {
                println!("    ... and {} more", peak_freqs.len() - 5);
            }

            // Process each channel
            if is_stereo {
                let mut left_attenuator = PeakAttenuator::new(sample_rate);
                let mut right_attenuator = PeakAttenuator::new(sample_rate);
                left_attenuator.set_max_attenuation_db(args.depeak_max_db);
                right_attenuator.set_max_attenuation_db(args.depeak_max_db);
                left_attenuator.init_with_profile(peak_profile.clone());
                right_attenuator.init_with_profile(peak_profile);
                samples[0] = left_attenuator.process(&samples[0]);
                samples[1] = right_attenuator.process(&samples[1]);
                println!("    Max attenuation: {:.1}dB", left_attenuator.get_max_attenuation_db());
            } else {
                let mut attenuator = PeakAttenuator::new(sample_rate);
                attenuator.set_max_attenuation_db(args.depeak_max_db);
                attenuator.init_with_profile(peak_profile);
                samples[0] = attenuator.process(&samples[0]);
                println!("    Max attenuation: {:.1}dB", attenuator.get_max_attenuation_db());
            }
        }
    }

    // =========================================================================
    // DYNAMICS & EQ
    // =========================================================================
    println!("\n[Processing]");

    // FET Compressor (optional)
    if args.fet {
        let mut fetcomp = StereoFetCompressor::new_default(sample_rate as f32);
        println!("  FetComp: threshold -18dB, ratio 4:1, attack 1ms, release 100ms");
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fetcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fetcomp.process_mono(&mut samples[0]);
        }
        println!("    Max GR: {:.1}dB", fetcomp.get_gain_reduction_db());
    } else {
        // Peak compressor
        let mut peakcomp = StereoVcaPeakComp::new(sample_rate as f32);
        let profile = peakcomp.configure(&samples).clone();
        println!(
            "  PeakComp: threshold {:.1}dB",
            profile.histogram_threshold_db
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            peakcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            peakcomp.process_mono(&mut samples[0]);
        }
    }

    // Spectral analysis (on denoised audio - used by FixEq and EnhanceEq)
    let mono = if is_stereo {
        analysis::utils::mix_to_mono(&samples[0], &samples[1])
    } else {
        samples[0].clone()
    };
    let spectrum = analysis::SpectralAnalysis::new(&mono, sample_rate);

    // FixEq
    let mut fixeq = FixEq::new(sample_rate as f32);
    fixeq.configure_from_spectrum(&spectrum, preset, is_stereo);
    println!(
        "  FixEq: demud {:.0}%, corrA {:.0}%, corrB {:.0}%",
        fixeq.get_demud_strength() * 100.0,
        fixeq.get_correction_a_strength() * 100.0,
        fixeq.get_correction_b_strength() * 100.0
    );
    if is_stereo {
        let (left, right) = samples.split_at_mut(1);
        fixeq.process_stereo(&mut left[0], &mut right[0]);
    } else {
        fixeq.process_mono(&mut samples[0]);
    }

    // De-esser (analyzes current audio state)
    let mut deesser = StereoDeEsser::new(sample_rate as f32);
    let sibilance = deesser.configure(&samples).clone();
    println!(
        "  DeEsser: {:.0}-{:.0}Hz, strength {:.0}%",
        sibilance.start_freq,
        sibilance.stop_freq,
        deesser.get_strength() * 100.0
    );
    let mut max_gr = 0.0f32;
    if is_stereo {
        let (left, right) = samples.split_at_mut(1);
        for (l, r) in left[0].iter_mut().zip(right[0].iter_mut()) {
            deesser.process_stereo(std::slice::from_mut(l), std::slice::from_mut(r));
            max_gr = max_gr.min(deesser.get_gain_reduction_db());
        }
    } else {
        for s in samples[0].iter_mut() {
            deesser.process_mono(std::slice::from_mut(s));
            max_gr = max_gr.min(deesser.get_gain_reduction_db());
        }
    }
    println!("    Max GR: {:.1}dB", max_gr);

    // Saturation
    let mut channel9: Stereo<Channel9> = Stereo::new(sample_rate as f32);
    channel9.set_both(|c| c.set_drive(0.2));
    println!("  Saturation: drive 40%");
    if is_stereo {
        let (left, right) = samples.split_at_mut(1);
        channel9.process_stereo(&mut left[0], &mut right[0]);
    } else {
        channel9.process_mono(&mut samples[0]);
    }

    // Compressor
    let mut compressor: Stereo<ButterComp2> = Stereo::new(sample_rate as f32);
    compressor.set_both(|c| c.set_compress(0.8));
    println!("  Compressor: 80%");
    if is_stereo {
        let (left, right) = samples.split_at_mut(1);
        compressor.process_stereo(&mut left[0], &mut right[0]);
    } else {
        compressor.process_mono(&mut samples[0]);
    }

    // Fresh spectral analysis for EnhanceEQ (on current audio state)
    let mono_for_enhance = if is_stereo {
        analysis::utils::mix_to_mono(&samples[0], &samples[1])
    } else {
        samples[0].clone()
    };

    if !args.radio {
        let enhance_spectrum = analysis::SpectralAnalysis::new(&mono_for_enhance, sample_rate);
        // Enhance EQ (uses fresh spectrum)
        let mut enhanceeq = StereoEnhanceEq::new(sample_rate as f32);
        enhanceeq.configure_from_spectrum(&enhance_spectrum);
        println!(
            "  EnhanceEQ: low-mid: {:+.1}dB, presence {:+.1}dB, air {:+.1}dB",
            enhanceeq.get_lowmid_gain(),
            enhanceeq.get_presence_gain(),
            enhanceeq.get_shelf_gain()
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            enhanceeq.process_stereo(&mut left[0], &mut right[0]);
        } else {
            enhanceeq.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // RADIO VOICE EQ (optional)
    // =========================================================================
    if args.radio {
        println!("\n[Radio Voice EQ]");
        let mut radio = RadioVoiceProcessor::new(sample_rate);
        radio.set_amount(args.radio_amount);

        // Analyze (uses mono mix)
        let mono = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        radio.analyze(&mono);

        // Process
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            radio.process(&mut left[0]);
            radio.reset();
            radio.process(&mut right[0]);
        } else {
            radio.process(&mut samples[0]);
        }

        println!("  f0: {:.1} Hz (sibilance: {:.0}%)", radio.get_detected_f0(), radio.get_sibilance_level() * 100.0);
        println!("  CPP: {:.1} dB (harmonicity)", radio.get_cpp());
        if let Some(delay) = radio.get_echo_delay_ms() {
            println!("  Echo: {:.0}ms delay ({:.0}% strength)", delay, radio.get_echo_strength() * 100.0);
        }
        println!("  HPF: {:.0} Hz", radio.get_hpf_freq());
        println!(
            "  Low: {:+.1} dB @ {:.0} Hz",
            radio.get_low_shelf_gain(),
            radio.get_low_shelf_freq()
        );
        println!(
            "  Mud: {:+.1} dB @ {:.0} Hz",
            radio.get_mud_gain(),
            radio.get_mud_freq()
        );
        if radio.get_mid_gain().abs() > 0.1 {
            println!(
                "  Mid: {:+.1} dB @ {:.0} Hz",
                radio.get_mid_gain(),
                radio.get_mid_freq()
            );
        } else {
            println!("  Mid: off");
        }
        println!(
            "  Presence: {:+.1} dB @ {:.0} Hz",
            radio.get_presence_gain(),
            radio.get_presence_freq()
        );
        println!("  Air: {:+.1} dB", radio.get_air_gain());
    }

    // =========================================================================
    // OUTPUT STAGE
    // =========================================================================
    println!("\n[Output]");

    // LUFS normalization
    let lufs = measure_integrated_lufs(&samples, sample_rate);
    let lufs_gain_db = DEFAULT_TARGET_LUFS - lufs;
    println!(
        "  LUFS: {:.1} -> target {:.1} ({:+.1}dB)",
        lufs, DEFAULT_TARGET_LUFS, lufs_gain_db
    );
    apply_gain(&mut samples, lufs_gain_db);

    // Limiter
    let mut limiter = Limiter::new(-1.0, 5.0, 100.0, sample_rate as f32);
    let stats = if is_stereo {
        let (left, right) = samples.split_at_mut(1);
        limiter.process_stereo(&mut left[0], &mut right[0])
    } else {
        limiter.process_mono(&mut samples[0])
    };
    println!("  Limiter: -1dBTP, max GR {:.1}dB", stats.max_reduction_db);

    // =========================================================================
    // SAVE
    // =========================================================================
    let output_path = args.output.unwrap_or_else(|| {
        let stem = args
            .input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let name = PRESETS[preset - 1].name.to_lowercase();
        let output_dir = PathBuf::from("sounds_out");
        std::fs::create_dir_all(&output_dir).ok();
        output_dir.join(format!("{stem}_denoised_{preset}_{name}.wav"))
    });

    println!("\nSaving: {}", output_path.display());
    save_wav(&output_path, &samples, sample_rate)?;
    println!("Done.");
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
