#![cfg(feature = "plugin")]

mod denoiser;
mod filters;
mod visualizations;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::{Arc, Mutex};

use denoiser::{
    DenoiserParams, RealtimeDenoiser, VisualizationData, DEFAULT_ALPHA_BASE, DEFAULT_ALPHA_MAX,
    DEFAULT_ALPHA_MIN, DEFAULT_BETA, DEFAULT_DELTA, DEFAULT_GAMMA, DEFAULT_LAMBDA,
    DEFAULT_SFM_NOISE, DEFAULT_SFM_SPEECH, DEFAULT_SPIKE_THRESHOLD, HOP_SIZE, NUM_BANDS,
    PRESETS, WINDOW_SIZE,
};

use filters::{FilterChain, HighPassSlope};

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Params)]
struct FilterParams {
    #[id = "filter_enable"]
    enable: BoolParam,

    #[id = "hp_slope"]
    hp_slope: EnumParam<HighPassSlope>,
}

#[derive(Params)]
struct SubtractionParams {
    #[id = "alpha_base"]
    alpha_base: FloatParam,

    #[id = "alpha_min"]
    alpha_min: FloatParam,

    #[id = "alpha_max"]
    alpha_max: FloatParam,

    #[id = "beta"]
    beta: FloatParam,
}

#[derive(Params)]
struct NoiseEstimationParams {
    #[id = "lambda"]
    lambda: FloatParam,

    #[id = "spike_threshold"]
    spike_threshold: FloatParam,

    #[id = "sfm_speech"]
    sfm_speech: FloatParam,

    #[id = "sfm_noise"]
    sfm_noise: FloatParam,
}

#[derive(Params)]
struct PresetParams {
    /// Strength parameter (1-5, matches CLI presets with interpolation)
    #[id = "strength"]
    strength: FloatParam,
}

#[derive(Params)]
struct PoddyclipParams {
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[id = "reset_noise"]
    reset_noise: BoolParam,

    #[nested(group = "Filters")]
    filters: FilterParams,

    #[nested(group = "Subtraction")]
    subtraction: SubtractionParams,

    #[nested(group = "Noise Estimation")]
    noise_estimation: NoiseEstimationParams,

    #[nested(group = "Preset")]
    preset: PresetParams,
}

// =============================================================================
// Plugin
// =============================================================================

struct Poddyclip {
    params: Arc<PoddyclipParams>,

    // Filters (applied before denoising)
    filter_left: FilterChain,
    filter_right: FilterChain,
    prev_hp_slope: HighPassSlope,

    denoiser_left: RealtimeDenoiser,
    denoiser_right: RealtimeDenoiser,
    sample_rate: f32,

    // Buffering for frame-based processing (sliding window)
    input_ring_left: Vec<f32>,
    input_ring_right: Vec<f32>,
    output_buffer_left: Vec<f32>,
    output_buffer_right: Vec<f32>,
    samples_since_process: usize,

    // Track reset button state
    prev_reset_state: bool,

    // Visualization data shared with GUI
    visualization_data: Arc<Mutex<VisualizationData>>,
}

impl Default for Poddyclip {
    fn default() -> Self {
        Self {
            params: Arc::new(PoddyclipParams::default()),
            filter_left: FilterChain::new(48000.0, HighPassSlope::Slope24dB),
            filter_right: FilterChain::new(48000.0, HighPassSlope::Slope24dB),
            prev_hp_slope: HighPassSlope::Slope24dB,
            denoiser_left: RealtimeDenoiser::new(48000),
            denoiser_right: RealtimeDenoiser::new(48000),
            sample_rate: 48000.0,
            input_ring_left: vec![0.0; WINDOW_SIZE],
            input_ring_right: vec![0.0; WINDOW_SIZE],
            output_buffer_left: Vec::new(),
            output_buffer_right: Vec::new(),
            samples_since_process: 0,
            prev_reset_state: false,
            visualization_data: Arc::new(Mutex::new(VisualizationData::default())),
        }
    }
}

