//! Poddyclip VST3/CLAP plugin

mod editor;
mod params;
mod visualizations;

use nih_plug::prelude::*;
use std::sync::{Arc, Mutex};

use poddyclip::denoiser::{StreamingDenoiser, VisualizationData};
use poddyclip::dynamics::{ButterComp2, StereoFetCompressor, StereoRealtimeLimiter, StereoVcaPeakComp};
use poddyclip::eq::{DeEsser, FilterChain, FixEq, HighPassSlope, StereoEnhanceEq};
use poddyclip::saturation::{Channel9, TapeGlue};
use poddyclip::traits::Stereo;

pub use params::PoddyclipParams;

// =============================================================================
// Plugin State
// =============================================================================

pub struct Poddyclip {
    params: Arc<PoddyclipParams>,

    // Filters (applied before denoising)
    filter: Stereo<FilterChain>,
    prev_hp_slope: HighPassSlope,

    // Streaming denoiser
    denoiser: Stereo<StreamingDenoiser>,
    sample_rate: f32,

    // Track reset button state
    prev_reset_state: bool,

    // Visualization data shared with GUI
    visualization_data: Arc<Mutex<VisualizationData>>,

    // FixEq (post-denoiser dynamic EQ)
    fixeq: FixEq,

    // Gain reduction for UI meters
    demud_gain_db: Arc<Mutex<f32>>,
    correction_a_gain_db: Arc<Mutex<f32>>,
    correction_b_gain_db: Arc<Mutex<f32>>,

    // De-Esser
    deesser: Stereo<DeEsser>,
    deesser_gain_db: Arc<Mutex<f32>>,

    // FET Compressor (1176-style)
    fetcomp: StereoFetCompressor,
    fetcomp_gain_db: Arc<Mutex<f32>>,

    // VCA Peak Compressor
    peakcomp: StereoVcaPeakComp,
    peakcomp_gain_db: Arc<Mutex<f32>>,

    // Channel9 (Neve transformer)
    channel9: Stereo<Channel9>,

    // Enhance EQ
    enhance_eq: StereoEnhanceEq,

    // ButterComp2
    buttercomp: Stereo<ButterComp2>,

    // TapeGlue
    tape_glue: Stereo<TapeGlue>,

    // Limiter
    limiter: StereoRealtimeLimiter,
    limiter_gain_db: Arc<Mutex<f32>>,
}

impl Default for Poddyclip {
    fn default() -> Self {
        Self {
            params: Arc::new(PoddyclipParams::default()),
            filter: Stereo::<FilterChain>::new(48000.0),
            prev_hp_slope: HighPassSlope::Slope24dB,
            denoiser: Stereo::from_pair(
                StreamingDenoiser::new(48000),
                StreamingDenoiser::new(48000),
            ),
            sample_rate: 48000.0,
            prev_reset_state: false,
            visualization_data: Arc::new(Mutex::new(VisualizationData::default())),
            fixeq: FixEq::new(48000.0),
            demud_gain_db: Arc::new(Mutex::new(0.0)),
            correction_a_gain_db: Arc::new(Mutex::new(0.0)),
            correction_b_gain_db: Arc::new(Mutex::new(0.0)),
            deesser: Stereo::<DeEsser>::new(48000.0),
            deesser_gain_db: Arc::new(Mutex::new(0.0)),
            fetcomp: StereoFetCompressor::new_default(48000.0),
            fetcomp_gain_db: Arc::new(Mutex::new(0.0)),
            peakcomp: StereoVcaPeakComp::new(48000.0),
            peakcomp_gain_db: Arc::new(Mutex::new(0.0)),
            channel9: Stereo::<Channel9>::new(48000.0),
            enhance_eq: StereoEnhanceEq::new(48000.0),
            buttercomp: Stereo::<ButterComp2>::new(48000.0),
            tape_glue: Stereo::<TapeGlue>::new_f64(48000.0),
            limiter: StereoRealtimeLimiter::new(-1.0, 5.0, 100.0, 48000.0),
            limiter_gain_db: Arc::new(Mutex::new(0.0)),
        }
    }
}

// =============================================================================
// Plugin Trait
// =============================================================================

impl Plugin for Poddyclip {
    const NAME: &'static str = "Poddyclip";
    const VENDOR: &'static str = "Poddyclip";
    const URL: &'static str = "https://github.com/poddyclip";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create_plugin_editor(
            self.params.editor_state.clone(),
            self.params.clone(),
            self.visualization_data.clone(),
            self.demud_gain_db.clone(),
            self.correction_a_gain_db.clone(),
            self.correction_b_gain_db.clone(),
            self.deesser_gain_db.clone(),
            self.fetcomp_gain_db.clone(),
            self.peakcomp_gain_db.clone(),
            self.limiter_gain_db.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        self.denoiser = Stereo::from_pair(
            StreamingDenoiser::new(buffer_config.sample_rate as u32),
            StreamingDenoiser::new(buffer_config.sample_rate as u32),
        );

