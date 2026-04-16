//! Audio processing engine - adapts CLI processing chain for API use

use anyhow::Result;
use thiserror::Error;
use tracing::{debug, info, warn};

#[cfg(feature = "deepfilter")]
use poddyclip::deepfilter::{analyze_for_deepfilter, DeepFilterDenoiser};

use poddyclip::analysis;
use poddyclip::analysis::lufs::measure_integrated_lufs;
use poddyclip::dynamics::autogain::{analyze_gain, apply_gain, DEFAULT_TARGET_PEAK_DB, DEFAULT_TARGET_RMS_DB};
use poddyclip::dynamics::limiter::Limiter;
use poddyclip::dynamics::{StereoFetCompressor, StereoVcaPeakComp};
use poddyclip::eq::deesser::StereoDeEsser;
use poddyclip::eq::enhanceeq::StereoEnhanceEq;
use poddyclip::eq::filters::{FilterChain, HighPassSlope};
use poddyclip::eq::fixeq::FixEq;
use poddyclip::eq::get_eq_preset;
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
    "denoise",            // 3
    "peakcomp",           // 4
    "analyzing_eq",       // 5
    "fixeq",              // 6
    "deesser",            // 7
    "analyzing_enhance",  // 8
    "enhanceeq",          // 9
    "fetcomp",            // 10
    "analyzing_levels",   // 11
    "output",             // 12
];

/// Run the full processing chain on audio samples
pub fn process_audio(
    samples: &mut Vec<Vec<f32>>,
    sample_rate: u32,
    config: &ProcessConfig,
    on_progress: Option<ProgressCallback>,
) -> Result<()> {
    let is_stereo = samples.len() >= 2;

    let report = |stage: &str, index: u8| -> Result<()> {
        if let Some(ref cb) = on_progress {
            cb(stage, index).map_err(|e| anyhow::anyhow!(e))?;
        }
        Ok(())
    };

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
        config.output_enabled, config.lufs_target,
    );

    // =========================================================================
    // FILTERS
    // =========================================================================
    report("filters", 1)?;

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
    report("input_gain", 2)?;

    let gain_db = analyze_gain(samples, DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB);
    debug!("Input gain: {:.1} dB", gain_db);
    apply_gain(samples, gain_db);

    // =========================================================================
    // DENOISE (DeepFilterNet)
    // =========================================================================
    report("denoise", 3)?;

    #[cfg(feature = "deepfilter")]
    {
        let analysis = analyze_for_deepfilter(&samples[0], sample_rate);
        info!(
            "Denoise: SNR {:.1}dB ({})",
            analysis.estimated_snr,
            analysis.noise_severity()
        );

        match DeepFilterDenoiser::new_with_analysis(sample_rate, &analysis) {
            Ok(mut denoiser) => {
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
                warn!("Denoise unavailable: {}", e);
            }
        }
    }

    #[cfg(not(feature = "deepfilter"))]
    {
        warn!("Denoise skipped: deepfilter feature not enabled");
    }

    // =========================================================================
    // PEAK COMPRESSOR
    // =========================================================================
    report("peakcomp", 4)?;

    if config.peakcomp_enabled {
        let mut peakcomp = StereoVcaPeakComp::new_with_preset(sample_rate as f32, config.peakcomp_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid peakcomp preset"))?;
        peakcomp.configure(samples);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            peakcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            peakcomp.process_mono(&mut samples[0]);
        }
        debug!("PeakComp: preset={} gr={:.1}dB", config.peakcomp_preset, peakcomp.get_gain_reduction_db());
    } else {
        debug!("PeakComp: skipped");
    }

    // =========================================================================
    // FIXEQ
    // =========================================================================
    if config.fixeq_enabled {
        report("analyzing_eq", 5)?;
        let mono = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let spectrum = analysis::SpectralAnalysis::new(&mono, sample_rate);

        report("fixeq", 6)?;
        let mut fixeq = FixEq::new(sample_rate as f32);
        fixeq.configure_from_spectrum(&spectrum, config.fixeq_preset as usize, is_stereo);
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fixeq.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fixeq.process_mono(&mut samples[0]);
        }
        debug!("FixEQ: preset={} demud={:.1}dB corrA={:.1}dB corrB={:.1}dB",
            config.fixeq_preset,
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
    report("deesser", 7)?;

    if config.deesser_enabled {
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
    // ENHANCE EQ
    // =========================================================================
    if config.enhanceeq_enabled {
        report("analyzing_enhance", 8)?;
        let eq_preset_data = get_eq_preset(config.enhanceeq_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid EQ preset"))?;
        let mono_for_enhance = if is_stereo {
            analysis::utils::mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };
        let enhance_spectrum = analysis::SpectralAnalysis::new(&mono_for_enhance, sample_rate);

        report("enhanceeq", 9)?;
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
            config.enhanceeq_preset,
            base_lowmid * scale, base_presence * scale, base_air * scale,
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
    report("fetcomp", 10)?;

    if config.fetcomp_enabled {
        let mut fetcomp = StereoFetCompressor::new_with_preset(sample_rate as f32, config.fetcomp_preset)
            .ok_or_else(|| anyhow::anyhow!("Invalid fetcomp preset"))?;
        if is_stereo {
            let (left, right) = samples.split_at_mut(1);
            fetcomp.process_stereo(&mut left[0], &mut right[0]);
        } else {
            fetcomp.process_mono(&mut samples[0]);
        }
        debug!("FetComp: preset={} gr={:.1}dB", config.fetcomp_preset, fetcomp.get_gain_reduction_db());
    } else {
        debug!("FetComp: skipped");
    }

    // =========================================================================
    // OUTPUT STAGE
    // =========================================================================
    if config.output_enabled {
        report("analyzing_levels", 11)?;
        let lufs = measure_integrated_lufs(samples, sample_rate);
        let lufs_gain_db = config.lufs_target - lufs;
        debug!("LUFS: measured={:.1} target={:.1} gain={:.1} dB", lufs, config.lufs_target, lufs_gain_db);
        apply_gain(samples, lufs_gain_db);

        report("output", 12)?;
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
