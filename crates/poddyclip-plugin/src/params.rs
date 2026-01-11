//! Plugin parameters and their default values

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

use poddyclip::denoiser::{
    DenoiserParams, DEFAULT_LAMBDA, DEFAULT_SFM_NOISE, DEFAULT_SFM_SPEECH,
    DEFAULT_SPIKE_THRESHOLD, NUM_BANDS, PRESETS,
};
use poddyclip::eq::HighPassSlope;

// =============================================================================
// Parameter Structs
// =============================================================================

#[derive(Params)]
pub struct FilterParams {
    #[id = "filter_enable"]
    pub enable: BoolParam,

    #[id = "hp_slope"]
    pub hp_slope: EnumParam<HighPassSlope>,
}

#[derive(Params)]
pub struct SubtractionParams {
    #[id = "alpha_base"]
    pub alpha_base: FloatParam,

    #[id = "alpha_min"]
    pub alpha_min: FloatParam,

    #[id = "alpha_max"]
    pub alpha_max: FloatParam,

    #[id = "beta"]
    pub beta: FloatParam,
}

#[derive(Params)]
pub struct NoiseEstimationParams {
    #[id = "lambda"]
    pub lambda: FloatParam,

    #[id = "spike_threshold"]
    pub spike_threshold: FloatParam,

    #[id = "sfm_speech"]
    pub sfm_speech: FloatParam,

    #[id = "sfm_noise"]
    pub sfm_noise: FloatParam,
}

#[derive(Params)]
pub struct PresetParams {
    #[id = "strength"]
    pub strength: FloatParam,
}

#[derive(Params)]
pub struct DeMudParams {
    #[id = "demud_enable"]
    pub enable: BoolParam,

    #[id = "demud_macro"]
    pub macro_val: FloatParam,

    #[id = "demud_freq"]
    pub frequency: FloatParam,
}

#[derive(Params)]
pub struct CorrectionAParams {
    #[id = "corr_a_enable"]
    pub enable: BoolParam,

    #[id = "corr_a_macro"]
    pub macro_val: FloatParam,

    #[id = "corr_a_freq"]
    pub frequency: FloatParam,
}

#[derive(Params)]
pub struct CorrectionBParams {
    #[id = "corr_b_enable"]
    pub enable: BoolParam,

    #[id = "corr_b_macro"]
    pub macro_val: FloatParam,

    #[id = "corr_b_freq"]
    pub frequency: FloatParam,
}

#[derive(Params)]
pub struct DeEsserParams {
    #[id = "deesser_enable"]
    pub enable: BoolParam,

    #[id = "deesser_freq"]
    pub frequency: FloatParam,

    #[id = "deesser_q"]
    pub q: FloatParam,

    #[id = "deesser_strength"]
    pub strength: FloatParam,
}

#[derive(Params)]
pub struct FetCompParams {
    #[id = "fetcomp_enable"]
    pub enable: BoolParam,

    #[id = "fetcomp_threshold"]
    pub threshold: FloatParam,

    #[id = "fetcomp_ratio"]
    pub ratio: FloatParam,

    #[id = "fetcomp_attack"]
    pub attack: FloatParam,

    #[id = "fetcomp_release"]
    pub release: FloatParam,

    #[id = "fetcomp_input_drive"]
    pub input_drive: FloatParam,

    #[id = "fetcomp_output_drive"]
    pub output_drive: FloatParam,
}

#[derive(Params)]
pub struct PeakCompParams {
    #[id = "peakcomp_enable"]
    pub enable: BoolParam,

    #[id = "peakcomp_threshold"]
    pub threshold: FloatParam,

    #[id = "peakcomp_ratio"]
    pub ratio: FloatParam,

    #[id = "peakcomp_attack"]
    pub attack: FloatParam,

    #[id = "peakcomp_release"]
    pub release: FloatParam,
}

#[derive(Params)]
pub struct Channel9Params {
    #[id = "channel9_enable"]
    pub enable: BoolParam,

    #[id = "channel9_drive"]
    pub drive: FloatParam,
}

#[derive(Params)]
pub struct EnhanceEqParams {
    #[id = "enhanceeq_enable"]
    pub enable: BoolParam,