impl Default for PoddyclipParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1400, 800),

            reset_noise: BoolParam::new("Reset Noise Estimation", false).with_value_to_string(
                Arc::new(|value| {
                    if value {
                        "RESET".to_string()
                    } else {
                        "Ready".to_string()
                    }
                }),
            ),

            filters: FilterParams {
                enable: BoolParam::new("Enable Filters", true),
                hp_slope: EnumParam::new("HP Slope", HighPassSlope::Slope24dB),
            },

            subtraction: SubtractionParams {
                alpha_base: FloatParam::new(
                    "Alpha Base",
                    DEFAULT_ALPHA_BASE,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 20.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                alpha_min: FloatParam::new(
                    "Alpha Min",
                    DEFAULT_ALPHA_MIN,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 20.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                alpha_max: FloatParam::new(
                    "Alpha Max",
                    DEFAULT_ALPHA_MAX,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 20.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                beta: FloatParam::new(
                    "Beta (Floor)",
                    DEFAULT_BETA,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.001)
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
            },

            noise_estimation: NoiseEstimationParams {
                lambda: FloatParam::new(
                    "Lambda (Forget Factor)",
                    DEFAULT_LAMBDA,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 0.9999,
                    },
                )
                .with_step_size(0.0001)
                .with_value_to_string(formatters::v2s_f32_rounded(4)),

                spike_threshold: FloatParam::new(
                    "Spike Threshold",
                    DEFAULT_SPIKE_THRESHOLD,
                    FloatRange::Linear {
                        min: 0.5,
                        max: 100.0,
                    },
                )
                .with_step_size(0.5)
                .with_value_to_string(formatters::v2s_f32_rounded(1)),

                sfm_speech: FloatParam::new(
                    "SFM Speech Threshold",
                    DEFAULT_SFM_SPEECH,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                sfm_noise: FloatParam::new(
                    "SFM Noise Threshold",
                    DEFAULT_SFM_NOISE,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            },

            preset: PresetParams {
                strength: FloatParam::new(
                    "Strength",
                    3.0, // Default to preset 3 (Moderate)
                    FloatRange::Linear {
                        min: 1.0,
                        max: 5.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            },
        }
    }
}

impl PoddyclipParams {
    /// Interpolate between two presets based on strength parameter
    fn interpolate_preset_values(&self) -> ([f32; NUM_BANDS], [f32; NUM_BANDS]) {
        let strength = self.preset.strength.value();

        // Clamp to valid range
        let strength_clamped = strength.clamp(1.0, 5.0);

        // Determine which two presets to interpolate between
        // strength=1.0 → preset 0, strength=2.0 → preset 1, etc.
        let preset_idx_low = (strength_clamped - 1.0).floor() as usize;
        let preset_idx_high = (strength_clamped - 1.0).ceil() as usize;

        // Clamp indices to valid range [0, 4]
        let preset_idx_low = preset_idx_low.min(4);
        let preset_idx_high = preset_idx_high.min(4);

        // Calculate interpolation factor (0.0 to 1.0)
        let t = (strength_clamped - 1.0).fract();

        // Get the two presets
        let preset_low = &PRESETS[preset_idx_low];
        let preset_high = &PRESETS[preset_idx_high];

        // Interpolate delta and gamma arrays
        let mut delta = [0.0; NUM_BANDS];
        let mut gamma = [0.0; NUM_BANDS];

        for band in 0..NUM_BANDS {
            delta[band] = preset_low.delta[band] * (1.0 - t) + preset_high.delta[band] * t;
            gamma[band] = preset_low.gamma[band] * (1.0 - t) + preset_high.gamma[band] * t;
        }

        (delta, gamma)
    }

    fn get_delta_array(&self) -> [f32; NUM_BANDS] {
        let (delta, _) = self.interpolate_preset_values();
        delta
    }

    fn get_gamma_array(&self) -> [f32; NUM_BANDS] {
        let (_, gamma) = self.interpolate_preset_values();
        gamma
    }

    fn get_denoiser_params(&self) -> DenoiserParams {
        DenoiserParams {
            alpha_base: self.subtraction.alpha_base.value(),
            alpha_min: self.subtraction.alpha_min.value(),
            alpha_max: self.subtraction.alpha_max.value(),
            beta: self.subtraction.beta.value(),
            lambda: self.noise_estimation.lambda.value(),
            spike_threshold: self.noise_estimation.spike_threshold.value(),
            sfm_speech: self.noise_estimation.sfm_speech.value(),
            sfm_noise: self.noise_estimation.sfm_noise.value(),
            delta: self.get_delta_array(),
            gamma: self.get_gamma_array(),
        }
    }
}

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
        let params = self.params.clone();
        let viz_data = self.visualization_data.clone();

        create_egui_editor(
            params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.heading("Poddyclip - Spectral Subtraction");
                    ui.separator();

                    ui.columns(2, |columns| {
                        // LEFT COLUMN: Parameters (scrollable)
                        egui::ScrollArea::vertical().id_salt("params_scroll").show(
                            &mut columns[0],
                            |ui| {
                                // Reset button
                                ui.horizontal(|ui| {
                                    ui.label("Noise Estimation:");
                                    if ui.button("Reset Noise Floor").clicked() {
                                        setter.begin_set_parameter(&params.reset_noise);
                                        setter.set_parameter(
                                            &params.reset_noise,
                                            !params.reset_noise.value(),
                                        );
                                        setter.end_set_parameter(&params.reset_noise);
                                    }
                                });

                                ui.add_space(15.0);
                                ui.separator();

                                // Filter controls
                                ui.heading("Filters (HP 80Hz + LP 15.5kHz)");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.filters.enable,
                                        setter,
                                    ));
                                });

                                ui.horizontal(|ui| {
                                    ui.label("HP Slope:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.filters.hp_slope,
                                        setter,
                                    ));
                                });

                                ui.add_space(15.0);
                                ui.separator();

                                // Subtraction parameters
                                ui.heading("Subtraction");
                                ui.add_space(5.0);

                                ui.label("Alpha Base:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.subtraction.alpha_base,
                                    setter,
                                ));
                                ui.label("Alpha Min:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.subtraction.alpha_min,
                                    setter,
                                ));
                                ui.label("Alpha Max:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.subtraction.alpha_max,
                                    setter,
                                ));
                                ui.label("Beta (Floor):");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.subtraction.beta,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // Noise estimation parameters
                                ui.heading("Noise Estimation");
                                ui.add_space(5.0);

                                ui.label("Lambda (Forget Factor):");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.noise_estimation.lambda,
                                    setter,
                                ));
                                ui.label("Spike Threshold:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.noise_estimation.spike_threshold,
                                    setter,
                                ));
                                ui.label("SFM Speech Threshold:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.noise_estimation.sfm_speech,
                                    setter,
                                ));
                                ui.label("SFM Noise Threshold:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.noise_estimation.sfm_noise,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // Strength (replaces per-band delta/gamma)
                                ui.heading("Strength (1-5)");
                                ui.add_space(5.0);

                                ui.label("Interpolates between CLI presets:");
                                ui.label("  1 = Gentle");
                                ui.label("  2 = Light");
                                ui.label("  3 = Moderate");
                                ui.label("  4 = Strong");
                                ui.label("  5 = Aggressive");
                                ui.add_space(5.0);

                                ui.label("Strength:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.preset.strength,
                                    setter,
                                ));
                            },
                        );

                        // RIGHT COLUMN: Visualizations
                        columns[1].vertical(|ui| {
                            ui.heading("Analysis");
                            ui.add_space(10.0);

                            if let Ok(viz) = viz_data.lock() {
                                visualizations::draw_spectrum_analyzer(ui, &viz);

                                ui.add_space(15.0);
                                ui.separator();

                                visualizations::draw_gain_reduction_bars(ui, &viz);

                                ui.add_space(15.0);
                                ui.separator();

                                visualizations::draw_snr_table(ui, &viz);
                            } else {
                                ui.label("Waiting for audio data...");
                            }
                        });
                    });
                });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;

        // Reinitialize denoisers with correct sample rate
        self.denoiser_left = RealtimeDenoiser::new(buffer_config.sample_rate as u32);
        self.denoiser_right = RealtimeDenoiser::new(buffer_config.sample_rate as u32);

        true
    }

    fn reset(&mut self) {
        self.denoiser_left.reset();
        self.denoiser_right.reset();

        // Clear all buffers
        self.input_ring_left.fill(0.0);
        self.input_ring_right.fill(0.0);
        self.output_buffer_left.clear();
        self.output_buffer_right.clear();
        self.samples_since_process = 0;
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Check if HP slope parameter changed - rebuild filters if needed
        let current_hp_slope = self.params.filters.hp_slope.value();
        if current_hp_slope != self.prev_hp_slope {
            self.filter_left = FilterChain::new(self.sample_rate, current_hp_slope);
            self.filter_right = FilterChain::new(self.sample_rate, current_hp_slope);
            self.prev_hp_slope = current_hp_slope;
        }

        // Check if reset button was pressed (any edge detection)
        let reset_state = self.params.reset_noise.value();
        if reset_state != self.prev_reset_state {
            // State changed (either edge) - reset everything
            self.filter_left.reset();
            self.filter_right.reset();
            self.denoiser_left.reset();
            self.denoiser_right.reset();
            self.input_ring_left.fill(0.0);
            self.input_ring_right.fill(0.0);
            self.output_buffer_left.clear();
            self.output_buffer_right.clear();
            self.samples_since_process = 0;
        }
        self.prev_reset_state = reset_state;

        // Update denoiser parameters
        let params = self.params.get_denoiser_params();

        let num_channels = buffer.channels();
        let num_samples = buffer.samples();

        if num_channels == 0 || num_samples == 0 {
            return ProcessStatus::Normal;
        }

        // Report latency (lookahead needed for STFT)
        context.set_latency_samples(self.denoiser_left.latency_samples());

        // Process based on channel count
        match num_channels {
            1 => {
                // Mono processing
                self.denoiser_left.set_params(params);
                self.process_mono_channel(buffer, 0);
            }
            2 => {
                // Stereo L/R independent
                self.denoiser_left.set_params(params.clone());
                self.denoiser_right.set_params(params);
                self.process_stereo_lr(buffer);
            }
            _ => {
                // Unsupported configuration, pass through
            }
        }

        ProcessStatus::Normal
    }

    fn deactivate(&mut self) {}
}

