//! Audio processing engine - adapts CLI processing chain for API use

use anyhow::Result;
use thiserror::Error;
use tracing::{debug, info, warn};

#[cfg(feature = "deepfilter")]
use poddyclip::deepfilter::{analyze_for_deepfilter, DeepFilterDenoiser};

use poddyclip::analysis;
use poddyclip::analysis::lufs::measure_integrated_lufs;
use poddyclip::denoiser::{
    analyze_audio, RealtimeDenoiser,
    SpectralGate,
};
use poddyclip::dynamics::autogain::{analyze_gain, apply_gain, DEFAULT_TARGET_PEAK_DB, DEFAULT_TARGET_RMS_DB};
use poddyclip::dynamics::limiter::Limiter;
use poddyclip::dynamics::{get_buttercomp_preset, ButterComp2, StereoFetCompressor, StereoVcaPeakComp};
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

use crate::models::ProcessConfig;

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
    "peakcomp",           // 9
    "analyzing_eq",       // 10
    "fixeq",              // 11
    "deesser",            // 12
    "saturation",         // 13
    "buttercomp",         // 14
    "analyzing_enhance",  // 15
    "enhanceeq",          // 16
    "radio",              // 17
    "fetcomp",            // 18
    "tape",               // 19
    "analyzing_levels",   // 20
    "output",             // 21
];

