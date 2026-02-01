mod chain;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chain::{ChainPreset, CompressorType, OutputSetting, ProcessorSetting};
use clap::Parser;
use hound::{SampleFormat, WavSpec, WavWriter};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use poddyclip::analysis;
use poddyclip::analysis::lufs::measure_integrated_lufs;
use poddyclip::denoiser::{
    analyze_audio, detect_tonal_peaks, get_gate_preset_name, get_preset, PeakAttenuator,
    PeakAttenuatorParams, RealtimeDenoiser, SpectralGate, DEFAULT_PRESET, PRESETS,
};
use poddyclip::dynamics::autogain::{
    analyze_gain, apply_gain, linear_to_db, DEFAULT_TARGET_PEAK_DB, DEFAULT_TARGET_RMS_DB,
};
use poddyclip::dynamics::limiter::Limiter;
use poddyclip::dynamics::{
    get_buttercomp_preset, get_buttercomp_preset_name, get_expander_preset_name,
    get_fetcomp_preset_name, get_peakcomp_preset_name, ButterComp2, StereoExpander,
    StereoFetCompressor, StereoVcaPeakComp,
};
use poddyclip::eq::deesser::StereoDeEsser;
use poddyclip::eq::{
    get_eq_preset, get_eq_preset_name, FilterChain, FixEq, HighPassSlope, RadioVoiceProcessor,
    StereoEnhanceEq,
};
use poddyclip::repair::Declicker;
use poddyclip::saturation::{
    get_saturation_preset, get_saturation_preset_name, Channel9, TapeGlue,
};
use poddyclip::traits::{Stereo, StereoProcessor};

#[derive(Parser)]
#[command(name = "poddyclip")]
#[command(about = "Spectral Subtraction Denoiser", long_about = None)]
#[command(after_help = r#"Preset Modes:
  1 = Subtle   - Minimal processing, preserves everything
  2 = Balanced - Subtle noise reduction (default)
  3 = Intense  - Noticeable noise reduction"#)]
struct Args {
    /// Input audio file (WAV, MP3, etc.)
    input: Option<PathBuf>,

    /// Output WAV file
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Denoising strength 1-3
    #[arg(short, long, default_value_t = DEFAULT_PRESET, value_parser = clap::value_parser!(u8).range(1..=3))]
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

    /// De-reverb strength 1-3 (0 = disabled)
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
    dereverb: u8,

    /// Use AI (DeepFilterNet) denoiser for voice (auto-tuned based on SNR)
    #[cfg(feature = "deepfilter")]
    #[arg(long)]
    ai_denoise: bool,

    /// Spectral gate strength 1-3 (0 = off) - handles intermittent noise
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=3))]
    spectral_gate: u8,

    /// Enable tonal peak attenuation (hum, whine removal)
    #[arg(long)]
    depeak: bool,

    /// Maximum peak attenuation in dB (with --depeak)
    #[arg(long, default_value_t = 18.0)]
    depeak_max_db: f32,

    // =========================================================================
    // DISABLE FLAGS - skip individual processors
    // =========================================================================
    /// Skip expander (noise gate)
    #[arg(long)]
    disable_expander: bool,

    /// Skip compressor (peakcomp or fetcomp)
    #[arg(long)]
    disable_comp: bool,

    /// Skip FixEQ
    #[arg(long)]
    disable_fixeq: bool,

    /// Skip de-esser
    #[arg(long)]
    disable_deesser: bool,

    /// Skip Channel9 saturation
    #[arg(long)]
    disable_saturation: bool,

    /// Skip ButterComp
    #[arg(long)]
    disable_buttercomp: bool,

    /// Skip EnhanceEQ
    #[arg(long)]
    disable_enhanceeq: bool,

    /// Skip TapeGlue
    #[arg(long)]
    disable_tape: bool,

    /// Skip limiter (not recommended)
    #[arg(long)]
    disable_limiter: bool,

    // =========================================================================
    // PROCESSOR PRESETS - fine-tune individual processors (1-3 scale)
    // =========================================================================
    /// Expander preset 1-3 (1=Subtle, 2=Balanced, 3=Intense)
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
    expander_preset: u8,

    /// FET compressor preset 1-3 (used with --fet)
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
    fetcomp_preset: u8,

    /// Peak compressor preset 1-3 (default compressor)
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
    peakcomp_preset: u8,

    /// Saturation preset 1-3 (Channel9 + TapeGlue)
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
    saturation_preset: u8,

    /// EQ preset 1-3 (EnhanceEQ gains)
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
    eq_preset: u8,

    /// ButterComp preset 1-3
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(1..=3))]
    buttercomp_preset: u8,

    // =========================================================================
    // CHAIN PRESETS
    // =========================================================================
    /// Use a chain preset from chains/ directory
    #[arg(long, value_name = "NAME")]
    chain: Option<String>,

    /// List available chain presets
    #[arg(long)]
    list_chains: bool,

    /// Show per-stage timing breakdown
    #[arg(long)]
    benchmark: bool,
}