        self.fetcomp = StereoFetCompressor::new_default(buffer_config.sample_rate);
        self.limiter = StereoRealtimeLimiter::new(-1.0, 5.0, 100.0, buffer_config.sample_rate);

        true
    }

    fn reset(&mut self) {
        self.denoiser.reset();
        self.fetcomp.reset();
        self.limiter.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Check if HP slope changed
        let current_hp_slope = self.params.filters.hp_slope.value();
        if current_hp_slope != self.prev_hp_slope {
            self.filter = Stereo::from_pair(
                FilterChain::new(self.sample_rate, current_hp_slope),
                FilterChain::new(self.sample_rate, current_hp_slope),
            );
            self.prev_hp_slope = current_hp_slope;
        }

        // Check reset button
        let reset_state = self.params.reset_noise.value();
        if reset_state != self.prev_reset_state {
            self.filter.reset();
            self.denoiser.reset();
        }
        self.prev_reset_state = reset_state;

        // Update denoiser parameters
        let params = self.params.get_denoiser_params();

        let num_channels = buffer.channels();
        let num_samples = buffer.samples();

        if num_channels == 0 || num_samples == 0 {
            return ProcessStatus::Normal;
        }

        // Report latency
        context.set_latency_samples(self.denoiser.left.latency_samples() as u32);

        // Process based on channel count
        match num_channels {
            1 => {
                self.denoiser.left.set_params(params);
                self.process_mono(buffer);
            }
            2 => {
                self.denoiser.left.set_params(params.clone());
                self.denoiser.right.set_params(params);
                self.process_stereo(buffer);
            }
            _ => {}
        }

        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {}
}

// =============================================================================
// Processing
// =============================================================================

impl Poddyclip {
    fn process_mono(&mut self, buffer: &mut Buffer) {
        // Read parameters once per buffer
        let viz_enabled = self.params.editor_state.is_open();
        let filter_enabled = self.params.filters.enable.value();

        let fetcomp_enabled = self.params.fetcomp.enable.value();
        let peakcomp_enabled = self.params.peakcomp.enable.value();
        let deesser_enabled = self.params.deesser.enable.value();
        let channel9_enabled = self.params.channel9.enable.value();
        let enhance_eq_enabled = self.params.enhance_eq.enable.value();
        let buttercomp_enabled = self.params.buttercomp.enable.value();
        let tape_glue_enabled = self.params.tape_glue.enable.value();
        let limiter_enabled = self.params.limiter.enable.value();

        // Sync processor params
        self.sync_processor_params(false);
        self.denoiser.left.set_visualization_enabled(viz_enabled);

        let mut last_limiter_gr_db = 0.0;

        for mut channel_samples in buffer.iter_samples() {
            let input = channel_samples.get_mut(0).copied().unwrap_or(0.0);

            // Filter
            let filtered = if filter_enabled {
                self.filter.left.process(input)
            } else {
                input
            };

            // Denoise
            let mut out = self.denoiser.left.process_sample(filtered);

            // FET Compressor (early, for peak control + saturation)
            if fetcomp_enabled {
                let (l, _) = self.fetcomp.process_sample(out, out);
                out = l;
            }

            // Peak Compressor
            if peakcomp_enabled {
                out = self.peakcomp.left.process(out);
            }

            // FixEq
            out = self.fixeq.process(out);

            // De-Esser
            if deesser_enabled {
                out = self.deesser.left.process(out);
            }

            // Channel9
            if channel9_enabled {
                out = self.channel9.left.process(out);
            }

            // EnhanceEQ
            if enhance_eq_enabled {
                out = self.enhance_eq.left.process(out);
            }

            // ButterComp
            if buttercomp_enabled {
                out = self.buttercomp.left.process(out);
            }

            // TapeGlue
            if tape_glue_enabled {
                out = self.tape_glue.left.process(out);
            }

            // Limiter
            if limiter_enabled {
                let (l, _, gr) = self.limiter.process(out, out);
                out = l;
                last_limiter_gr_db = gr;
            }

            if let Some(sample) = channel_samples.get_mut(0) {
                *sample = out;
            }
        }

        // Update visualization
        self.update_visualization(viz_enabled, last_limiter_gr_db);
    }

