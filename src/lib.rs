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
    WINDOW_SIZE,
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
struct BandParams {
    // Delta per band (24 bands)
    #[id = "delta_0"]
    delta_0: FloatParam,
    #[id = "delta_1"]
    delta_1: FloatParam,
    #[id = "delta_2"]
    delta_2: FloatParam,
    #[id = "delta_3"]
    delta_3: FloatParam,
    #[id = "delta_4"]
    delta_4: FloatParam,
    #[id = "delta_5"]
    delta_5: FloatParam,
    #[id = "delta_6"]
    delta_6: FloatParam,
    #[id = "delta_7"]
    delta_7: FloatParam,
    #[id = "delta_8"]
    delta_8: FloatParam,
    #[id = "delta_9"]
    delta_9: FloatParam,
    #[id = "delta_10"]
    delta_10: FloatParam,
    #[id = "delta_11"]
    delta_11: FloatParam,
    #[id = "delta_12"]
    delta_12: FloatParam,
    #[id = "delta_13"]
    delta_13: FloatParam,
    #[id = "delta_14"]
    delta_14: FloatParam,
    #[id = "delta_15"]
    delta_15: FloatParam,
    #[id = "delta_16"]
    delta_16: FloatParam,
    #[id = "delta_17"]
    delta_17: FloatParam,
    #[id = "delta_18"]
    delta_18: FloatParam,
    #[id = "delta_19"]
    delta_19: FloatParam,
    #[id = "delta_20"]
    delta_20: FloatParam,
    #[id = "delta_21"]
    delta_21: FloatParam,
    #[id = "delta_22"]
    delta_22: FloatParam,
    #[id = "delta_23"]
    delta_23: FloatParam,

    // Gamma per band (24 bands)
    #[id = "gamma_0"]
    gamma_0: FloatParam,
    #[id = "gamma_1"]
    gamma_1: FloatParam,
    #[id = "gamma_2"]
    gamma_2: FloatParam,
    #[id = "gamma_3"]
    gamma_3: FloatParam,
    #[id = "gamma_4"]
    gamma_4: FloatParam,
    #[id = "gamma_5"]
    gamma_5: FloatParam,
    #[id = "gamma_6"]
    gamma_6: FloatParam,
    #[id = "gamma_7"]
    gamma_7: FloatParam,
    #[id = "gamma_8"]
    gamma_8: FloatParam,
    #[id = "gamma_9"]
    gamma_9: FloatParam,
    #[id = "gamma_10"]
    gamma_10: FloatParam,
    #[id = "gamma_11"]
    gamma_11: FloatParam,
    #[id = "gamma_12"]
    gamma_12: FloatParam,
    #[id = "gamma_13"]
    gamma_13: FloatParam,
    #[id = "gamma_14"]
    gamma_14: FloatParam,
    #[id = "gamma_15"]
    gamma_15: FloatParam,
    #[id = "gamma_16"]
    gamma_16: FloatParam,
    #[id = "gamma_17"]
    gamma_17: FloatParam,
    #[id = "gamma_18"]
    gamma_18: FloatParam,
    #[id = "gamma_19"]
    gamma_19: FloatParam,
    #[id = "gamma_20"]
    gamma_20: FloatParam,
    #[id = "gamma_21"]
    gamma_21: FloatParam,
    #[id = "gamma_22"]
    gamma_22: FloatParam,
    #[id = "gamma_23"]
    gamma_23: FloatParam,
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

    #[nested(group = "Per-Band")]
    bands: BandParams,
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