/// Timing data for each processing stage
#[derive(Default)]
struct StageTimings {
    load: Duration,
    filter: Duration,
    input_gain: Duration,
    declick: Duration,
    dereverb: Duration,
    denoise: Duration,
    ai_denoise: Duration,
    spectral_gate: Duration,
    peak_attenuation: Duration,
    expander: Duration,
    compressor: Duration,
    fixeq: Duration,
    deesser: Duration,
    saturation: Duration,
    buttercomp: Duration,
    enhance: Duration,
    tape: Duration,
    radio: Duration,
    output: Duration,
    save: Duration,
}

impl StageTimings {
    fn total(&self) -> Duration {
        self.load
            + self.filter
            + self.input_gain
            + self.declick
            + self.dereverb
            + self.denoise
            + self.ai_denoise
            + self.spectral_gate
            + self.peak_attenuation
            + self.expander
            + self.compressor
            + self.fixeq
            + self.deesser
            + self.saturation
            + self.buttercomp
            + self.enhance
            + self.tape
            + self.radio
            + self.output
            + self.save
    }

    fn percent(&self, stage: Duration) -> f64 {
        let total = self.total().as_secs_f64();
        if total == 0.0 {
            0.0
        } else {
            (stage.as_secs_f64() / total) * 100.0
        }
    }

