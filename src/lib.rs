#![cfg(feature = "plugin")]

mod aireq;
mod channel9;
mod denoiser;
mod dynamic;
mod filters;
mod fixeq;
mod visualizations;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::{Arc, Mutex};

use denoiser::{
    DenoiserParams, RealtimeDenoiser, VisualizationData, DEFAULT_ALPHA_BASE, DEFAULT_ALPHA_MAX,
    DEFAULT_ALPHA_MIN, DEFAULT_BETA, DEFAULT_LAMBDA,
    DEFAULT_SFM_NOISE, DEFAULT_SFM_SPEECH, DEFAULT_SPIKE_THRESHOLD, HOP_SIZE, NUM_BANDS,
    PRESETS, WINDOW_SIZE,
};

use aireq::StereoAirEq;
use channel9::StereoChannel9;
use dynamic::StereoButterComp2;
use filters::{FilterChain, HighPassSlope};
use fixeq::FixEq;

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
struct DeMudParams {
    #[id = "demud_enable"]
    enable: BoolParam,

    #[id = "demud_macro"]
    macro_val: FloatParam,

    #[id = "demud_freq"]
    frequency: FloatParam,
}

#[derive(Params)]
struct DeEsserParams {
    #[id = "deesser_enable"]
    enable: BoolParam,

    #[id = "deesser_macro"]
    macro_val: FloatParam,

    #[id = "deesser_freq"]
    frequency: FloatParam,
}

#[derive(Params)]
struct CorrectionAParams {
    #[id = "corr_a_enable"]
    enable: BoolParam,

    #[id = "corr_a_macro"]
    macro_val: FloatParam,

    #[id = "corr_a_freq"]
    frequency: FloatParam,
}

#[derive(Params)]
struct CorrectionBParams {
    #[id = "corr_b_enable"]
    enable: BoolParam,

    #[id = "corr_b_macro"]
    macro_val: FloatParam,

    #[id = "corr_b_freq"]
    frequency: FloatParam,
}

#[derive(Params)]
struct Channel9Params {
    #[id = "channel9_enable"]
    enable: BoolParam,

    #[id = "channel9_drive"]
    drive: FloatParam,
}

#[derive(Params)]
struct AirEqParams {
    #[id = "aireq_enable"]
    enable: BoolParam,

    #[id = "aireq_gain"]
    gain: FloatParam,
}

#[derive(Params)]
struct ButterCompParams {
    #[id = "buttercomp_enable"]
    enable: BoolParam,

    #[id = "buttercomp_compress"]
    compress: FloatParam,
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

    #[nested(group = "Dynamic EQ")]
    demud: DeMudParams,

    #[nested(group = "Dynamic EQ")]
    deesser: DeEsserParams,

    #[nested(group = "Dynamic EQ")]
    correction_a: CorrectionAParams,

    #[nested(group = "Dynamic EQ")]
    correction_b: CorrectionBParams,

    #[nested(group = "Transformer")]
    channel9: Channel9Params,

    #[nested(group = "Air EQ")]
    air_eq: AirEqParams,

    #[nested(group = "Compressor")]
    buttercomp: ButterCompParams,
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

    // FixEq (post-denoiser dynamic EQ)
    fixeq_left: FixEq,
    fixeq_right: FixEq,

    // Gain reduction for UI meters
    demud_gain_db: Arc<Mutex<f32>>,
    deesser_gain_db: Arc<Mutex<f32>>,
    correction_a_gain_db: Arc<Mutex<f32>>,
    correction_b_gain_db: Arc<Mutex<f32>>,

    // Channel9 (Neve transformer emulation)
    channel9: StereoChannel9,

    // Air EQ (high shelf + LP)
    air_eq: StereoAirEq,

    // ButterComp2 (smooth leveling)
    buttercomp: StereoButterComp2,
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
            fixeq_left: FixEq::new(48000.0),
            fixeq_right: FixEq::new(48000.0),
            demud_gain_db: Arc::new(Mutex::new(0.0)),
            deesser_gain_db: Arc::new(Mutex::new(0.0)),
            correction_a_gain_db: Arc::new(Mutex::new(0.0)),
            correction_b_gain_db: Arc::new(Mutex::new(0.0)),
            channel9: StereoChannel9::new(48000.0),
            air_eq: StereoAirEq::new(48000.0),
            buttercomp: StereoButterComp2::new(48000.0),
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