/// Run the full processing chain on audio samples
pub fn process_audio(
    samples: &mut Vec<Vec<f32>>,
    sample_rate: u32,
    config: &ProcessConfig,
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

    let effective_config = config.clone();

    let duration_secs = samples[0].len() as f32 / sample_rate as f32;
    info!(
        "Processing {:.1}s {} audio @ {}Hz | denoise={} dereverb={} gate={} ai={} \
         peakcomp={}/{} fetcomp={}/{} fixeq={}/{} deesser={}sat={}/{} butter={}/{} \
         enhance={}/{} tape={}/{} radio={}/{:.1} output={}/{:.0}",
        duration_secs,
        if is_stereo { "stereo" } else { "mono" },
        sample_rate,
        effective_config.denoiser_preset,
        effective_config.dereverb,
        effective_config.spectral_gate,
        effective_config.ai_denoise,
        effective_config.peakcomp_enabled, effective_config.peakcomp_preset,
        effective_config.fetcomp_enabled, effective_config.fetcomp_preset,
        effective_config.fixeq_enabled, effective_config.fixeq_preset,
        effective_config.deesser_enabled,
        effective_config.saturation_enabled, effective_config.saturation_preset,
        effective_config.buttercomp_enabled, effective_config.buttercomp_preset,
        effective_config.enhanceeq_enabled, effective_config.enhanceeq_preset,
        effective_config.tape_enabled, effective_config.tape_preset,
        effective_config.radio, effective_config.radio_amount,
        effective_config.output_enabled, effective_config.lufs_target,
    );

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
        debug!("Filters: HP {}dB/oct @ {:.0}Hz", effective_config.hp_slope, cutoff);

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
    } else {
        debug!("Filters: skipped");
    }

    // =========================================================================
    // INPUT GAIN
    // =========================================================================
    report("input_gain", 2)?;

    let gain_db = analyze_gain(samples, DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB);
    debug!("Input gain: {:.1} dB", gain_db);
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
        debug!(
            "Reverb analysis: RT60={:.0}ms DRR={:.1}dB",
            reverb_analysis.rt60_avg_ms, reverb_analysis.drr_db
        );

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
        debug!("DeReverb: preset={} max_gr={:.1}dB", effective_config.dereverb, dereverb.get_max_gain_reduction_db());
    } else {
        debug!("DeReverb: skipped");
    }

    // =========================================================================
    // DENOISE
    // =========================================================================
    report("analyzing_noise", 5)?;
    let preset: usize = effective_config.denoiser_preset.into();
    let result = analyze_audio(&samples[0], sample_rate);

    report("denoise", 6)?;
    debug!("Denoise: preset={}", preset);
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
    #[cfg(feature = "deepfilter")]
    if effective_config.ai_denoise {
        report("ai_denoise", 7)?;

        // Analyze first to build model with correct params
        let analysis = analyze_for_deepfilter(&samples[0], sample_rate);
        info!(
            "AI Denoise: SNR {:.1}dB ({})",
            analysis.estimated_snr,
            analysis.noise_severity()
        );

        match DeepFilterDenoiser::new_with_analysis(sample_rate, &analysis) {
            Ok(mut denoiser) => {
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
        debug!("SpectralGate: preset={}", effective_config.spectral_gate);
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
            debug!("SpectralGate: L max_gr={:.1}dB R max_gr={:.1}dB", left_gate.get_max_gain_reduction_db(), right_gate.get_max_gain_reduction_db());
        } else {
            let mut gate =
                SpectralGate::new_with_preset(sample_rate, effective_config.spectral_gate)
                    .ok_or_else(|| anyhow::anyhow!("Invalid preset"))?;
            gate.init_noise_floor(&result.noise_floor);
            samples[0] = gate.process(&samples[0]);
            debug!("SpectralGate: max_gr={:.1}dB", gate.get_max_gain_reduction_db());
        }
    } else {
        debug!("SpectralGate: skipped");
    }

    // =========================================================================
    // PEAK COMPRESSOR
    // =========================================================================
    report("peakcomp", 9)?;

    if effective_config.peakcomp_enabled {
        let mut peakcomp = StereoVcaPeakComp::new_with_preset(sample_rate as f32, effective_config.peakcomp_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid peakcomp preset"))?;
        peakcomp.configure(samples);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            peakcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            peakcomp.process_mono(&mut samples[0]);
        }
        debug!("PeakComp: preset={} gr={:.1}dB", effective_config.peakcomp_preset, peakcomp.get_gain_reduction_db());
    } else {
        debug!("PeakComp: skipped");
    }

    // =========================================================================
    // FIXEQ
    // =========================================================================
    if effective_config.fixeq_enabled {
        report("analyzing_eq", 10)?;
        let mono = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let spectrum = analysis::SpectralAnalysis::new(&mono, sample_rate);

        report("fixeq", 11)?;
        let mut fixeq = FixEq::new(sample_rate as f32);
        fixeq.configure_from_spectrum(&spectrum, effective_config.fixeq_preset as usize, is_stereo);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fixeq.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fixeq.process_mono(&mut samples[0]);
        }
        debug!("FixEQ: preset={} demud={:.1}dB corrA={:.1}dB corrB={:.1}dB",
            effective_config.fixeq_preset,
            fixeq.get_demud_gain_db(),
            fixeq.get_correction_a_gain_db(),
            fixeq.get_correction_b_gain_db(),
        );
    } else {
        debug!("FixEQ: skipped");
    }

    // =========================================================================
    // DE-ESSER
    // =========================================================================
    report("deesser", 12)?;

    if effective_config.deesser_enabled {
        let mut deesser = StereoDeEsser::new(sample_rate as f32);
        deesser.configure(samples);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            deesser.process_stereo(&mut left[0], &mut right[0]);
        } else {
            deesser.process_mono(&mut samples[0]);
        }
        debug!("DeEsser: gr={:.1}dB", deesser.get_gain_reduction_db());
    } else {
        debug!("DeEsser: skipped");
    }

    // =========================================================================
    // SATURATION (Channel9)
    // =========================================================================
    report("saturation", 13)?;

    if effective_config.saturation_enabled {
        let sat_preset = get_saturation_preset(effective_config.saturation_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid saturation preset"))?;
        debug!("Channel9: preset={} drive={:.2}", effective_config.saturation_preset, sat_preset.channel9_drive);
        let mut channel9: Stereo<Channel9> = Stereo::new(sample_rate as f32);
        channel9.set_both(|c| c.set_drive(sat_preset.channel9_drive));
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            channel9.process_stereo(&mut left[0], &mut right[0]);
        } else {
            channel9.process_mono(&mut samples[0]);
        }
    } else {
        debug!("Channel9: skipped");
    }

    // =========================================================================
    // BUTTERCOMP
    // =========================================================================
    report("buttercomp", 14)?;

    if effective_config.buttercomp_enabled {
        let buttercomp_amount = get_buttercomp_preset(effective_config.buttercomp_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid buttercomp preset"))?;
        debug!("ButterComp: preset={} amount={:.2}", effective_config.buttercomp_preset, buttercomp_amount);
        let mut compressor: Stereo<ButterComp2> = Stereo::new(sample_rate as f32);
        compressor.set_both(|c| c.set_compress(buttercomp_amount));
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            compressor.process_stereo(&mut left[0], &mut right[0]);
        } else {
            compressor.process_mono(&mut samples[0]);
        }
    } else {
        debug!("ButterComp: skipped");
    }

    // =========================================================================
    // ENHANCE EQ (or RadioVoice)
    // =========================================================================
    if !effective_config.radio && effective_config.enhanceeq_enabled {
        report("analyzing_enhance", 15)?;
        let eq_preset_data = get_eq_preset(effective_config.enhanceeq_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid EQ preset"))?;
        let mono_for_enhance = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let enhance_spectrum = analysis::SpectralAnalysis::new(&mono_for_enhance, sample_rate);

        report("enhanceeq", 16)?;
        let mut enhanceeq_proc = StereoEnhanceEq::new(sample_rate as f32);
        enhanceeq_proc.configure_from_spectrum(&enhance_spectrum);

        let base_lowmid = enhanceeq_proc.get_lowmid_gain();
        let base_presence = enhanceeq_proc.get_presence_gain();
        let base_air = enhanceeq_proc.get_shelf_gain();
        let scale = eq_preset_data.lowmid_cut_db / -3.0;
        enhanceeq_proc.set_lowmid_gain(base_lowmid * scale);
        enhanceeq_proc.set_presence_gain(base_presence * scale);
        enhanceeq_proc.set_shelf_gain(base_air * scale);
        debug!("EnhanceEQ: preset={} lowmid={:.1}dB presence={:.1}dB air={:.1}dB",
            effective_config.enhanceeq_preset,
            base_lowmid * scale, base_presence * scale, base_air * scale,
        );

        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            enhanceeq_proc.process_stereo(&mut left[0], &mut right[0]);
        } else {
            enhanceeq_proc.process_mono(&mut samples[0]);
        }
    } else if !effective_config.radio {
        debug!("EnhanceEQ: skipped");
    }

    // =========================================================================
    // RADIO VOICE EQ (optional)
    // =========================================================================
    if effective_config.radio {
        report("radio", 17)?;
        let mut radio = RadioVoiceProcessor::new(sample_rate);
        radio.set_amount(effective_config.radio_amount);

        let mono = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        radio.analyze(&mono);
        debug!("RadioVoice: amount={:.2} low={:.1}dB mud={:.1}dB presence={:.1}dB air={:.1}dB",
            effective_config.radio_amount,
            radio.get_low_shelf_gain(), radio.get_mud_gain(),
            radio.get_presence_gain(), radio.get_air_gain(),
        );

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
    // FET COMPRESSOR (final glue after EQ)
    // =========================================================================
    report("fetcomp", 18)?;

    if effective_config.fetcomp_enabled {
        let mut fetcomp = StereoFetCompressor::new_with_preset(sample_rate as f32, effective_config.fetcomp_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid fetcomp preset"))?;
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fetcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fetcomp.process_mono(&mut samples[0]);
        }
        debug!("FetComp: preset={} gr={:.1}dB", effective_config.fetcomp_preset, fetcomp.get_gain_reduction_db());
    } else {
        debug!("FetComp: skipped");
    }

    // =========================================================================
    // TAPE
    // =========================================================================
    report("tape", 19)?;

    if effective_config.tape_enabled {
        let sat_preset = get_saturation_preset(effective_config.tape_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid tape preset"))?;
        debug!("Tape: preset={} warmth={:.2}", effective_config.tape_preset, sat_preset.tape_warmth);
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
    } else {
        debug!("Tape: skipped");
    }

    // =========================================================================
    // OUTPUT STAGE
    // =========================================================================
    if effective_config.output_enabled {
        report("analyzing_levels", 20)?;
        let lufs = measure_integrated_lufs(samples, sample_rate);
        let lufs_gain_db = effective_config.lufs_target - lufs;
        debug!("LUFS: measured={:.1} target={:.1} gain={:.1} dB", lufs, effective_config.lufs_target, lufs_gain_db);
        apply_gain(samples, lufs_gain_db);

        report("output", 21)?;
        let mut limiter = Limiter::new(-1.0, 5.0, 100.0, sample_rate as f32);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            limiter.process_stereo(&mut left[0], &mut right[0]);
        } else {
            limiter.process_mono(&mut samples[0]);
        }
        debug!("Limiter: ceiling=-1.0dB");
    } else {
        debug!("Output: skipped");
    }

    Ok(())
}

/// Get total number of processing stages
pub fn get_total_stages() -> u8 {
    STAGES.len() as u8
}