    fn print(&self, audio_duration_secs: f64) {
        let total = self.total();
        println!("\n┌─────────────────────────────────────────┐");
        println!("│          Stage Timings                  │");
        println!("├─────────────────────────────────────────┤");

        // Print each stage (skip zeros)
        let stages: [(&str, Duration); 20] = [
            ("Load", self.load),
            ("Filter", self.filter),
            ("Input Gain", self.input_gain),
            ("Declick", self.declick),
            ("DeReverb", self.dereverb),
            ("Denoise", self.denoise),
            ("AI Denoise", self.ai_denoise),
            ("Spectral Gate", self.spectral_gate),
            ("Peak Atten", self.peak_attenuation),
            ("Expander", self.expander),
            ("Compressor", self.compressor),
            ("FixEQ", self.fixeq),
            ("DeEsser", self.deesser),
            ("Saturation", self.saturation),
            ("ButterComp", self.buttercomp),
            ("Enhance", self.enhance),
            ("Tape", self.tape),
            ("Radio", self.radio),
            ("Output", self.output),
            ("Save", self.save),
        ];

        for (name, duration) in stages {
            if duration.as_nanos() > 0 {
                println!(
                    "│ {:14} {:>8.2}ms ({:>5.1}%)     │",
                    name,
                    duration.as_secs_f64() * 1000.0,
                    self.percent(duration)
                );
            }
        }

        println!("├─────────────────────────────────────────┤");
        println!(
            "│ {:14} {:>8.2}ms              │",
            "Total",
            total.as_secs_f64() * 1000.0
        );
        println!(
            "│ {:14} {:>8.1}x               │",
            "Realtime",
            audio_duration_secs / total.as_secs_f64()
        );
        println!("└─────────────────────────────────────────┘");
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // =========================================================================
    // LIST CHAINS (early exit)
    // =========================================================================
    if args.list_chains {
        let chains = chain::list_chains();
        if chains.is_empty() {
            println!("No chain presets found in chains/ directory");
        } else {
            println!("Available chain presets:");
            for (name, desc) in chains {
                if desc.is_empty() {
                    println!("  {}", name);
                } else {
                    println!("  {} - {}", name, desc);
                }
            }
        }
        return Ok(());
    }

    // =========================================================================
    // VALIDATE INPUT
    // =========================================================================
    let input = match args.input {
        Some(ref path) => path.clone(),
        None => {
            bail!("Input file is required. Usage: poddyclip <INPUT> [OPTIONS]");
        }
    };

    // =========================================================================
    // LOAD CHAIN PRESET (if specified)
    // =========================================================================
    let chain_preset: Option<ChainPreset> = if let Some(ref name) = args.chain {
        match chain::load_chain(name) {
            Ok(preset) => {
                println!(
                    "Using chain: {} ({})",
                    preset.name,
                    preset.description.as_deref().unwrap_or("")
                );
                Some(preset)
            }
            Err(e) => {
                bail!("{}", e);
            }
        }
    } else {
        None
    };

    // =========================================================================
    // RESOLVE SETTINGS (CLI overrides chain, chain overrides defaults)
    // =========================================================================
    // Helper to check if CLI arg was explicitly set (not default)
    // Since clap doesn't track this directly, we use a convention:
    // - For presets: CLI wins if != 3 (default), otherwise use chain
    // - For bools: CLI wins (disable flags)

    // Denoiser preset
    let denoiser_preset = chain_preset
        .as_ref()
        .map(|c| c.denoiser)
        .unwrap_or(args.preset);
    let denoiser_preset = if args.preset != DEFAULT_PRESET {
        args.preset
    } else {
        denoiser_preset
    };

    // Expander
    let (expander_enabled, expander_preset) = resolve_processor_setting(
        !args.disable_expander,
        args.expander_preset,
        chain_preset.as_ref().map(|c| &c.expander),
    );

    // Compressor
    let (comp_enabled, use_fet, comp_preset) = resolve_compressor_setting(
        !args.disable_comp,
        args.fet,
        args.peakcomp_preset,
        args.fetcomp_preset,
        chain_preset.as_ref().map(|c| &c.compressor),
    );

    // FixEQ
    let fixeq_enabled = if args.disable_fixeq {
        false
    } else {
        chain_preset.as_ref().map(|c| c.fixeq).unwrap_or(true)
    };

    // DeEsser
    let deesser_enabled = if args.disable_deesser {
        false
    } else {
        chain_preset.as_ref().map(|c| c.deesser).unwrap_or(true)
    };

    // Saturation (Channel9)
    let (saturation_enabled, saturation_preset) = resolve_processor_setting(
        !args.disable_saturation,
        args.saturation_preset,
        chain_preset.as_ref().map(|c| &c.saturation),
    );

    // ButterComp
    let (buttercomp_enabled, buttercomp_preset) = resolve_processor_setting(
        !args.disable_buttercomp,
        args.buttercomp_preset,
        chain_preset.as_ref().map(|c| &c.buttercomp),
    );

    // EnhanceEQ
    let (enhanceeq_enabled, eq_preset) = resolve_processor_setting(
        !args.disable_enhanceeq,
        args.eq_preset,
        chain_preset.as_ref().map(|c| &c.enhanceeq),
    );

    // TapeGlue
    let (tape_enabled, tape_preset) = resolve_processor_setting(
        !args.disable_tape,
        args.saturation_preset, // TapeGlue uses saturation preset
        chain_preset.as_ref().map(|c| &c.tape),
    );

    // Output (LUFS + limiter paired)
    let (output_enabled, lufs_target) = resolve_output_setting(
        !args.disable_limiter,
        chain_preset.as_ref().map(|c| &c.output),
    );

    // =========================================================================
    // TIMING SETUP
    // =========================================================================
    let mut timings = StageTimings::default();

    // =========================================================================
    // LOAD
    // =========================================================================
    println!("Loading: {}", input.display());
    let start = Instant::now();
    let (mut samples, sample_rate) = load_audio(&input)?;
    timings.load = start.elapsed();
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

        let start = Instant::now();
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
        timings.filter = start.elapsed();
    }

    // Input gain
    println!("\n[Input Gain]");
    let start = Instant::now();
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
    timings.input_gain = start.elapsed();

    // =========================================================================
    // REPAIR (OFFLINE)
    // =========================================================================
    if args.declick {
        println!("\n[Declick]");
        let start = Instant::now();
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
        timings.declick = start.elapsed();
    }