            demud: DeMudParams {
                enable: BoolParam::new("Enable De-Mud", false),
                macro_val: FloatParam::new(
                    "De-Mud Strength",
                    0.5,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                frequency: FloatParam::new(
                    "De-Mud Freq",
                    300.0,
                    FloatRange::Skewed {
                        min: 150.0,
                        max: 500.0,
                        factor: FloatRange::skew_factor(-0.5),
                    },
                )
                .with_step_size(1.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
            },

            deesser: DeEsserParams {
                enable: BoolParam::new("Enable De-Esser", false),
                macro_val: FloatParam::new(
                    "De-Esser Strength",
                    0.5,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                frequency: FloatParam::new(
                    "De-Esser Freq",
                    6500.0,
                    FloatRange::Skewed {
                        min: 4000.0,
                        max: 12000.0,
                        factor: FloatRange::skew_factor(-0.5),
                    },
                )
                .with_step_size(10.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(1))
                .with_unit(" Hz"),
            },

            correction_a: CorrectionAParams {
                enable: BoolParam::new("Enable Correction A", false),
                macro_val: FloatParam::new(
                    "Correction A Strength",
                    0.5,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                frequency: FloatParam::new(
                    "Correction A Freq",
                    1000.0,
                    FloatRange::Skewed {
                        min: 500.0,
                        max: 5000.0,
                        factor: FloatRange::skew_factor(-0.5),
                    },
                )
                .with_step_size(5.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
            },

            correction_b: CorrectionBParams {
                enable: BoolParam::new("Enable Correction B", false),
                macro_val: FloatParam::new(
                    "Correction B Strength",
                    0.5,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                frequency: FloatParam::new(
                    "Correction B Freq",
                    3000.0,
                    FloatRange::Skewed {
                        min: 500.0,
                        max: 5000.0,
                        factor: FloatRange::skew_factor(-0.5),
                    },
                )
                .with_step_size(5.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
            },

            channel9: Channel9Params {
                enable: BoolParam::new("Enable Neve Transformer", false),
                drive: FloatParam::new(
                    "Drive",
                    0.25, // 50% displayed (0-200% range)
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(Arc::new(|v| format!("{:.0}%", v * 200.0)))
                .with_string_to_value(Arc::new(|s| {
                    s.trim_end_matches('%')
                        .parse::<f32>()
                        .ok()
                        .map(|v| v / 200.0)
                })),
            },

            air_eq: AirEqParams {
                enable: BoolParam::new("Enable Air EQ", true), // On by default
                gain: FloatParam::new(
                    "Air Gain",
                    2.0, // +2dB default
                    FloatRange::Linear {
                        min: 0.0,
                        max: 6.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
            },

            buttercomp: ButterCompParams {
                enable: BoolParam::new("Enable ButterComp", true), // On by default
                compress: FloatParam::new(
                    "Compress",
                    0.3, // 30% default (subtle)
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
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
        let demud_gain = self.demud_gain_db.clone();
        let deesser_gain = self.deesser_gain_db.clone();
        let correction_a_gain = self.correction_a_gain_db.clone();
        let correction_b_gain = self.correction_b_gain_db.clone();

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

                                ui.add_space(15.0);
                                ui.separator();

                                // Dynamic EQ - De-Mud
                                ui.heading("Dynamic EQ");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("De-Mud:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.demud.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Frequency:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.demud.frequency,
                                    setter,
                                ));
                                ui.label("Strength:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.demud.macro_val,
                                    setter,
                                ));

                                ui.add_space(10.0);

                                ui.horizontal(|ui| {
                                    ui.label("De-Esser:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.deesser.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Frequency:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.deesser.frequency,
                                    setter,
                                ));
                                ui.label("Strength:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.deesser.macro_val,
                                    setter,
                                ));

                                ui.add_space(10.0);

                                ui.horizontal(|ui| {
                                    ui.label("Correction A:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.correction_a.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Frequency:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.correction_a.frequency,
                                    setter,
                                ));
                                ui.label("Strength:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.correction_a.macro_val,
                                    setter,
                                ));

                                ui.add_space(10.0);

                                ui.horizontal(|ui| {
                                    ui.label("Correction B:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.correction_b.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Frequency:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.correction_b.frequency,
                                    setter,
                                ));
                                ui.label("Strength:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.correction_b.macro_val,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // Neve Transformer (Channel9)
                                ui.heading("Neve Transformer");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.channel9.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Drive:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.channel9.drive,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // Air EQ
                                ui.heading("Air EQ");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.air_eq.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Gain:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.air_eq.gain,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // ButterComp (smooth leveling)
                                ui.heading("ButterComp");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.buttercomp.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Compress:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.buttercomp.compress,
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
                            }

                            ui.add_space(15.0);
                            ui.separator();

                            // De-Mud Gain Reduction Meter
                            ui.heading("De-Mud Gain Reduction");
                            ui.add_space(5.0);
                            let gain_db = demud_gain.lock().map(|g| *g).unwrap_or(0.0);
                            let reduction = -gain_db; // Convert to positive for display

                            ui.horizontal(|ui| {
                                ui.label(format!("{:.1} dB", gain_db));
                                let max_reduction = 6.0; // Max is -6dB
                                let ratio = (reduction / max_reduction).clamp(0.0, 1.0);
                                let available = ui.available_width() - 10.0;
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(available, 20.0),
                                    egui::Sense::hover(),
                                );

                                // Background
                                ui.painter().rect_filled(
                                    rect,
                                    4.0,
                                    egui::Color32::from_gray(40),
                                );

                                // Meter bar (yellow/orange for reduction)
                                if ratio > 0.0 {
                                    let bar_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(rect.width() * ratio, rect.height()),
                                    );
                                    let color = if ratio > 0.8 {
                                        egui::Color32::from_rgb(255, 100, 50) // Orange-red
                                    } else if ratio > 0.5 {
                                        egui::Color32::from_rgb(255, 180, 50) // Orange
                                    } else {
                                        egui::Color32::from_rgb(255, 220, 100) // Yellow
                                    };
                                    ui.painter().rect_filled(bar_rect, 4.0, color);
                                }
                            });

                            ui.add_space(10.0);

                            // De-Esser Gain Reduction Meter
                            ui.heading("De-Esser Gain Reduction");
                            ui.add_space(5.0);
                            let deesser_db = deesser_gain.lock().map(|g| *g).unwrap_or(0.0);
                            let deesser_reduction = -deesser_db;

                            ui.horizontal(|ui| {
                                ui.label(format!("{:.1} dB", deesser_db));
                                let max_reduction = 8.0; // Max is -8dB for de-esser
                                let ratio = (deesser_reduction / max_reduction).clamp(0.0, 1.0);
                                let available = ui.available_width() - 10.0;
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(available, 20.0),
                                    egui::Sense::hover(),
                                );

                                // Background
                                ui.painter().rect_filled(
                                    rect,
                                    4.0,
                                    egui::Color32::from_gray(40),
                                );

                                // Meter bar (cyan/blue for de-esser)
                                if ratio > 0.0 {
                                    let bar_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(rect.width() * ratio, rect.height()),
                                    );
                                    let color = if ratio > 0.8 {
                                        egui::Color32::from_rgb(50, 150, 255) // Bright blue
                                    } else if ratio > 0.5 {
                                        egui::Color32::from_rgb(80, 180, 255) // Light blue
                                    } else {
                                        egui::Color32::from_rgb(100, 200, 255) // Cyan
                                    };
                                    ui.painter().rect_filled(bar_rect, 4.0, color);
                                }
                            });

                            ui.add_space(10.0);

                            // Correction A Gain Reduction Meter
                            ui.heading("Correction A Gain Reduction");
                            ui.add_space(5.0);
                            let corr_a_db = correction_a_gain.lock().map(|g| *g).unwrap_or(0.0);
                            let corr_a_reduction = -corr_a_db;

                            ui.horizontal(|ui| {
                                ui.label(format!("{:.1} dB", corr_a_db));
                                let max_reduction = 6.0;
                                let ratio = (corr_a_reduction / max_reduction).clamp(0.0, 1.0);
                                let available = ui.available_width() - 10.0;
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(available, 20.0),
                                    egui::Sense::hover(),
                                );

                                ui.painter().rect_filled(
                                    rect,
                                    4.0,
                                    egui::Color32::from_gray(40),
                                );

                                // Meter bar (green for correction A)
                                if ratio > 0.0 {
                                    let bar_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(rect.width() * ratio, rect.height()),
                                    );
                                    let color = if ratio > 0.8 {
                                        egui::Color32::from_rgb(50, 200, 100) // Bright green
                                    } else if ratio > 0.5 {
                                        egui::Color32::from_rgb(80, 220, 120) // Light green
                                    } else {
                                        egui::Color32::from_rgb(100, 240, 150) // Pale green
                                    };
                                    ui.painter().rect_filled(bar_rect, 4.0, color);
                                }
                            });

                            ui.add_space(10.0);

                            // Correction B Gain Reduction Meter
                            ui.heading("Correction B Gain Reduction");
                            ui.add_space(5.0);
                            let corr_b_db = correction_b_gain.lock().map(|g| *g).unwrap_or(0.0);
                            let corr_b_reduction = -corr_b_db;

                            ui.horizontal(|ui| {
                                ui.label(format!("{:.1} dB", corr_b_db));
                                let max_reduction = 6.0;
                                let ratio = (corr_b_reduction / max_reduction).clamp(0.0, 1.0);
                                let available = ui.available_width() - 10.0;
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(available, 20.0),
                                    egui::Sense::hover(),
                                );

                                ui.painter().rect_filled(
                                    rect,
                                    4.0,
                                    egui::Color32::from_gray(40),
                                );

                                // Meter bar (purple for correction B)
                                if ratio > 0.0 {
                                    let bar_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(rect.width() * ratio, rect.height()),
                                    );
                                    let color = if ratio > 0.8 {
                                        egui::Color32::from_rgb(180, 100, 255) // Bright purple
                                    } else if ratio > 0.5 {
                                        egui::Color32::from_rgb(160, 120, 240) // Light purple
                                    } else {
                                        egui::Color32::from_rgb(140, 140, 220) // Pale purple
                                    };
                                    ui.painter().rect_filled(bar_rect, 4.0, color);
                                }
                            });

                            if viz_data.lock().is_err() {
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

        // FixEq parameters - De-mud
        let demud_enabled = self.params.demud.enable.value();
        self.fixeq_left.set_demud_enabled(demud_enabled);
        if demud_enabled {
            self.fixeq_left.set_demud_frequency(self.params.demud.frequency.value());
            self.fixeq_left.set_demud_strength(self.params.demud.macro_val.value());
        } else {
            self.fixeq_left.set_demud_strength(0.0);
        }

        // De-esser
        let deesser_enabled = self.params.deesser.enable.value();
        self.fixeq_left.set_deesser_enabled(deesser_enabled);
        if deesser_enabled {
            self.fixeq_left.set_deesser_frequency(self.params.deesser.frequency.value());
            self.fixeq_left.set_deesser_strength(self.params.deesser.macro_val.value());
        } else {
            self.fixeq_left.set_deesser_strength(0.0);
        }

        // Correction A
        let corr_a_enabled = self.params.correction_a.enable.value();
        self.fixeq_left.set_correction_a_enabled(corr_a_enabled);
        if corr_a_enabled {
            self.fixeq_left.set_correction_a_frequency(self.params.correction_a.frequency.value());
            self.fixeq_left.set_correction_a_strength(self.params.correction_a.macro_val.value());
        } else {
            self.fixeq_left.set_correction_a_strength(0.0);
        }

        // Correction B
        let corr_b_enabled = self.params.correction_b.enable.value();
        self.fixeq_left.set_correction_b_enabled(corr_b_enabled);
        if corr_b_enabled {
            self.fixeq_left.set_correction_b_frequency(self.params.correction_b.frequency.value());
            self.fixeq_left.set_correction_b_strength(self.params.correction_b.macro_val.value());
        } else {
            self.fixeq_left.set_correction_b_strength(0.0);
        }

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
            let mut output_sample = if !self.output_buffer_left.is_empty() {
                self.output_buffer_left.remove(0)
            } else {
                input_sample // Pass through if no output ready yet
            };

            // Apply FixEq AFTER denoiser
            output_sample = self.fixeq_left.process(output_sample);

            // Apply Channel9 (Neve transformer) AFTER FixEq
            if self.params.channel9.enable.value() {
                self.channel9.set_drive(self.params.channel9.drive.value());
                output_sample = self.channel9.left.process(output_sample);
            }

            // Apply Air EQ AFTER Channel9
            if self.params.air_eq.enable.value() {
                self.air_eq.left.set_shelf_gain(self.params.air_eq.gain.value());
                output_sample = self.air_eq.left.process(output_sample);
            }

            // Apply ButterComp AFTER Air EQ
            if self.params.buttercomp.enable.value() {
                self.buttercomp.left.set_compress(self.params.buttercomp.compress.value());
                output_sample = self.buttercomp.left.process(output_sample);
            }

            // Update gain reduction for UI
            if viz_enabled {
                if let Ok(mut gain) = self.demud_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_demud_gain_db();
                }
                if let Ok(mut gain) = self.deesser_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_deesser_gain_db();
                }
                if let Ok(mut gain) = self.correction_a_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_correction_a_gain_db();
                }
                if let Ok(mut gain) = self.correction_b_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_correction_b_gain_db();
                }
            }

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