// =============================================================================
// Processing Helpers
// =============================================================================

impl Poddyclip {
    fn process_mono_channel(&mut self, buffer: &mut Buffer, _channel_idx: usize) {
        // Check if editor is open to enable visualization
        let viz_enabled = self.params.editor_state.is_open();
        self.denoiser_left.set_visualization_enabled(viz_enabled);

        let filter_enabled = self.params.filters.enable.value();

        for mut channel_samples in buffer.iter_samples() {
            let input_sample = channel_samples.get_mut(0).copied().unwrap_or(0.0);

            // Apply filters before denoising
            let filtered_sample = if filter_enabled {
                self.filter_left.process(input_sample)
            } else {
                input_sample
            };

            // Slide the window: shift left and add new sample at the end
            self.input_ring_left.rotate_left(1);
            self.input_ring_left[WINDOW_SIZE - 1] = filtered_sample;
            self.samples_since_process += 1;

            // Process every HOP_SIZE samples
            if self.samples_since_process >= HOP_SIZE {
                let output_frame = self.denoiser_left.process_frame(&self.input_ring_left);
                self.output_buffer_left.extend(output_frame);
                self.samples_since_process = 0;

                // Update visualization data after processing
                if viz_enabled {
                    if let Ok(mut viz) = self.visualization_data.try_lock() {
                        *viz = self.denoiser_left.get_visualization_data();
                    }
                }
            }

            // Output from buffer
            let output_sample = if !self.output_buffer_left.is_empty() {
                self.output_buffer_left.remove(0)
            } else {
                input_sample // Pass through if no output ready yet
            };

            if let Some(sample) = channel_samples.get_mut(0) {
                *sample = output_sample;
            }
        }
    }