    // =========================================================================
    // DE-REVERB (optional, runs BEFORE denoiser)
    // =========================================================================
    if args.dereverb > 0 {
        println!("\n[DeReverb]");
        let start = Instant::now();

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

        let mut dereverb =
            poddyclip::dereverb::DeReverbProcessor::new_with_preset(sample_rate, dereverb_preset)
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
        timings.dereverb = start.elapsed();
    }

    // =========================================================================
    // DENOISE
    // =========================================================================
    println!("\n[Denoise]");
    let start = Instant::now();
    let preset: usize = denoiser_preset.into();

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
    timings.denoise = start.elapsed();

    // =========================================================================
    // AI DENOISE (DeepFilterNet) - optional, runs before spectral denoiser
    // =========================================================================
    #[cfg(feature = "deepfilter")]
    if args.ai_denoise {
        println!("\n[AI Denoise (DeepFilterNet)]");
        let start = Instant::now();

        // Analyze for auto-tuning
        let df_analysis = poddyclip::deepfilter::analyze_for_deepfilter(&samples[0], sample_rate);
        println!(
            "  SNR: {:.1}dB ({})",
            df_analysis.estimated_snr,
            df_analysis.noise_severity()
        );
        if df_analysis.has_dc_offset {
            println!("  DC offset detected, will be removed");
        }
        if df_analysis.low_freq_energy_ratio > 0.20 {
            println!(
                "  Low-freq energy: {:.0}%, applying high-pass",
                df_analysis.low_freq_energy_ratio * 100.0
            );
        }

        // Create denoiser with analysis (builds model with correct params from start)
        match poddyclip::deepfilter::DeepFilterDenoiser::new_with_analysis(sample_rate, &df_analysis) {
            Ok(mut denoiser) => {
                if is_stereo {
                    samples[0] = denoiser.process_with_analysis(&samples[0], &df_analysis);
                    denoiser.reset();
                    // Re-analyze right channel (may have different noise characteristics)
                    let df_analysis_r =
                        poddyclip::deepfilter::analyze_for_deepfilter(&samples[1], sample_rate);
                    samples[1] = denoiser.process_with_analysis(&samples[1], &df_analysis_r);
                } else {
                    samples[0] = denoiser.process_with_analysis(&samples[0], &df_analysis);
                }
                println!("  AI denoising complete");
            }
            Err(e) => {
                eprintln!("  Warning: Failed to initialize DeepFilterNet: {}", e);
                eprintln!("  Skipping AI denoising, will use spectral denoiser only");
            }
        }
        timings.ai_denoise = start.elapsed();
    }

    // =========================================================================
    // SPECTRAL GATE (optional, after denoiser)
    // =========================================================================
    if args.spectral_gate > 0 {
        println!("\n[Spectral Gate]");
        let start = Instant::now();
        println!(
            "  Preset: {} ({})",
            args.spectral_gate,
            get_gate_preset_name(args.spectral_gate)
        );

        if is_stereo {
            let mut left_gate = SpectralGate::new_with_preset(sample_rate, args.spectral_gate)
                .expect("Invalid preset");
            let mut right_gate = SpectralGate::new_with_preset(sample_rate, args.spectral_gate)
                .expect("Invalid preset");
            left_gate.init_noise_floor(&result.noise_floor);
            right_gate.init_noise_floor(&result.noise_floor);
            samples[0] = left_gate.process(&samples[0]);
            samples[1] = right_gate.process(&samples[1]);
            println!("    Max GR: {:.1}dB", left_gate.get_max_gain_reduction_db());
        } else {
            let mut gate = SpectralGate::new_with_preset(sample_rate, args.spectral_gate)
                .expect("Invalid preset");
            gate.init_noise_floor(&result.noise_floor);
            samples[0] = gate.process(&samples[0]);
            println!("    Max GR: {:.1}dB", gate.get_max_gain_reduction_db());
        }
        timings.spectral_gate = start.elapsed();
    }