    #[id = "enhanceeq_lowmid_freq"]
    pub lowmid_freq: FloatParam,

    #[id = "enhanceeq_lowmid_gain"]
    pub lowmid_gain: FloatParam,

    #[id = "enhanceeq_presence_freq"]
    pub presence_freq: FloatParam,

    #[id = "enhanceeq_presence_gain"]
    pub presence_gain: FloatParam,

    #[id = "enhanceeq_air_gain"]
    pub air_gain: FloatParam,
}

#[derive(Params)]
pub struct ButterCompParams {
    #[id = "buttercomp_enable"]
    pub enable: BoolParam,

    #[id = "buttercomp_compress"]
    pub compress: FloatParam,
}

#[derive(Params)]
pub struct TapeGlueParams {
    #[id = "tapeglue_enable"]
    pub enable: BoolParam,

    #[id = "tapeglue_warmth"]
    pub warmth: FloatParam,
}

#[derive(Params)]
pub struct LimiterParams {
    #[id = "limiter_enable"]
    pub enable: BoolParam,

    #[id = "limiter_ceiling"]
    pub ceiling: FloatParam,
}

// =============================================================================
// Main Plugin Parameters
// =============================================================================

#[derive(Params)]
pub struct PoddyclipParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "reset_noise"]
    pub reset_noise: BoolParam,

    #[nested(group = "Filters")]
    pub filters: FilterParams,

    #[nested(group = "Subtraction")]
    pub subtraction: SubtractionParams,

    #[nested(group = "Noise Estimation")]
    pub noise_estimation: NoiseEstimationParams,

    #[nested(group = "Preset")]
    pub preset: PresetParams,

    #[nested(group = "Dynamic EQ")]
    pub demud: DeMudParams,

    #[nested(group = "Dynamic EQ")]
    pub correction_a: CorrectionAParams,

    #[nested(group = "Dynamic EQ")]
    pub correction_b: CorrectionBParams,

    #[nested(group = "De-Esser")]
    pub deesser: DeEsserParams,

    #[nested(group = "FET Comp")]
    pub fetcomp: FetCompParams,

    #[nested(group = "Peak Comp")]
    pub peakcomp: PeakCompParams,

    #[nested(group = "Transformer")]
    pub channel9: Channel9Params,

    #[nested(group = "Enhance EQ")]
    pub enhance_eq: EnhanceEqParams,

    #[nested(group = "Compressor")]
    pub buttercomp: ButterCompParams,

    #[nested(group = "Tape")]
    pub tape_glue: TapeGlueParams,

    #[nested(group = "Limiter")]
    pub limiter: LimiterParams,
}

// =============================================================================
// Default Implementation
// =============================================================================

