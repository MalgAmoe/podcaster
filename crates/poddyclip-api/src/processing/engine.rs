//! Audio processing engine - adapts CLI processing chain for API use

use std::path::Path;

use anyhow::Result;
use thiserror::Error;
use tracing::{debug, info, warn};

#[cfg(feature = "deepfilter")]
use poddyclip::deepfilter::{analyze_for_deepfilter, DeepFilterDenoiser};

use poddyclip::analysis;
use poddyclip::analysis::lufs::measure_integrated_lufs;
use poddyclip::denoiser::{
    analyze_audio, detect_tonal_peaks, PeakAttenuator, PeakAttenuatorParams, RealtimeDenoiser,
    SpectralGate,
};
use poddyclip::dynamics::autogain::{analyze_gain, apply_gain, DEFAULT_TARGET_PEAK_DB, DEFAULT_TARGET_RMS_DB};
use poddyclip::dynamics::limiter::Limiter;
use poddyclip::dynamics::{get_buttercomp_preset, ButterComp2, StereoExpander, StereoFetCompressor, StereoVcaPeakComp};
use poddyclip::eq::deesser::StereoDeEsser;
use poddyclip::eq::enhanceeq::StereoEnhanceEq;
use poddyclip::eq::filters::{FilterChain, HighPassSlope};
use poddyclip::eq::fixeq::FixEq;
use poddyclip::eq::radio_voice::RadioVoiceProcessor;
use poddyclip::eq::{get_eq_preset};
use poddyclip::saturation::channel9::Channel9;
use poddyclip::saturation::tape::TapeGlue;
use poddyclip::saturation::get_saturation_preset;
use poddyclip::traits::{Stereo, StereoProcessor};

use crate::models::{CompressorType, ProcessConfig};
use crate::processing::chain::{load_chain, ChainPreset};

/// Error returned when a job is cancelled
#[derive(Debug, Error)]
#[error("Job was cancelled")]
pub struct CancelledError;

/// Progress callback type - returns Ok(()) to continue, Err to cancel
pub type ProgressCallback = Box<dyn Fn(&str, u8) -> Result<(), CancelledError> + Send>;

/// Processing stages for progress tracking
const STAGES: &[&str] = &[
    "decoding",           // 0
    "filters",            // 1
    "input_gain",         // 2
    "analyzing_reverb",   // 3
    "dereverb",           // 4
    "analyzing_noise",    // 5
    "denoise",            // 6
    "ai_denoise",         // 7 - DeepFilterNet AI denoiser
    "spectral_gate",      // 8
    "analyzing_peaks",    // 9
    "peak_attenuation",   // 10
    "expander",           // 11
    "compressor",         // 12
    "analyzing_eq",       // 13
    "fixeq",              // 14
    "deesser",            // 15
    "saturation",         // 16
    "buttercomp",         // 17
    "analyzing_enhance",  // 18
    "enhanceeq",          // 19
    "tape",               // 20
    "radio",              // 21
    "analyzing_levels",   // 22
    "output",             // 23
    "encoding",           // 24
];