    fn process_stereo(&mut self, buffer: &mut Buffer) {
        // Read parameters once per buffer
        let viz_enabled = self.params.editor_state.is_open();
        let filter_enabled = self.params.filters.enable.value();

        let fetcomp_enabled = self.params.fetcomp.enable.value();
        let peakcomp_enabled = self.params.peakcomp.enable.value();
        let deesser_enabled = self.params.deesser.enable.value();
        let channel9_enabled = self.params.channel9.enable.value();
        let enhance_eq_enabled = self.params.enhance_eq.enable.value();
        let buttercomp_enabled = self.params.buttercomp.enable.value();
        let tape_glue_enabled = self.params.tape_glue.enable.value();
        let limiter_enabled = self.params.limiter.enable.value();

        // Sync processor params
        self.sync_processor_params(true);
        self.denoiser.left.set_visualization_enabled(viz_enabled);
        self.denoiser.right.set_visualization_enabled(false);

        let mut last_limiter_gr_db = 0.0;

        for mut channel_samples in buffer.iter_samples() {
            let left_in = channel_samples.get_mut(0).copied().unwrap_or(0.0);
            let right_in = channel_samples.get_mut(1).copied().unwrap_or(0.0);

            // Filter
            let (filtered_l, filtered_r) = if filter_enabled {
                (self.filter.left.process(left_in), self.filter.right.process(right_in))
            } else {
                (left_in, right_in)
            };

            // Denoise
            let mut left_out = self.denoiser.left.process_sample(filtered_l);
            let mut right_out = self.denoiser.right.process_sample(filtered_r);

            // FET Compressor (early, for peak control + saturation)
            if fetcomp_enabled {
                (left_out, right_out) = self.fetcomp.process_sample(left_out, right_out);
            }

            // Peak Compressor
            if peakcomp_enabled {
                (left_out, right_out) = self.peakcomp.process_sample_stereo(left_out, right_out);
            }

            // FixEq
            left_out = self.fixeq.process_sample_left(left_out);
            right_out = self.fixeq.process_sample_right(right_out);

            // De-Esser
            if deesser_enabled {
                left_out = self.deesser.left.process(left_out);
                right_out = self.deesser.right.process(right_out);
            }

            // Channel9
            if channel9_enabled {
                left_out = self.channel9.left.process(left_out);
                right_out = self.channel9.right.process(right_out);
            }

            // EnhanceEQ
            if enhance_eq_enabled {
                left_out = self.enhance_eq.left.process(left_out);
                right_out = self.enhance_eq.right.process(right_out);
            }

            // ButterComp
            if buttercomp_enabled {
                left_out = self.buttercomp.left.process(left_out);
                right_out = self.buttercomp.right.process(right_out);
            }

            // TapeGlue
            if tape_glue_enabled {
                left_out = self.tape_glue.left.process(left_out);
                right_out = self.tape_glue.right.process(right_out);
            }

            // Limiter
            if limiter_enabled {
                let (l, r, gr) = self.limiter.process(left_out, right_out);
                left_out = l;
                right_out = r;
                last_limiter_gr_db = gr;
            }

            if let Some(sample) = channel_samples.get_mut(0) {
                *sample = left_out;
            }
            if let Some(sample) = channel_samples.get_mut(1) {
                *sample = right_out;
            }
        }

        // Update visualization
        self.update_visualization(viz_enabled, last_limiter_gr_db);
    }