    fn process_stereo_lr(&mut self, buffer: &mut Buffer) {
        // Check if editor is open to enable visualization
        let viz_enabled = self.params.editor_state.is_open();
        self.denoiser_left.set_visualization_enabled(viz_enabled);
        self.denoiser_right.set_visualization_enabled(false); // Only visualize left

        let filter_enabled = self.params.filters.enable.value();

        for mut channel_samples in buffer.iter_samples() {
            let left_in = channel_samples.get_mut(0).copied().unwrap_or(0.0);
            let right_in = channel_samples.get_mut(1).copied().unwrap_or(0.0);

            // Apply filters before denoising
            let filtered_left = if filter_enabled {
                self.filter_left.process(left_in)
            } else {
                left_in
            };
            let filtered_right = if filter_enabled {
                self.filter_right.process(right_in)
            } else {
                right_in
            };

            // Slide windows and add new samples
            self.input_ring_left.rotate_left(1);
            self.input_ring_left[WINDOW_SIZE - 1] = filtered_left;
            self.input_ring_right.rotate_left(1);
            self.input_ring_right[WINDOW_SIZE - 1] = filtered_right;
            self.samples_since_process += 1;

            // Process every HOP_SIZE samples
            if self.samples_since_process >= HOP_SIZE {
                let left_frame = self.denoiser_left.process_frame(&self.input_ring_left);
                let right_frame = self.denoiser_right.process_frame(&self.input_ring_right);
                self.output_buffer_left.extend(left_frame);
                self.output_buffer_right.extend(right_frame);
                self.samples_since_process = 0;

                // Update visualization from left channel
                if viz_enabled {
                    if let Ok(mut viz) = self.visualization_data.try_lock() {
                        *viz = self.denoiser_left.get_visualization_data();
                    }
                }
            }

            // Output from buffers
            let left_out = if !self.output_buffer_left.is_empty() {
                self.output_buffer_left.remove(0)
            } else {
                left_in
            };

            let right_out = if !self.output_buffer_right.is_empty() {
                self.output_buffer_right.remove(0)
            } else {
                right_in
            };

            if let Some(sample) = channel_samples.get_mut(0) {
                *sample = left_out;
            }
            if let Some(sample) = channel_samples.get_mut(1) {
                *sample = right_out;
            }
        }
    }
}

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