/// Run the full processing chain on audio samples
pub fn process_audio(
    samples: &mut Vec<Vec<f32>>,
    sample_rate: u32,
    config: &ProcessConfig,
    chains_dir: &Path,
    on_progress: Option<ProgressCallback>,
) -> Result<()> {
    let is_stereo = samples.len() >= 2;

    // Report progress helper - returns error if cancelled
    let report = |stage: &str, index: u8| -> Result<()> {
        if let Some(ref cb) = on_progress {
            cb(stage, index).map_err(|e| anyhow::anyhow!(e))?;
        }
        Ok(())
    };

    // Load chain preset if specified (overrides individual settings)
    let effective_config = if let Some(chain_name) = &config.chain {
        let chain = load_chain(chain_name, chains_dir)
            .map_err(|e| anyhow::anyhow!("Failed to load chain: {}", e))?;
        config_from_chain(&chain, config)
    } else {
        config.clone()
    };

    // =========================================================================
    // FILTERS
    // =========================================================================
    report("filters", 1)?;

    if effective_config.filters_enabled {
        let slope = if effective_config.hp_slope == 24 {
            HighPassSlope::Slope24dB
        } else {
            HighPassSlope::Slope12dB
        };
        let cutoff = effective_config.hp_cutoff;

        let mut filters = Stereo::from_pair(
            FilterChain::new_with_cutoff(sample_rate as f32, slope, cutoff),
            FilterChain::new_with_cutoff(sample_rate as f32, slope, cutoff),
        );
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            filters.process_stereo(&mut left[0], &mut right[0]);
        } else {
            filters.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // INPUT GAIN
    // =========================================================================
    report("input_gain", 2)?;

    let gain_db = analyze_gain(samples, DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB);
    apply_gain(samples, gain_db);

    // =========================================================================
    // DE-REVERB (optional)
    // =========================================================================
    if effective_config.dereverb > 0 {
        report("analyzing_reverb", 3)?;
        let mono_for_reverb = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let spectrum = analysis::SpectralAnalysis::new(&mono_for_reverb, sample_rate);
        let cepstral = analysis::CepstralAnalysis::from_spectrum(&spectrum);
        let reverb_analysis = analysis::ReverbAnalysis::from_analyses(&spectrum, &cepstral);

        report("dereverb", 4)?;
        let mut dereverb = poddyclip::dereverb::DeReverbProcessor::new_with_preset(
            sample_rate,
            effective_config.dereverb,
        )
        .ok_or_else(|| anyhow::anyhow!("Invalid dereverb preset"))?;
        dereverb.init_with_analysis(&reverb_analysis);

        if is_stereo {
            let mut left_dereverb = dereverb.clone();
            samples[0] = dereverb.process(&samples[0]);
            samples[1] = left_dereverb.process(&samples[1]);
        } else {
            samples[0] = dereverb.process(&samples[0]);
        }
    }

    // =========================================================================
    // DENOISE
    // =========================================================================
    report("analyzing_noise", 5)?;
    let preset: usize = effective_config.denoiser_preset.into();
    let result = analyze_audio(&samples[0], sample_rate);

    report("denoise", 6)?;
    if is_stereo {
        let mut left_denoiser =
            RealtimeDenoiser::new_with_preset(sample_rate, preset).ok_or_else(|| anyhow::anyhow!("Invalid preset"))?;
        let mut right_denoiser =
            RealtimeDenoiser::new_with_preset(sample_rate, preset).ok_or_else(|| anyhow::anyhow!("Invalid preset"))?;
        left_denoiser.init_with_noise_floor(&result.noise_floor);
        right_denoiser.init_with_noise_floor(&result.noise_floor);
        samples[0] = left_denoiser.process(&samples[0]);
        samples[1] = right_denoiser.process(&samples[1]);
    } else {
        let mut denoiser =
            RealtimeDenoiser::new_with_preset(sample_rate, preset).ok_or_else(|| anyhow::anyhow!("Invalid preset"))?;
        denoiser.init_with_noise_floor(&result.noise_floor);
        samples[0] = denoiser.process(&samples[0]);
    }

    // =========================================================================
    // AI DENOISE (DeepFilterNet) - runs after spectral subtraction
    // =========================================================================
    debug!("AI Denoise enabled: {}", effective_config.ai_denoise);

    #[cfg(feature = "deepfilter")]
    if effective_config.ai_denoise {
        report("ai_denoise", 7)?;

        match DeepFilterDenoiser::new(sample_rate) {
            Ok(mut denoiser) => {
                // Analyze for auto-tuning (on already denoised audio)
                let analysis = analyze_for_deepfilter(&samples[0], sample_rate);
                info!(
                    "AI Denoise: SNR {:.1}dB ({})",
                    analysis.estimated_snr,
                    analysis.noise_severity()
                );

                // Process each channel
                if is_stereo {
                    samples[0] = denoiser.process_with_analysis(&samples[0], &analysis);
                    denoiser.reset();
                    let analysis_r = analyze_for_deepfilter(&samples[1], sample_rate);
                    samples[1] = denoiser.process_with_analysis(&samples[1], &analysis_r);
                } else {
                    samples[0] = denoiser.process_with_analysis(&samples[0], &analysis);
                }
            }
            Err(e) => {
                warn!("AI Denoise unavailable: {}", e);
            }
        }
    }

    #[cfg(not(feature = "deepfilter"))]
    if effective_config.ai_denoise {
        warn!("AI Denoise requested but deepfilter feature not enabled");
    }

    // =========================================================================
    // SPECTRAL GATE (optional)
    // =========================================================================
    report("spectral_gate", 8)?;

    if effective_config.spectral_gate > 0 {
        if is_stereo {
            let mut left_gate =
                SpectralGate::new_with_preset(sample_rate, effective_config.spectral_gate)
                    .ok_or_else(|| anyhow::anyhow!("Invalid preset"))?;
            let mut right_gate =
                SpectralGate::new_with_preset(sample_rate, effective_config.spectral_gate)
                    .ok_or_else(|| anyhow::anyhow!("Invalid preset"))?;
            left_gate.init_noise_floor(&result.noise_floor);
            right_gate.init_noise_floor(&result.noise_floor);
            samples[0] = left_gate.process(&samples[0]);
            samples[1] = right_gate.process(&samples[1]);
        } else {
            let mut gate =
                SpectralGate::new_with_preset(sample_rate, effective_config.spectral_gate)
                    .ok_or_else(|| anyhow::anyhow!("Invalid preset"))?;
            gate.init_noise_floor(&result.noise_floor);
            samples[0] = gate.process(&samples[0]);
        }
    }

    // =========================================================================
    // PEAK ATTENUATION (optional)
    // =========================================================================
    if effective_config.depeak {
        report("analyzing_peaks", 9)?;
        let analyze_samples = (sample_rate as usize).min(samples[0].len());
        let params = PeakAttenuatorParams::default();
        let peak_profile = detect_tonal_peaks(&samples[0][..analyze_samples], sample_rate, &params);

        if !peak_profile.peak_bins.is_empty() {
            report("peak_attenuation", 10)?;
            if is_stereo {
                let mut left_attenuator = PeakAttenuator::new(sample_rate);
                let mut right_attenuator = PeakAttenuator::new(sample_rate);
                left_attenuator.set_max_attenuation_db(effective_config.depeak_max_db);
                right_attenuator.set_max_attenuation_db(effective_config.depeak_max_db);
                left_attenuator.init_with_profile(peak_profile.clone());
                right_attenuator.init_with_profile(peak_profile);
                samples[0] = left_attenuator.process(&samples[0]);
                samples[1] = right_attenuator.process(&samples[1]);
            } else {
                let mut attenuator = PeakAttenuator::new(sample_rate);
                attenuator.set_max_attenuation_db(effective_config.depeak_max_db);
                attenuator.init_with_profile(peak_profile);
                samples[0] = attenuator.process(&samples[0]);
            }
        }
    }

    // =========================================================================
    // EXPANDER
    // =========================================================================
    report("expander", 11)?;

    if effective_config.expander_enabled {
        let mut expander = StereoExpander::new_with_preset(sample_rate as f32, effective_config.expander_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid expander preset"))?;
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            expander.process_stereo(&mut left[0], &mut right[0]);
        } else {
            expander.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // COMPRESSOR
    // =========================================================================
    report("compressor", 12)?;

    if effective_config.compressor_enabled {
        let use_fet = effective_config.compressor_type == CompressorType::Fet;
        let comp_preset = effective_config.compressor_preset;

        if use_fet {
            let mut fetcomp = StereoFetCompressor::new_with_preset(sample_rate as f32, comp_preset)
                .ok_or_else(|| anyhow::anyhow!("Invalid fetcomp preset"))?;
            if is_stereo {
                let (left, right) = samples.split_at_mut(1);
                fetcomp.process_stereo(&mut left[0], &mut right[0]);
            } else {
                fetcomp.process_mono(&mut samples[0]);
            }
        } else {
            let mut peakcomp = StereoVcaPeakComp::new_with_preset(sample_rate as f32, comp_preset)
                .ok_or_else(|| anyhow::anyhow!("Invalid peakcomp preset"))?;
            peakcomp.configure(samples);
            if is_stereo {
                let (left, right) = samples.split_at_mut(1);
                peakcomp.process_stereo(&mut left[0], &mut right[0]);
            } else {
                peakcomp.process_mono(&mut samples[0]);
            }
        }
    }

    // =========================================================================
    // FIXEQ
    // =========================================================================
    if effective_config.fixeq_enabled {
        report("analyzing_eq", 13)?;
        let mono = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let spectrum = analysis::SpectralAnalysis::new(&mono, sample_rate);

        report("fixeq", 14)?;
        let mut fixeq = FixEq::new(sample_rate as f32);
        fixeq.configure_from_spectrum(&spectrum, effective_config.fixeq_preset as usize, is_stereo);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fixeq.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fixeq.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // DE-ESSER
    // =========================================================================
    report("deesser", 15)?;

    if effective_config.deesser_enabled {
        let mut deesser = StereoDeEsser::new(sample_rate as f32);
        deesser.configure(samples);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            for (l, r) in left[0].iter_mut().zip(right[0].iter_mut()) {
                deesser.process_stereo(std::slice::from_mut(l), std::slice::from_mut(r));
            }
        } else {
            for s in samples[0].iter_mut() {
                deesser.process_mono(std::slice::from_mut(s));
            }
        }
    }

    // =========================================================================
    // SATURATION (Channel9)
    // =========================================================================
    report("saturation", 16)?;

    if effective_config.saturation_enabled {
        let sat_preset = get_saturation_preset(effective_config.saturation_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid saturation preset"))?;
        let mut channel9: Stereo<Channel9> = Stereo::new(sample_rate as f32);
        channel9.set_both(|c| c.set_drive(sat_preset.channel9_drive));
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            channel9.process_stereo(&mut left[0], &mut right[0]);
        } else {
            channel9.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // BUTTERCOMP
    // =========================================================================
    report("buttercomp", 17)?;

    if effective_config.buttercomp_enabled {
        let buttercomp_amount = get_buttercomp_preset(effective_config.buttercomp_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid buttercomp preset"))?;
        let mut compressor: Stereo<ButterComp2> = Stereo::new(sample_rate as f32);
        compressor.set_both(|c| c.set_compress(buttercomp_amount));
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            compressor.process_stereo(&mut left[0], &mut right[0]);
        } else {
            compressor.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // ENHANCEEQ (or RadioVoice)
    // =========================================================================
    if !effective_config.radio && effective_config.enhanceeq_enabled {
        report("analyzing_enhance", 18)?;
        let eq_preset_data = get_eq_preset(effective_config.enhanceeq_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid EQ preset"))?;
        let mono_for_enhance = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let enhance_spectrum = analysis::SpectralAnalysis::new(&mono_for_enhance, sample_rate);

        report("enhanceeq", 19)?;
        let mut enhanceeq_proc = StereoEnhanceEq::new(sample_rate as f32);
        enhanceeq_proc.configure_from_spectrum(&enhance_spectrum);

        let base_lowmid = enhanceeq_proc.get_lowmid_gain();
        let base_presence = enhanceeq_proc.get_presence_gain();
        let base_air = enhanceeq_proc.get_shelf_gain();
        let scale = eq_preset_data.lowmid_cut_db / -3.0;
        enhanceeq_proc.set_lowmid_gain(base_lowmid * scale);
        enhanceeq_proc.set_presence_gain(base_presence * scale);
        enhanceeq_proc.set_shelf_gain(base_air * scale);

        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            enhanceeq_proc.process_stereo(&mut left[0], &mut right[0]);
        } else {
            enhanceeq_proc.process_mono(&mut samples[0]);
        }
    }

    // =========================================================================
    // TAPE
    // =========================================================================
    report("tape", 20)?;

    if effective_config.tape_enabled {
        let sat_preset = get_saturation_preset(effective_config.tape_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid tape preset"))?;
        let mut tape_left = TapeGlue::new(sample_rate as f64);
        let mut tape_right = TapeGlue::new(sample_rate as f64);
        tape_left.set_warmth(sat_preset.tape_warmth);
        tape_right.set_warmth(sat_preset.tape_warmth);
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
    }

    // =========================================================================
    // RADIO VOICE EQ (optional)
    // =========================================================================
    if effective_config.radio {
        report("radio", 21)?;
        let mut radio = RadioVoiceProcessor::new(sample_rate);
        radio.set_amount(effective_config.radio_amount);

        let mono = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        radio.analyze(&mono);

        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            radio.process(&mut left[0]);
            radio.reset();
            radio.process(&mut right[0]);
        } else {
            radio.process(&mut samples[0]);
        }
    }

    // =========================================================================
    // OUTPUT STAGE
    // =========================================================================
    if effective_config.output_enabled {
        report("analyzing_levels", 22)?;
        let lufs = measure_integrated_lufs(samples, sample_rate);
        let lufs_gain_db = effective_config.lufs_target - lufs;
        apply_gain(samples, lufs_gain_db);

        report("output", 23)?;
        let mut limiter = Limiter::new(-1.0, 5.0, 100.0, sample_rate as f32);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            limiter.process_stereo(&mut left[0], &mut right[0]);
        } else {
            limiter.process_mono(&mut samples[0]);
        }
    }

    Ok(())
}

/// Create effective ProcessConfig from chain preset, keeping some overrides from request
fn config_from_chain(chain: &ChainPreset, request: &ProcessConfig) -> ProcessConfig {
    ProcessConfig {
        // Keep output format and bitrate from request
        output_format: request.output_format,
        mp3_bitrate: request.mp3_bitrate,
        // Chain name (already resolved)
        chain: None,
        // Values from chain preset
        denoiser_preset: chain.denoiser,
        dereverb: chain.dereverb,
        spectral_gate: chain.spectral_gate,
        depeak: chain.depeak,
        depeak_max_db: 18.0,
        filters_enabled: true,
        hp_slope: 24,
        hp_cutoff: 90.0, // Voice default
        declick: false, // Always off for API (offline only)
        expander_enabled: chain.expander.is_enabled(),
        expander_preset: chain.expander.preset().unwrap_or(3),
        compressor_enabled: chain.compressor.enabled,
        compressor_type: match chain.compressor.comp_type {
            crate::processing::chain::CompressorType::Peak => CompressorType::Peak,
            crate::processing::chain::CompressorType::Fet => CompressorType::Fet,
        },
        compressor_preset: chain.compressor.preset,
        fixeq_enabled: chain.fixeq.is_enabled(),
        fixeq_preset: chain.fixeq.preset().unwrap_or(3),
        deesser_enabled: chain.deesser,
        saturation_enabled: chain.saturation.is_enabled(),
        saturation_preset: chain.saturation.preset().unwrap_or(3),
        buttercomp_enabled: chain.buttercomp.is_enabled(),
        buttercomp_preset: chain.buttercomp.preset().unwrap_or(3),
        enhanceeq_enabled: chain.enhanceeq.is_enabled(),
        enhanceeq_preset: chain.enhanceeq.preset().unwrap_or(3),
        tape_enabled: chain.tape.is_enabled(),
        tape_preset: chain.tape.preset().unwrap_or(3),
        output_enabled: chain.output.is_enabled(),
        lufs_target: chain.output.lufs_target().unwrap_or(-16.0),
        radio: chain.radio,
        radio_amount: chain.radio_amount,
        ai_denoise: false, // Chain presets don't include AI denoise by default
    }
}

/// Get total number of processing stages
pub fn get_total_stages() -> u8 {
    STAGES.len() as u8
}