    fn sync_processor_params(&mut self, stereo: bool) {
        // FET Compressor
        if self.params.fetcomp.enable.value() {
            self.fetcomp.set_threshold(self.params.fetcomp.threshold.value());
            self.fetcomp.set_ratio(self.params.fetcomp.ratio.value());
            self.fetcomp.set_attack(self.params.fetcomp.attack.value());
            self.fetcomp.set_release(self.params.fetcomp.release.value());
            self.fetcomp.set_input_drive(self.params.fetcomp.input_drive.value());
            self.fetcomp.set_output_drive(self.params.fetcomp.output_drive.value());
        }

        // Peak Compressor
        if self.params.peakcomp.enable.value() {
            self.peakcomp.set_threshold(self.params.peakcomp.threshold.value());
            self.peakcomp.set_ratio(self.params.peakcomp.ratio.value());
            self.peakcomp.set_attack(self.params.peakcomp.attack.value());
            self.peakcomp.set_release(self.params.peakcomp.release.value());
        }

        // FixEq
        self.fixeq.set_stereo(stereo);

        let demud_enabled = self.params.demud.enable.value();
        self.fixeq.set_demud_enabled(demud_enabled);
        if demud_enabled {
            self.fixeq.set_demud_frequency(self.params.demud.frequency.value());
            self.fixeq.set_demud_strength(self.params.demud.macro_val.value());
        } else {
            self.fixeq.set_demud_strength(0.0);
        }

        let corr_a_enabled = self.params.correction_a.enable.value();
        self.fixeq.set_correction_a_enabled(corr_a_enabled);
        if corr_a_enabled {
            self.fixeq.set_correction_a_frequency(self.params.correction_a.frequency.value());
            self.fixeq.set_correction_a_strength(self.params.correction_a.macro_val.value());
        } else {
            self.fixeq.set_correction_a_strength(0.0);
        }

        let corr_b_enabled = self.params.correction_b.enable.value();
        self.fixeq.set_correction_b_enabled(corr_b_enabled);
        if corr_b_enabled {
            self.fixeq.set_correction_b_frequency(self.params.correction_b.frequency.value());
            self.fixeq.set_correction_b_strength(self.params.correction_b.macro_val.value());
        } else {
            self.fixeq.set_correction_b_strength(0.0);
        }

        // De-Esser
        if self.params.deesser.enable.value() {
            self.deesser.set_both(|d| {
                d.set_frequency(self.params.deesser.frequency.value());
                d.set_q(self.params.deesser.q.value());
                d.set_strength(self.params.deesser.strength.value());
            });
        }

        // Channel9
        if self.params.channel9.enable.value() {
            let drive = self.params.channel9.drive.value();
            if stereo {
                self.channel9.set_both(|c| c.set_drive(drive));
            } else {
                self.channel9.left.set_drive(drive);
            }
        }

        // EnhanceEQ
        if self.params.enhance_eq.enable.value() {
            if stereo {
                self.enhance_eq.set_lowmid_freq(self.params.enhance_eq.lowmid_freq.value());
                self.enhance_eq.set_lowmid_gain(self.params.enhance_eq.lowmid_gain.value());
                self.enhance_eq.set_presence_freq(self.params.enhance_eq.presence_freq.value());
                self.enhance_eq.set_presence_gain(self.params.enhance_eq.presence_gain.value());
                self.enhance_eq.set_shelf_gain(self.params.enhance_eq.air_gain.value());
            } else {
                self.enhance_eq.left.set_lowmid_freq(self.params.enhance_eq.lowmid_freq.value());
                self.enhance_eq.left.set_lowmid_gain(self.params.enhance_eq.lowmid_gain.value());
                self.enhance_eq.left.set_presence_freq(self.params.enhance_eq.presence_freq.value());
                self.enhance_eq.left.set_presence_gain(self.params.enhance_eq.presence_gain.value());
                self.enhance_eq.left.set_shelf_gain(self.params.enhance_eq.air_gain.value());
            }
        }

        // ButterComp
        if self.params.buttercomp.enable.value() {
            let compress = self.params.buttercomp.compress.value();
            if stereo {
                self.buttercomp.set_both(|c| c.set_compress(compress));
            } else {
                self.buttercomp.left.set_compress(compress);
            }
        }

        // TapeGlue
        if self.params.tape_glue.enable.value() {
            let warmth = self.params.tape_glue.warmth.value() as f64;
            if stereo {
                self.tape_glue.set_both(|t| t.set_warmth(warmth));
            } else {
                self.tape_glue.left.set_warmth(warmth);
            }
        }
    }

    fn update_visualization(&self, viz_enabled: bool, limiter_gr_db: f32) {
        if viz_enabled {
            if let Ok(mut viz) = self.visualization_data.try_lock() {
                *viz = self.denoiser.left.get_visualization_data();
            }
            if let Ok(mut gain) = self.demud_gain_db.try_lock() {
                *gain = self.fixeq.get_demud_gain_db();
            }
            if let Ok(mut gain) = self.correction_a_gain_db.try_lock() {
                *gain = self.fixeq.get_correction_a_gain_db();
            }
            if let Ok(mut gain) = self.correction_b_gain_db.try_lock() {
                *gain = self.fixeq.get_correction_b_gain_db();
            }
            if let Ok(mut gain) = self.deesser_gain_db.try_lock() {
                *gain = self.deesser.left.get_gain_reduction_db();
            }
            if let Ok(mut gain) = self.fetcomp_gain_db.try_lock() {
                *gain = self.fetcomp.get_gain_reduction_db();
            }
            if let Ok(mut gain) = self.peakcomp_gain_db.try_lock() {
                *gain = self.peakcomp.get_gain_reduction_db();
            }
            if let Ok(mut gain) = self.limiter_gain_db.try_lock() {
                *gain = limiter_gr_db;
            }
        }
    }
}

// =============================================================================
// CLAP/VST3
// =============================================================================

impl ClapPlugin for Poddyclip {
    const CLAP_ID: &'static str = "com.poddyclip.spectral-subtraction";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Spectral subtraction noise reduction");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Poddyclip {
    const VST3_CLASS_ID: [u8; 16] = *b"PoddyclipSpecSub";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Tools];
}

nih_export_clap!(Poddyclip);
nih_export_vst3!(Poddyclip);