            bands: BandParams {
                delta_0: Self::make_delta_param("Delta 0-100Hz", DEFAULT_DELTA[0]),
                delta_1: Self::make_delta_param("Delta 100-200Hz", DEFAULT_DELTA[1]),
                delta_2: Self::make_delta_param("Delta 200-300Hz", DEFAULT_DELTA[2]),
                delta_3: Self::make_delta_param("Delta 300-400Hz", DEFAULT_DELTA[3]),
                delta_4: Self::make_delta_param("Delta 400-510Hz", DEFAULT_DELTA[4]),
                delta_5: Self::make_delta_param("Delta 510-630Hz", DEFAULT_DELTA[5]),
                delta_6: Self::make_delta_param("Delta 630-770Hz", DEFAULT_DELTA[6]),
                delta_7: Self::make_delta_param("Delta 770-920Hz", DEFAULT_DELTA[7]),
                delta_8: Self::make_delta_param("Delta 920-1080Hz", DEFAULT_DELTA[8]),
                delta_9: Self::make_delta_param("Delta 1.1-1.3kHz", DEFAULT_DELTA[9]),
                delta_10: Self::make_delta_param("Delta 1.3-1.5kHz", DEFAULT_DELTA[10]),
                delta_11: Self::make_delta_param("Delta 1.5-1.7kHz", DEFAULT_DELTA[11]),
                delta_12: Self::make_delta_param("Delta 1.7-2.0kHz", DEFAULT_DELTA[12]),
                delta_13: Self::make_delta_param("Delta 2.0-2.3kHz", DEFAULT_DELTA[13]),
                delta_14: Self::make_delta_param("Delta 2.3-2.7kHz", DEFAULT_DELTA[14]),
                delta_15: Self::make_delta_param("Delta 2.7-3.2kHz", DEFAULT_DELTA[15]),
                delta_16: Self::make_delta_param("Delta 3.2-3.7kHz", DEFAULT_DELTA[16]),
                delta_17: Self::make_delta_param("Delta 3.7-4.4kHz", DEFAULT_DELTA[17]),
                delta_18: Self::make_delta_param("Delta 4.4-5.3kHz", DEFAULT_DELTA[18]),
                delta_19: Self::make_delta_param("Delta 5.3-6.4kHz", DEFAULT_DELTA[19]),
                delta_20: Self::make_delta_param("Delta 6.4-7.7kHz", DEFAULT_DELTA[20]),
                delta_21: Self::make_delta_param("Delta 7.7-9.5kHz", DEFAULT_DELTA[21]),
                delta_22: Self::make_delta_param("Delta 9.5-12kHz", DEFAULT_DELTA[22]),
                delta_23: Self::make_delta_param("Delta 12-15.5kHz", DEFAULT_DELTA[23]),

                gamma_0: Self::make_gamma_param("Gamma 0-100Hz", DEFAULT_GAMMA[0]),
                gamma_1: Self::make_gamma_param("Gamma 100-200Hz", DEFAULT_GAMMA[1]),
                gamma_2: Self::make_gamma_param("Gamma 200-300Hz", DEFAULT_GAMMA[2]),
                gamma_3: Self::make_gamma_param("Gamma 300-400Hz", DEFAULT_GAMMA[3]),
                gamma_4: Self::make_gamma_param("Gamma 400-510Hz", DEFAULT_GAMMA[4]),
                gamma_5: Self::make_gamma_param("Gamma 510-630Hz", DEFAULT_GAMMA[5]),
                gamma_6: Self::make_gamma_param("Gamma 630-770Hz", DEFAULT_GAMMA[6]),
                gamma_7: Self::make_gamma_param("Gamma 770-920Hz", DEFAULT_GAMMA[7]),
                gamma_8: Self::make_gamma_param("Gamma 920-1080Hz", DEFAULT_GAMMA[8]),
                gamma_9: Self::make_gamma_param("Gamma 1.1-1.3kHz", DEFAULT_GAMMA[9]),
                gamma_10: Self::make_gamma_param("Gamma 1.3-1.5kHz", DEFAULT_GAMMA[10]),
                gamma_11: Self::make_gamma_param("Gamma 1.5-1.7kHz", DEFAULT_GAMMA[11]),
                gamma_12: Self::make_gamma_param("Gamma 1.7-2.0kHz", DEFAULT_GAMMA[12]),
                gamma_13: Self::make_gamma_param("Gamma 2.0-2.3kHz", DEFAULT_GAMMA[13]),
                gamma_14: Self::make_gamma_param("Gamma 2.3-2.7kHz", DEFAULT_GAMMA[14]),
                gamma_15: Self::make_gamma_param("Gamma 2.7-3.2kHz", DEFAULT_GAMMA[15]),
                gamma_16: Self::make_gamma_param("Gamma 3.2-3.7kHz", DEFAULT_GAMMA[16]),
                gamma_17: Self::make_gamma_param("Gamma 3.7-4.4kHz", DEFAULT_GAMMA[17]),
                gamma_18: Self::make_gamma_param("Gamma 4.4-5.3kHz", DEFAULT_GAMMA[18]),
                gamma_19: Self::make_gamma_param("Gamma 5.3-6.4kHz", DEFAULT_GAMMA[19]),
                gamma_20: Self::make_gamma_param("Gamma 6.4-7.7kHz", DEFAULT_GAMMA[20]),
                gamma_21: Self::make_gamma_param("Gamma 7.7-9.5kHz", DEFAULT_GAMMA[21]),
                gamma_22: Self::make_gamma_param("Gamma 9.5-12kHz", DEFAULT_GAMMA[22]),
                gamma_23: Self::make_gamma_param("Gamma 12-15.5kHz", DEFAULT_GAMMA[23]),
            },
        }
    }
}

