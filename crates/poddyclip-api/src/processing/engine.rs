//! Audio processing engine - adapts CLI processing chain for API use

#[cfg(feature = "mossformer2")]
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use thiserror::Error;
use tracing::{debug, info, warn};

#[cfg(feature = "mossformer2")]
use poddyclip::ai_clean::AiCleanProcessor;
#[cfg(feature = "mossformer2")]
use poddyclip::sample_rate::{resample_mono, resample_mono_if_needed};

use poddyclip::analysis;
use poddyclip::analysis::lufs::measure_integrated_lufs;
use poddyclip::dynamics::autogain::{
    analyze_gain, apply_gain, DEFAULT_TARGET_PEAK_DB, DEFAULT_TARGET_RMS_DB,
};
use poddyclip::dynamics::limiter::Limiter;
use poddyclip::dynamics::{StereoFetCompressor, StereoVcaPeakComp};
use poddyclip::eq::deesser::StereoDeEsser;
use poddyclip::eq::enhanceeq::StereoEnhanceEq;
use poddyclip::eq::filters::{FilterChain, HighPassSlope};
use poddyclip::eq::fixeq::FixEq;
use poddyclip::eq::get_eq_preset;
use poddyclip::traits::{Stereo, StereoProcessor};

use crate::models::ProcessConfig;
#[cfg(feature = "mossformer2")]
use crate::state::AiCleanRuntimePool;

/// Error returned when a job is cancelled
#[derive(Debug, Error)]
#[error("Job was cancelled")]
pub struct CancelledError;

#[derive(Debug, Clone, Copy)]
pub struct ProgressUpdate {
    pub stage: &'static str,
    pub stage_index: u8,
    pub stage_progress: f32,
}

impl ProgressUpdate {
    pub fn new(stage: &'static str, stage_index: u8, stage_progress: f32) -> Self {
        Self {
            stage,
            stage_index,
            stage_progress,
        }
    }
}

/// Progress callback type - returns Ok(()) to continue, Err to cancel.
pub type ProgressCallback = Box<dyn FnMut(ProgressUpdate) -> Result<(), CancelledError> + Send>;

fn report_progress(
    on_progress: &mut Option<ProgressCallback>,
    stage: &'static str,
    index: u8,
) -> Result<()> {
    if let Some(ref mut cb) = on_progress {
        cb(ProgressUpdate::new(stage, index, 0.0)).map_err(|e| anyhow::anyhow!(e))?;
    }
    Ok(())
}

/// Processing stages for progress tracking
const STAGES: &[&str] = &[
    "decoding",          // 0
    "filters",           // 1
    "input_gain",        // 2
    "denoise",           // 3
    "peakcomp",          // 4
    "analyzing_eq",      // 5
    "fixeq",             // 6
    "deesser",           // 7
    "analyzing_enhance", // 8
    "enhanceeq",         // 9
    "fetcomp",           // 10
    "analyzing_levels",  // 11
    "output",            // 12
];