        // FixEq parameters - De-mud
        let demud_enabled = self.params.demud.enable.value();
        let demud_freq = self.params.demud.frequency.value();
        let demud_strength = if demud_enabled {
            self.params.demud.macro_val.value()
        } else {
            0.0
        };
        self.fixeq_left.set_demud_enabled(demud_enabled);
        self.fixeq_right.set_demud_enabled(demud_enabled);
        if demud_enabled {
            self.fixeq_left.set_demud_frequency(demud_freq);
            self.fixeq_right.set_demud_frequency(demud_freq);
        }
        self.fixeq_left.set_demud_strength(demud_strength);
        self.fixeq_right.set_demud_strength(demud_strength);

        // De-esser
        let deesser_enabled = self.params.deesser.enable.value();
        let deesser_freq = self.params.deesser.frequency.value();
        let deesser_strength = if deesser_enabled {
            self.params.deesser.macro_val.value()
        } else {
            0.0
        };
        self.fixeq_left.set_deesser_enabled(deesser_enabled);
        self.fixeq_right.set_deesser_enabled(deesser_enabled);
        if deesser_enabled {
            self.fixeq_left.set_deesser_frequency(deesser_freq);
            self.fixeq_right.set_deesser_frequency(deesser_freq);
        }
        self.fixeq_left.set_deesser_strength(deesser_strength);
        self.fixeq_right.set_deesser_strength(deesser_strength);