impl PoddyclipParams {
    fn make_delta_param(name: &str, default: f32) -> FloatParam {
        FloatParam::new(
            name,
            default,
            FloatRange::Linear {
                min: 0.01,
                max: 5.0,
            },
        )
        .with_step_size(0.01)
        .with_value_to_string(formatters::v2s_f32_rounded(2))
    }

    fn make_gamma_param(name: &str, default: f32) -> FloatParam {
        FloatParam::new(
            name,
            default,
            FloatRange::Linear {
                min: 0.0,
                max: 0.999,
            },
        )
        .with_step_size(0.001)
        .with_value_to_string(formatters::v2s_f32_rounded(3))
    }

    fn get_delta_array(&self) -> [f32; NUM_BANDS] {
        [
            self.bands.delta_0.value(),
            self.bands.delta_1.value(),
            self.bands.delta_2.value(),
            self.bands.delta_3.value(),
            self.bands.delta_4.value(),
            self.bands.delta_5.value(),
            self.bands.delta_6.value(),
            self.bands.delta_7.value(),
            self.bands.delta_8.value(),
            self.bands.delta_9.value(),
            self.bands.delta_10.value(),
            self.bands.delta_11.value(),
            self.bands.delta_12.value(),
            self.bands.delta_13.value(),
            self.bands.delta_14.value(),
            self.bands.delta_15.value(),
            self.bands.delta_16.value(),
            self.bands.delta_17.value(),
            self.bands.delta_18.value(),
            self.bands.delta_19.value(),
            self.bands.delta_20.value(),
            self.bands.delta_21.value(),
            self.bands.delta_22.value(),
            self.bands.delta_23.value(),
        ]
    }

    fn get_gamma_array(&self) -> [f32; NUM_BANDS] {
        [
            self.bands.gamma_0.value(),
            self.bands.gamma_1.value(),
            self.bands.gamma_2.value(),
            self.bands.gamma_3.value(),
            self.bands.gamma_4.value(),
            self.bands.gamma_5.value(),
            self.bands.gamma_6.value(),
            self.bands.gamma_7.value(),
            self.bands.gamma_8.value(),
            self.bands.gamma_9.value(),
            self.bands.gamma_10.value(),
            self.bands.gamma_11.value(),
            self.bands.gamma_12.value(),
            self.bands.gamma_13.value(),
            self.bands.gamma_14.value(),
            self.bands.gamma_15.value(),
            self.bands.gamma_16.value(),
            self.bands.gamma_17.value(),
            self.bands.gamma_18.value(),
            self.bands.gamma_19.value(),
            self.bands.gamma_20.value(),
            self.bands.gamma_21.value(),
            self.bands.gamma_22.value(),
            self.bands.gamma_23.value(),
        ]
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

                                // Per-band delta
                                ui.heading("Per-Band Delta (Subtraction Sensitivity)");
                                ui.add_space(5.0);

                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_0,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_1,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_2,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_3,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_4,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_5,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_6,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_7,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_8,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_9,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_10,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_11,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_12,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_13,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_14,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_15,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_16,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_17,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_18,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_19,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_20,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_21,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_22,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.delta_23,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // Per-band gamma
                                ui.heading("Per-Band Gamma (Temporal Smoothing)");
                                ui.add_space(5.0);

                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_0,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_1,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_2,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_3,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_4,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_5,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_6,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_7,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_8,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_9,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_10,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_11,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_12,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_13,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_14,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_15,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_16,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_17,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_18,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_19,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_20,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_21,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_22,
                                    setter,
                                ));
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.bands.gamma_23,
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