/// Run the full processing chain on audio samples
pub fn process_audio(
    samples: &mut Vec<Vec<f32>>,
    sample_rate: u32,
    config: &ProcessConfig,
    #[cfg(feature = "mossformer2")] ai_clean_runtime_pool: Option<Arc<AiCleanRuntimePool>>,
    mut on_progress: Option<ProgressCallback>,
) -> Result<()> {
    let processing_start = Instant::now();
    let is_stereo = samples.len() >= 2;

    let duration_secs = samples[0].len() as f32 / sample_rate as f32;
    info!(
        "Processing {:.1}s {} audio @ {}Hz | declick={} \
         peakcomp={} fetcomp={} fixeq={} deesser={} enhance={} output={}/{:.0}",
        duration_secs,
        if is_stereo { "stereo" } else { "mono" },
        sample_rate,
        config.declick,
        config.peakcomp_preset,
        config.fetcomp_preset,
        config.fixeq_preset,
        config.deesser_enabled,
        config.enhanceeq_preset,
        config.output_enabled,
        config.lufs_target,
    );

    // =========================================================================
    // FILTERS
    // =========================================================================
    report_progress(&mut on_progress, "filters", 1)?;

    if config.filters_enabled {
        let slope = if config.hp_slope == 24 {
            HighPassSlope::Slope24dB
        } else {
            HighPassSlope::Slope12dB
        };
        let cutoff = config.hp_cutoff;
        debug!("Filters: HP {}dB/oct @ {:.0}Hz", config.hp_slope, cutoff);

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
    report_progress(&mut on_progress, "input_gain", 2)?;

    let gain_db = analyze_gain(
        samples,
        sample_rate,
        DEFAULT_TARGET_RMS_DB,
        DEFAULT_TARGET_PEAK_DB,
    );
    debug!("Input gain: {:.1} dB", gain_db);
    apply_gain(samples, gain_db);

    // =========================================================================
    // DENOISE (AI Clean)
    // =========================================================================
    report_progress(&mut on_progress, "denoise", 3)?;

    #[cfg(feature = "mossformer2")]
    {
        match ai_clean_runtime_pool {
            Some(pool) => {
                let denoise_start = Instant::now();
                let acquire_start = Instant::now();
                let lease = pool
                    .acquire_blocking()
                    .map_err(|e| anyhow::anyhow!("AI clean runtime pool error: {}", e))?;
                let wait_ms = acquire_start.elapsed().as_millis();
                let denoiser = AiCleanProcessor::from_runtime(lease.runtime());
                let model_sample_rate = denoiser.model_sample_rate();
                let left_input =
                    resample_mono_if_needed(&samples[0], sample_rate, denoiser.model_sample_rate())?;
                let plan = denoiser.analyze_audio_run(&left_input);
                info!(
                    "AI clean: MossFormer2 mode={:?} segments={} input_sr={} model_sr={} pool_size={} wait_ms={}",
                    plan.mode,
                    plan.segment_count,
                    sample_rate,
                    model_sample_rate,
                    pool.size(),
                    wait_ms,
                );

                if is_stereo {
                    let right_input =
                        resample_mono_if_needed(&samples[1], sample_rate, model_sample_rate)?;
                    let left_output =
                        denoiser.process_with_progress(&left_input, |ai_progress| {
                            if let Some(cb) = on_progress.as_mut() {
                                Ok(cb(ProgressUpdate::new(
                                    "denoise",
                                    3,
                                    ai_progress.fraction_complete,
                                ))?)
                            } else {
                                Ok(())
                            }
                        })?;
                    let right_output = denoiser.process(&right_input)?;
                    if sample_rate == model_sample_rate {
                        samples[0] = left_output;
                        samples[1] = right_output;
                    } else {
                        samples[0] = resample_mono(&left_output, model_sample_rate, sample_rate)?;
                        samples[1] = resample_mono(&right_output, model_sample_rate, sample_rate)?;
                    }
                } else {
                    let output = denoiser.process_with_progress(&left_input, |ai_progress| {
                        if let Some(cb) = on_progress.as_mut() {
                            Ok(cb(ProgressUpdate::new(
                                "denoise",
                                3,
                                ai_progress.fraction_complete,
                            ))?)
                        } else {
                            Ok(())
                        }
                    })?;
                    if sample_rate == model_sample_rate {
                        samples[0] = output;
                    } else {
                        samples[0] = resample_mono(&output, model_sample_rate, sample_rate)?;
                    }
                }

                info!(
                    mode = ?plan.mode,
                    segments = plan.segment_count,
                    pool_size = pool.size(),
                    wait_ms,
                    duration_ms = denoise_start.elapsed().as_millis(),
                    "AI clean completed"
                );
            }
            None => {
                warn!("AI clean unavailable: shared runtime not configured");
            }
        }
    }

    #[cfg(not(feature = "mossformer2"))]
    {
        warn!("Denoise skipped: mossformer2 feature not enabled");
    }

    // =========================================================================
    // PEAK COMPRESSOR
    // =========================================================================
    report_progress(&mut on_progress, "peakcomp", 4)?;

    if config.peakcomp_enabled {
        let mut peakcomp =
            StereoVcaPeakComp::new_with_preset(sample_rate as f32, config.peakcomp_preset)
                .ok_or_else(|| anyhow::anyhow!("Invalid peakcomp preset"))?;
        peakcomp.configure(samples);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            peakcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            peakcomp.process_mono(&mut samples[0]);
        }
        debug!(
            "PeakComp: preset={} max_gr={:.1}dB",
            config.peakcomp_preset,
            peakcomp.get_max_gain_reduction_db()
        );
    } else {
        debug!("PeakComp: skipped");
    }

    // =========================================================================
    // FIXEQ
    // =========================================================================
    if config.fixeq_enabled {
        report_progress(&mut on_progress, "analyzing_eq", 5)?;
        let mono = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let spectrum = analysis::SpectralAnalysis::new(&mono, sample_rate);

        report_progress(&mut on_progress, "fixeq", 6)?;
        let mut fixeq = FixEq::new(sample_rate as f32);
        fixeq.configure_from_spectrum(&spectrum, config.fixeq_preset as usize, is_stereo);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fixeq.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fixeq.process_mono(&mut samples[0]);
        }
        debug!(
            "FixEQ: preset={} max_demud={:.1}dB max_corrA={:.1}dB max_corrB={:.1}dB",
            config.fixeq_preset,
            fixeq.get_max_demud_gain_db(),
            fixeq.get_max_correction_a_gain_db(),
            fixeq.get_max_correction_b_gain_db(),
        );
    } else {
        debug!("FixEQ: skipped");
    }

    // =========================================================================
    // DE-ESSER
    // =========================================================================
    report_progress(&mut on_progress, "deesser", 7)?;

    if config.deesser_enabled {
        let mut deesser = StereoDeEsser::new(sample_rate as f32);
        deesser.configure(samples);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            deesser.process_stereo(&mut left[0], &mut right[0]);
        } else {
            deesser.process_mono(&mut samples[0]);
        }
        debug!("DeEsser: max_gr={:.1}dB", deesser.get_max_gain_reduction_db());
    } else {
        debug!("DeEsser: skipped");
    }

    // =========================================================================
    // ENHANCE EQ
    // =========================================================================
    if config.enhanceeq_enabled {
        report_progress(&mut on_progress, "analyzing_enhance", 8)?;
        let eq_preset_data = get_eq_preset(config.enhanceeq_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid EQ preset"))?;
        let mono_for_enhance = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let enhance_spectrum = analysis::SpectralAnalysis::new(&mono_for_enhance, sample_rate);

        report_progress(&mut on_progress, "enhanceeq", 9)?;
        let mut enhanceeq_proc = StereoEnhanceEq::new(sample_rate as f32);
        enhanceeq_proc.configure_from_spectrum(&enhance_spectrum);

        let base_lowmid = enhanceeq_proc.get_lowmid_gain();
        let base_presence = enhanceeq_proc.get_presence_gain();
        let base_air = enhanceeq_proc.get_shelf_gain();
        let scale = eq_preset_data.lowmid_cut_db / -3.0;
        enhanceeq_proc.set_lowmid_gain(base_lowmid * scale);
        enhanceeq_proc.set_presence_gain(base_presence * scale);
        enhanceeq_proc.set_shelf_gain(base_air * scale);
        debug!(
            "EnhanceEQ: preset={} lowmid={:.1}dB presence={:.1}dB air={:.1}dB",
            config.enhanceeq_preset,
            base_lowmid * scale,
            base_presence * scale,
            base_air * scale,
        );

        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            enhanceeq_proc.process_stereo(&mut left[0], &mut right[0]);
        } else {
            enhanceeq_proc.process_mono(&mut samples[0]);
        }
    } else {
        debug!("EnhanceEQ: skipped");
    }

    // =========================================================================
    // FET COMPRESSOR
    // =========================================================================
    report_progress(&mut on_progress, "fetcomp", 10)?;

    if config.fetcomp_enabled {
        let mut fetcomp =
            StereoFetCompressor::new_with_preset(sample_rate as f32, config.fetcomp_preset)
                .ok_or_else(|| anyhow::anyhow!("Invalid fetcomp preset"))?;
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fetcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fetcomp.process_mono(&mut samples[0]);
        }
        debug!(
            "FetComp: preset={} gr={:.1}dB",
            config.fetcomp_preset,
            fetcomp.get_gain_reduction_db()
        );
    } else {
        debug!("FetComp: skipped");
    }

    // =========================================================================
    // OUTPUT STAGE
    // =========================================================================
    if config.output_enabled {
        report_progress(&mut on_progress, "analyzing_levels", 11)?;
        let lufs = measure_integrated_lufs(samples, sample_rate);
        let lufs_gain_db = config.lufs_target - lufs;
        debug!(
            "LUFS: measured={:.1} target={:.1} gain={:.1} dB",
            lufs, config.lufs_target, lufs_gain_db
        );
        apply_gain(samples, lufs_gain_db);

        report_progress(&mut on_progress, "output", 12)?;
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

    info!(
        duration_ms = processing_start.elapsed().as_millis(),
        "Processing pipeline completed"
    );

    Ok(())
}

/// Get total number of processing stages
pub fn get_total_stages() -> u8 {
    STAGES.len() as u8
}