        // Correction A
        let corr_a_enabled = self.params.correction_a.enable.value();
        let corr_a_freq = self.params.correction_a.frequency.value();
        let corr_a_strength = if corr_a_enabled {
            self.params.correction_a.macro_val.value()
        } else {
            0.0
        };
        self.fixeq_left.set_correction_a_enabled(corr_a_enabled);
        self.fixeq_right.set_correction_a_enabled(corr_a_enabled);
        if corr_a_enabled {
            self.fixeq_left.set_correction_a_frequency(corr_a_freq);
            self.fixeq_right.set_correction_a_frequency(corr_a_freq);
        }
        self.fixeq_left.set_correction_a_strength(corr_a_strength);
        self.fixeq_right.set_correction_a_strength(corr_a_strength);

        // Correction B
        let corr_b_enabled = self.params.correction_b.enable.value();
        let corr_b_freq = self.params.correction_b.frequency.value();
        let corr_b_strength = if corr_b_enabled {
            self.params.correction_b.macro_val.value()
        } else {
            0.0
        };
        self.fixeq_left.set_correction_b_enabled(corr_b_enabled);
        self.fixeq_right.set_correction_b_enabled(corr_b_enabled);
        if corr_b_enabled {
            self.fixeq_left.set_correction_b_frequency(corr_b_freq);
            self.fixeq_right.set_correction_b_frequency(corr_b_freq);
        }
        self.fixeq_left.set_correction_b_strength(corr_b_strength);
        self.fixeq_right.set_correction_b_strength(corr_b_strength);

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
            let mut left_out = if !self.output_buffer_left.is_empty() {
                self.output_buffer_left.remove(0)
            } else {
                left_in
            };