impl Default for PoddyclipParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1400, 900),

            reset_noise: BoolParam::new("Reset Noise Estimation", false).with_value_to_string(
                Arc::new(|value| {
                    if value { "RESET".to_string() } else { "Ready".to_string() }
                }),
            ),

            filters: FilterParams {
                enable: BoolParam::new("Enable Filters", true),
                hp_slope: EnumParam::new("HP Slope", HighPassSlope::Slope24dB),
            },

            subtraction: SubtractionParams {
                alpha_base: FloatParam::new(
                    "Alpha Base",
                    PRESETS[2].alpha_base,
                    FloatRange::Linear { min: 0.0, max: 25.0 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                alpha_min: FloatParam::new(
                    "Alpha Min",
                    PRESETS[2].alpha_min,
                    FloatRange::Linear { min: 0.0, max: 25.0 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                alpha_max: FloatParam::new(
                    "Alpha Max",
                    PRESETS[2].alpha_max,
                    FloatRange::Linear { min: 0.0, max: 25.0 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                beta: FloatParam::new(
                    "Beta (Floor)",
                    PRESETS[2].beta,
                    FloatRange::Linear { min: 0.0, max: 0.1 },
                )
                .with_step_size(0.001)
                .with_value_to_string(formatters::v2s_f32_rounded(3)),
            },

            noise_estimation: NoiseEstimationParams {
                lambda: FloatParam::new(
                    "Lambda (Forget Factor)",
                    DEFAULT_LAMBDA,
                    FloatRange::Linear { min: 0.0, max: 0.9999 },
                )
                .with_step_size(0.0001)
                .with_value_to_string(formatters::v2s_f32_rounded(4)),

                spike_threshold: FloatParam::new(
                    "Spike Threshold",
                    DEFAULT_SPIKE_THRESHOLD,
                    FloatRange::Linear { min: 0.5, max: 100.0 },
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
                    3.0,
                    FloatRange::Linear { min: 1.0, max: 5.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            },

            demud: DeMudParams {
                enable: BoolParam::new("Enable De-Mud", false),
                macro_val: FloatParam::new(
                    "De-Mud Strength",
                    0.5,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                frequency: FloatParam::new(
                    "De-Mud Freq",
                    300.0,
                    FloatRange::Skewed { min: 150.0, max: 500.0, factor: FloatRange::skew_factor(-0.5) },
                )
                .with_step_size(1.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
            },

            correction_a: CorrectionAParams {
                enable: BoolParam::new("Enable Correction A", false),
                macro_val: FloatParam::new(
                    "Correction A Strength",
                    0.5,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                frequency: FloatParam::new(
                    "Correction A Freq",
                    1000.0,
                    FloatRange::Skewed { min: 500.0, max: 5000.0, factor: FloatRange::skew_factor(-0.5) },
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
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                frequency: FloatParam::new(
                    "Correction B Freq",
                    3000.0,
                    FloatRange::Skewed { min: 500.0, max: 5000.0, factor: FloatRange::skew_factor(-0.5) },
                )
                .with_step_size(5.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
            },

            deesser: DeEsserParams {
                enable: BoolParam::new("Enable De-Esser", false),
                frequency: FloatParam::new(
                    "De-Esser Freq",
                    6500.0,
                    FloatRange::Skewed { min: 4000.0, max: 10000.0, factor: FloatRange::skew_factor(-0.3) },
                )
                .with_step_size(10.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
                q: FloatParam::new(
                    "De-Esser Q",
                    1.5,
                    FloatRange::Linear { min: 0.7, max: 2.5 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
                strength: FloatParam::new(
                    "De-Esser Strength",
                    0.5,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
            },

            fetcomp: FetCompParams {
                enable: BoolParam::new("Enable FET Comp", false),
                threshold: FloatParam::new(
                    "FET Threshold",
                    -18.0,
                    FloatRange::Linear { min: -40.0, max: 0.0 },
                )
                .with_step_size(0.5)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
                ratio: FloatParam::new(
                    "FET Ratio",
                    4.0,
                    FloatRange::Skewed { min: 2.0, max: 20.0, factor: FloatRange::skew_factor(-1.0) },
                )
                .with_step_size(0.1)
                .with_value_to_string(Arc::new(|v| format!("{:.1}:1", v))),
                attack: FloatParam::new(
                    "FET Attack",
                    0.8,
                    FloatRange::Skewed { min: 0.1, max: 5.0, factor: FloatRange::skew_factor(-1.0) },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" ms"),
                release: FloatParam::new(
                    "FET Release",
                    50.0,
                    FloatRange::Skewed { min: 20.0, max: 500.0, factor: FloatRange::skew_factor(-1.0) },
                )
                .with_step_size(1.0)
                .with_value_to_string(formatters::v2s_f32_rounded(0))
                .with_unit(" ms"),
                input_drive: FloatParam::new(
                    "Input Drive",
                    0.3,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
                output_drive: FloatParam::new(
                    "Output Drive",
                    0.1,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
            },

            peakcomp: PeakCompParams {
                enable: BoolParam::new("Enable Peak Comp", false),
                threshold: FloatParam::new(
                    "Threshold",
                    -12.0,
                    FloatRange::Linear { min: -40.0, max: 0.0 },
                )
                .with_step_size(0.5)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
                ratio: FloatParam::new(
                    "Ratio",
                    12.0,
                    FloatRange::Skewed { min: 1.0, max: 20.0, factor: FloatRange::skew_factor(-1.0) },
                )
                .with_step_size(0.1)
                .with_value_to_string(Arc::new(|v| format!("{:.1}:1", v))),
                attack: FloatParam::new(
                    "Attack",
                    0.5,
                    FloatRange::Skewed { min: 0.1, max: 10.0, factor: FloatRange::skew_factor(-1.0) },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" ms"),
                release: FloatParam::new(
                    "Release",
                    100.0,
                    FloatRange::Skewed { min: 10.0, max: 500.0, factor: FloatRange::skew_factor(-1.0) },
                )
                .with_step_size(1.0)
                .with_value_to_string(formatters::v2s_f32_rounded(0))
                .with_unit(" ms"),
            },

            channel9: Channel9Params {
                enable: BoolParam::new("Enable Neve Transformer", false),
                drive: FloatParam::new(
                    "Drive",
                    0.25,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(Arc::new(|v| format!("{:.0}%", v * 200.0)))
                .with_string_to_value(Arc::new(|s| {
                    s.trim_end_matches('%').parse::<f32>().ok().map(|v| v / 200.0)
                })),
            },

            enhance_eq: EnhanceEqParams {
                enable: BoolParam::new("Enable Enhance EQ", true),
                lowmid_freq: FloatParam::new(
                    "Low-Mid Freq",
                    250.0,
                    FloatRange::Skewed { min: 150.0, max: 400.0, factor: FloatRange::skew_factor(-0.3) },
                )
                .with_step_size(5.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
                lowmid_gain: FloatParam::new(
                    "Low-Mid Cut",
                    -1.5,
                    FloatRange::Linear { min: -6.0, max: 0.0 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
                presence_freq: FloatParam::new(
                    "Presence Freq",
                    3000.0,
                    FloatRange::Skewed { min: 2000.0, max: 5000.0, factor: FloatRange::skew_factor(-0.3) },
                )
                .with_step_size(10.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
                presence_gain: FloatParam::new(
                    "Presence Gain",
                    1.0,
                    FloatRange::Linear { min: 0.0, max: 6.0 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
                air_gain: FloatParam::new(
                    "Air Gain",
                    3.0,
                    FloatRange::Linear { min: 0.0, max: 6.0 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
            },

            buttercomp: ButterCompParams {
                enable: BoolParam::new("Enable ButterComp", true),
                compress: FloatParam::new(
                    "Compress",
                    0.3,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
            },

            tape_glue: TapeGlueParams {
                enable: BoolParam::new("Enable TapeGlue", true),
                warmth: FloatParam::new(
                    "Warmth",
                    0.3,
                    FloatRange::Linear { min: 0.0, max: 1.0 },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
            },

            limiter: LimiterParams {
                enable: BoolParam::new("Enable Limiter", true),
                ceiling: FloatParam::new(
                    "Ceiling",
                    -1.0,
                    FloatRange::Linear { min: -6.0, max: 0.0 },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dBTP"),
            },
        }
    }
}

// =============================================================================
// Helper Methods
// =============================================================================

impl PoddyclipParams {
    /// Interpolate between two presets based on strength parameter
    pub fn interpolate_preset_values(&self) -> ([f32; NUM_BANDS], [f32; NUM_BANDS]) {
        let strength = self.preset.strength.value();
        let strength_clamped = strength.clamp(1.0, 5.0);

        let preset_idx_low = (strength_clamped - 1.0).floor() as usize;
        let preset_idx_high = (strength_clamped - 1.0).ceil() as usize;
        let preset_idx_low = preset_idx_low.min(4);
        let preset_idx_high = preset_idx_high.min(4);

        let t = (strength_clamped - 1.0).fract();

        let preset_low = &PRESETS[preset_idx_low];
        let preset_high = &PRESETS[preset_idx_high];

        let mut delta = [0.0; NUM_BANDS];
        let mut gamma = [0.0; NUM_BANDS];

        for band in 0..NUM_BANDS {
            delta[band] = preset_low.delta[band] * (1.0 - t) + preset_high.delta[band] * t;
            gamma[band] = preset_low.gamma[band] * (1.0 - t) + preset_high.gamma[band] * t;
        }

        (delta, gamma)
    }

    pub fn get_delta_array(&self) -> [f32; NUM_BANDS] {
        let (delta, _) = self.interpolate_preset_values();
        delta
    }

    pub fn get_gamma_array(&self) -> [f32; NUM_BANDS] {
        let (_, gamma) = self.interpolate_preset_values();
        gamma
    }

    pub fn get_denoiser_params(&self) -> DenoiserParams {
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