    // =========================================================================
    // PEAK ATTENUATION (optional, detect and remove tonal noise)
    // =========================================================================
    if args.depeak {
        println!("\n[Peak Attenuation]");
        let start = Instant::now();

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
                println!(
                    "    Max attenuation: {:.1}dB",
                    left_attenuator.get_max_attenuation_db()
                );
            } else {
                let mut attenuator = PeakAttenuator::new(sample_rate);
                attenuator.set_max_attenuation_db(args.depeak_max_db);
                attenuator.init_with_profile(peak_profile);
                samples[0] = attenuator.process(&samples[0]);
                println!(
                    "    Max attenuation: {:.1}dB",
                    attenuator.get_max_attenuation_db()
                );
            }
        }
        timings.peak_attenuation = start.elapsed();
    }

    // =========================================================================
    // DYNAMICS & EQ
    // =========================================================================
    println!("\n[Processing]");

    // Expander (first - reduces noise in quiet passages)
    if expander_enabled {
        let start = Instant::now();
        let mut expander = StereoExpander::new_with_preset(sample_rate as f32, expander_preset)
            .expect("Invalid expander preset");
        println!(
            "  Expander: preset {} ({})",
            expander_preset,
            get_expander_preset_name(expander_preset)
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            expander.process_stereo(&mut left[0], &mut right[0]);
        } else {
            expander.process_mono(&mut samples[0]);
        }
        println!("    Max GR: {:.1}dB", expander.get_max_gain_reduction_db());
        timings.expander = start.elapsed();
    }

    // Compressor (FET or Peak)
    if comp_enabled {
        let start = Instant::now();
        if use_fet {
            let mut fetcomp = StereoFetCompressor::new_with_preset(sample_rate as f32, comp_preset)
                .expect("Invalid fetcomp preset");
            println!(
                "  FetComp: preset {} ({})",
                comp_preset,
                get_fetcomp_preset_name(comp_preset)
            );
            if is_stereo {
                let (left, right) = samples.split_at_mut(1);
                fetcomp.process_stereo(&mut left[0], &mut right[0]);
            } else {
                fetcomp.process_mono(&mut samples[0]);
            }
            println!("    Max GR: {:.1}dB", fetcomp.get_gain_reduction_db());
        } else {
            // Peak compressor with preset
            let mut peakcomp = StereoVcaPeakComp::new_with_preset(sample_rate as f32, comp_preset)
                .expect("Invalid peakcomp preset");
            // Still run analysis to set auto threshold if needed
            let profile = peakcomp.configure(&samples).clone();
            println!(
                "  PeakComp: preset {} ({}), threshold {:.1}dB",
                comp_preset,
                get_peakcomp_preset_name(comp_preset),
                profile.histogram_threshold_db
            );
            if is_stereo {
                let (left, right) = samples.split_at_mut(1);
                peakcomp.process_stereo(&mut left[0], &mut right[0]);
            } else {
                peakcomp.process_mono(&mut samples[0]);
            }
        }
        timings.compressor = start.elapsed();
    }

    // Spectral analysis (on denoised audio - used by FixEq and EnhanceEq)
    let mono = if is_stereo {
        analysis::utils::mix_to_mono(&samples[0], &samples[1])
    } else {
        samples[0].clone()
    };
    let spectrum = analysis::SpectralAnalysis::new(&mono, sample_rate);

    // FixEq
    if fixeq_enabled {
        let start = Instant::now();
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
        timings.fixeq = start.elapsed();
    }

    // De-esser
    if deesser_enabled {
        let start = Instant::now();
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
        timings.deesser = start.elapsed();
    }

    // Saturation (Channel9)
    if saturation_enabled {
        let start = Instant::now();
        let sat_preset =
            get_saturation_preset(saturation_preset).expect("Invalid saturation preset");
        let mut channel9: Stereo<Channel9> = Stereo::new(sample_rate as f32);
        channel9.set_both(|c| c.set_drive(sat_preset.channel9_drive));
        println!(
            "  Channel9: preset {} ({}), drive {:.0}%",
            saturation_preset,
            get_saturation_preset_name(saturation_preset),
            sat_preset.channel9_drive * 100.0
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            channel9.process_stereo(&mut left[0], &mut right[0]);
        } else {
            channel9.process_mono(&mut samples[0]);
        }
        timings.saturation = start.elapsed();
    }

    // ButterComp
    if buttercomp_enabled {
        let start = Instant::now();
        let buttercomp_amount =
            get_buttercomp_preset(buttercomp_preset).expect("Invalid buttercomp preset");
        let mut compressor: Stereo<ButterComp2> = Stereo::new(sample_rate as f32);
        compressor.set_both(|c| c.set_compress(buttercomp_amount));
        println!(
            "  ButterComp: preset {} ({}), {:.0}%",
            buttercomp_preset,
            get_buttercomp_preset_name(buttercomp_preset),
            buttercomp_amount * 100.0
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            compressor.process_stereo(&mut left[0], &mut right[0]);
        } else {
            compressor.process_mono(&mut samples[0]);
        }
        timings.buttercomp = start.elapsed();
    }

    // Fresh spectral analysis for EnhanceEQ (on current audio state)
    let mono_for_enhance = if is_stereo {
        analysis::utils::mix_to_mono(&samples[0], &samples[1])
    } else {
        samples[0].clone()
    };

    // EnhanceEQ (or RadioVoice)
    if !args.radio && enhanceeq_enabled {
        let start = Instant::now();
        let eq_preset_data = get_eq_preset(eq_preset).expect("Invalid EQ preset");
        let enhance_spectrum = analysis::SpectralAnalysis::new(&mono_for_enhance, sample_rate);
        let mut enhanceeq_proc = StereoEnhanceEq::new(sample_rate as f32);
        enhanceeq_proc.configure_from_spectrum(&enhance_spectrum);

        // Scale gains by preset
        let base_lowmid = enhanceeq_proc.get_lowmid_gain();
        let base_presence = enhanceeq_proc.get_presence_gain();
        let base_air = enhanceeq_proc.get_shelf_gain();

        // Apply preset scaling (preset 3 = 1.0x, others scale proportionally)
        let scale = eq_preset_data.lowmid_cut_db / -3.0; // Normalize to preset 3
        enhanceeq_proc.set_lowmid_gain(base_lowmid * scale);
        enhanceeq_proc.set_presence_gain(base_presence * scale);
        enhanceeq_proc.set_shelf_gain(base_air * scale);

        println!(
            "  EnhanceEQ: preset {} ({}), low-mid: {:+.1}dB, presence {:+.1}dB, air {:+.1}dB",
            eq_preset,
            get_eq_preset_name(eq_preset),
            enhanceeq_proc.get_lowmid_gain(),
            enhanceeq_proc.get_presence_gain(),
            enhanceeq_proc.get_shelf_gain()
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            enhanceeq_proc.process_stereo(&mut left[0], &mut right[0]);
        } else {
            enhanceeq_proc.process_mono(&mut samples[0]);
        }
        timings.enhance = start.elapsed();
    }

    // TapeGlue
    if tape_enabled {
        let start = Instant::now();
        let sat_preset = get_saturation_preset(tape_preset).expect("Invalid saturation preset");
        let mut tape_left = TapeGlue::new(sample_rate as f64);
        let mut tape_right = TapeGlue::new(sample_rate as f64);
        tape_left.set_warmth(sat_preset.tape_warmth);
        tape_right.set_warmth(sat_preset.tape_warmth);
        println!("  TapeGlue: warmth {:.0}%", sat_preset.tape_warmth * 100.0);
        if is_stereo {
            for sample in samples[0].iter_mut() {
                *sample = tape_left.process(*sample);
            }
            for sample in samples[1].iter_mut() {
                *sample = tape_right.process(*sample);
            }
        } else {
            for sample in samples[0].iter_mut() {
                *sample = tape_left.process(*sample);
            }
        }
        timings.tape = start.elapsed();
    }

    // =========================================================================
    // RADIO VOICE EQ (optional)
    // =========================================================================
    if args.radio {
        println!("\n[Radio Voice EQ]");
        let start = Instant::now();
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

        println!(
            "  f0: {:.1} Hz (sibilance: {:.0}%)",
            radio.get_detected_f0(),
            radio.get_sibilance_level() * 100.0
        );
        println!("  CPP: {:.1} dB (harmonicity)", radio.get_cpp());
        if let Some(delay) = radio.get_echo_delay_ms() {
            println!(
                "  Echo: {:.0}ms delay ({:.0}% strength)",
                delay,
                radio.get_echo_strength() * 100.0
            );
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
        timings.radio = start.elapsed();
    }

    // =========================================================================
    // OUTPUT STAGE
    // =========================================================================
    println!("\n[Output]");

    // LUFS normalization + Limiter (paired together)
    if output_enabled {
        let start = Instant::now();
        let lufs = measure_integrated_lufs(&samples, sample_rate);
        let lufs_gain_db = lufs_target - lufs;
        println!(
            "  LUFS: {:.1} -> target {:.1} ({:+.1}dB)",
            lufs, lufs_target, lufs_gain_db
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
        timings.output = start.elapsed();
    } else {
        println!("  Output: DISABLED (no LUFS normalization or limiting)");
    }

    // =========================================================================
    // SAVE
    // =========================================================================
    let output_path = args.output.unwrap_or_else(|| {
        let stem = input
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let name = PRESETS[preset - 1].name.to_lowercase();
        let output_dir = PathBuf::from("sounds_out");
        std::fs::create_dir_all(&output_dir).ok();
        output_dir.join(format!("{stem}_denoised_{preset}_{name}.wav"))
    });

    println!("\nSaving: {}", output_path.display());
    let start = Instant::now();
    save_wav(&output_path, &samples, sample_rate)?;
    timings.save = start.elapsed();
    println!("Done.");

    // =========================================================================
    // BENCHMARK OUTPUT
    // =========================================================================
    if args.benchmark {
        let audio_duration_secs = samples[0].len() as f64 / sample_rate as f64;
        timings.print(audio_duration_secs);
    }

    Ok(())
}

/// Resolve processor setting from CLI args and chain preset
/// Returns (enabled, preset)
fn resolve_processor_setting(
    cli_enabled: bool,
    cli_preset: u8,
    chain_setting: Option<&ProcessorSetting>,
) -> (bool, u8) {
    // CLI disable flag always wins
    if !cli_enabled {
        return (false, cli_preset);
    }

    // Check chain preset
    match chain_setting {
        Some(ProcessorSetting::Disabled) => (false, 2),
        Some(ProcessorSetting::Preset(p)) => {
            // CLI preset wins if explicitly set (not default)
            if cli_preset != 2 {
                (true, cli_preset)
            } else {
                (true, *p)
            }
        }
        Some(ProcessorSetting::Default) | None => {
            // Use CLI preset (or default 2)
            (true, cli_preset)
        }
    }
}

/// Resolve compressor setting from CLI args and chain preset
/// Returns (enabled, use_fet, preset)
fn resolve_compressor_setting(
    cli_enabled: bool,
    cli_use_fet: bool,
    cli_peak_preset: u8,
    cli_fet_preset: u8,
    chain_setting: Option<&chain::CompressorSetting>,
) -> (bool, bool, u8) {
    // CLI disable flag always wins
    if !cli_enabled {
        return (false, false, 2);
    }

    match chain_setting {
        Some(chain::CompressorSetting { enabled: false, .. }) => (false, false, 2),
        Some(chain::CompressorSetting {
            comp_type,
            preset,
            enabled: true,
        }) => {
            // CLI --fet flag overrides chain type
            let use_fet = if cli_use_fet {
                true
            } else {
                *comp_type == CompressorType::Fet
            };

            // CLI preset overrides chain if explicitly set
            let final_preset = if use_fet {
                if cli_fet_preset != 2 {
                    cli_fet_preset
                } else {
                    *preset
                }
            } else if cli_peak_preset != 2 {
                cli_peak_preset
            } else {
                *preset
            };

            (true, use_fet, final_preset)
        }
        None => {
            // No chain, use CLI settings
            let preset = if cli_use_fet {
                cli_fet_preset
            } else {
                cli_peak_preset
            };
            (true, cli_use_fet, preset)
        }
    }
}

/// Resolve output setting from CLI args and chain preset
/// Returns (enabled, lufs_target)
fn resolve_output_setting(cli_enabled: bool, chain_setting: Option<&OutputSetting>) -> (bool, f32) {
    // CLI disable flag always wins
    if !cli_enabled {
        return (false, -16.0);
    }

    match chain_setting {
        Some(OutputSetting::Disabled) => (false, -16.0),
        Some(OutputSetting::LufsTarget(t)) => (true, *t as f32),
        Some(OutputSetting::Default) | None => (true, -16.0),
    }
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