            let mut right_out = if !self.output_buffer_right.is_empty() {
                self.output_buffer_right.remove(0)
            } else {
                right_in
            };

            // Apply FixEq AFTER denoiser
            left_out = self.fixeq_left.process(left_out);
            right_out = self.fixeq_right.process(right_out);

            // Apply Channel9 (Neve transformer) AFTER FixEq
            if self.params.channel9.enable.value() {
                self.channel9.set_drive(self.params.channel9.drive.value());
                left_out = self.channel9.left.process(left_out);
                right_out = self.channel9.right.process(right_out);
            }

            // Apply Air EQ AFTER Channel9
            if self.params.air_eq.enable.value() {
                self.air_eq.set_shelf_gain(self.params.air_eq.gain.value());
                left_out = self.air_eq.left.process(left_out);
                right_out = self.air_eq.right.process(right_out);
            }

            // Apply ButterComp AFTER Air EQ
            if self.params.buttercomp.enable.value() {
                self.buttercomp.set_compress(self.params.buttercomp.compress.value());
                left_out = self.buttercomp.left.process(left_out);
                right_out = self.buttercomp.right.process(right_out);
            }

            // Update gain reduction for UI (from left channel only)
            if viz_enabled {
                if let Ok(mut gain) = self.demud_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_demud_gain_db();
                }
                if let Ok(mut gain) = self.deesser_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_deesser_gain_db();
                }
                if let Ok(mut gain) = self.correction_a_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_correction_a_gain_db();
                }
                if let Ok(mut gain) = self.correction_b_gain_db.try_lock() {
                    *gain = self.fixeq_left.get_correction_b_gain_db();
                }
            }

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
