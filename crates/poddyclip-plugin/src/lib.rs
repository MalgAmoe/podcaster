mod visualizations;

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::{Arc, Mutex};

use poddyclip::denoiser::{
    DenoiserParams, StreamingDenoiser, VisualizationData, DEFAULT_LAMBDA,
    DEFAULT_SFM_NOISE, DEFAULT_SFM_SPEECH, DEFAULT_SPIKE_THRESHOLD, NUM_BANDS, PRESETS,
};

use poddyclip::saturation::{Channel9, TapeGlue};
use poddyclip::dynamics::{StereoVcaPeakComp, ButterComp2, StereoRealtimeLimiter};
use poddyclip::eq::{DeEsser, FilterChain, FixEq, HighPassSlope, StereoEnhanceEq};
use poddyclip::traits::Stereo;

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
struct DeEsserParams {
    #[id = "deesser_enable"]
    enable: BoolParam,

    #[id = "deesser_freq"]
    frequency: FloatParam,

    #[id = "deesser_q"]
    q: FloatParam,

    #[id = "deesser_strength"]
    strength: FloatParam,
}

#[derive(Params)]
struct PeakCompParams {
    #[id = "peakcomp_enable"]
    enable: BoolParam,

    #[id = "peakcomp_threshold"]
    threshold: FloatParam,

    #[id = "peakcomp_ratio"]
    ratio: FloatParam,

    #[id = "peakcomp_attack"]
    attack: FloatParam,

    #[id = "peakcomp_release"]
    release: FloatParam,
}

#[derive(Params)]
struct Channel9Params {
    #[id = "channel9_enable"]
    enable: BoolParam,

    #[id = "channel9_drive"]
    drive: FloatParam,
}

#[derive(Params)]
struct EnhanceEqParams {
    #[id = "enhanceeq_enable"]
    enable: BoolParam,

    #[id = "enhanceeq_lowmid_freq"]
    lowmid_freq: FloatParam,

    #[id = "enhanceeq_lowmid_gain"]
    lowmid_gain: FloatParam,

    #[id = "enhanceeq_presence_freq"]
    presence_freq: FloatParam,

    #[id = "enhanceeq_presence_gain"]
    presence_gain: FloatParam,

    #[id = "enhanceeq_air_gain"]
    air_gain: FloatParam,
}

#[derive(Params)]
struct ButterCompParams {
    #[id = "buttercomp_enable"]
    enable: BoolParam,

    #[id = "buttercomp_compress"]
    compress: FloatParam,
}

#[derive(Params)]
struct TapeGlueParams {
    #[id = "tapeglue_enable"]
    enable: BoolParam,

    #[id = "tapeglue_warmth"]
    warmth: FloatParam,
}

#[derive(Params)]
struct LimiterParams {
    #[id = "limiter_enable"]
    enable: BoolParam,

    #[id = "limiter_ceiling"]
    ceiling: FloatParam,
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
    correction_a: CorrectionAParams,

    #[nested(group = "Dynamic EQ")]
    correction_b: CorrectionBParams,

    #[nested(group = "De-Esser")]
    deesser: DeEsserParams,

    #[nested(group = "Peak Comp")]
    peakcomp: PeakCompParams,

    #[nested(group = "Transformer")]
    channel9: Channel9Params,

    #[nested(group = "Enhance EQ")]
    enhance_eq: EnhanceEqParams,

    #[nested(group = "Compressor")]
    buttercomp: ButterCompParams,

    #[nested(group = "Tape")]
    tape_glue: TapeGlueParams,

    #[nested(group = "Limiter")]
    limiter: LimiterParams,
}

// =============================================================================
// Plugin
// =============================================================================

struct Poddyclip {
    params: Arc<PoddyclipParams>,

    // Filters (applied before denoising)
    filter: Stereo<FilterChain>,
    prev_hp_slope: HighPassSlope,

    // Streaming denoiser (handles frame buffering internally)
    denoiser: Stereo<StreamingDenoiser>,
    sample_rate: f32,

    // Track reset button state
    prev_reset_state: bool,

    // Visualization data shared with GUI
    visualization_data: Arc<Mutex<VisualizationData>>,

    // FixEq (post-denoiser dynamic EQ) - single instance handles both channels
    fixeq: FixEq,

    // Gain reduction for UI meters
    demud_gain_db: Arc<Mutex<f32>>,
    correction_a_gain_db: Arc<Mutex<f32>>,
    correction_b_gain_db: Arc<Mutex<f32>>,

    // De-Esser (post-FixEq sibilance reduction)
    deesser: Stereo<DeEsser>,
    deesser_gain_db: Arc<Mutex<f32>>,

    // VCA Peak Compressor (post-DeEsser clinical peak control) - linked stereo
    peakcomp: StereoVcaPeakComp,
    peakcomp_gain_db: Arc<Mutex<f32>>,

    // Channel9 (Neve transformer emulation)
    channel9: Stereo<Channel9>,

    // Enhance EQ (lowmid cut + presence + dynamic air)
    enhance_eq: StereoEnhanceEq,

    // ButterComp2 (smooth leveling)
    buttercomp: Stereo<ButterComp2>,

    // TapeGlue (subtle tape saturation)
    tape_glue: Stereo<TapeGlue>,

    // Limiter (true peak limiting)
    limiter: StereoRealtimeLimiter,

    // Limiter gain reduction for UI
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

impl Default for PoddyclipParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1400, 900),

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
                // Use Moderate preset (index 2) as defaults
                alpha_base: FloatParam::new(
                    "Alpha Base",
                    PRESETS[2].alpha_base,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 25.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                alpha_min: FloatParam::new(
                    "Alpha Min",
                    PRESETS[2].alpha_min,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 25.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(2)),

                alpha_max: FloatParam::new(
                    "Alpha Max",
                    PRESETS[2].alpha_max,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 25.0,
                    },
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

            deesser: DeEsserParams {
                enable: BoolParam::new("Enable De-Esser", false),
                frequency: FloatParam::new(
                    "De-Esser Freq",
                    6500.0, // Default sibilance frequency
                    FloatRange::Skewed {
                        min: 4000.0,
                        max: 10000.0,
                        factor: FloatRange::skew_factor(-0.3),
                    },
                )
                .with_step_size(10.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
                q: FloatParam::new(
                    "De-Esser Q",
                    1.5, // Default Q
                    FloatRange::Linear {
                        min: 0.7,
                        max: 2.5,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
                strength: FloatParam::new(
                    "De-Esser Strength",
                    0.5,
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
            },

            peakcomp: PeakCompParams {
                enable: BoolParam::new("Enable Peak Comp", false),
                threshold: FloatParam::new(
                    "Threshold",
                    -12.0,
                    FloatRange::Linear {
                        min: -40.0,
                        max: 0.0,
                    },
                )
                .with_step_size(0.5)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
                ratio: FloatParam::new(
                    "Ratio",
                    12.0,
                    FloatRange::Skewed {
                        min: 1.0,
                        max: 20.0,
                        factor: FloatRange::skew_factor(-1.0),
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(Arc::new(|v| format!("{:.1}:1", v))),
                attack: FloatParam::new(
                    "Attack",
                    0.5,
                    FloatRange::Skewed {
                        min: 0.1,
                        max: 10.0,
                        factor: FloatRange::skew_factor(-1.0),
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" ms"),
                release: FloatParam::new(
                    "Release",
                    100.0,
                    FloatRange::Skewed {
                        min: 10.0,
                        max: 500.0,
                        factor: FloatRange::skew_factor(-1.0),
                    },
                )
                .with_step_size(1.0)
                .with_value_to_string(formatters::v2s_f32_rounded(0))
                .with_unit(" ms"),
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

            enhance_eq: EnhanceEqParams {
                enable: BoolParam::new("Enable Enhance EQ", true), // On by default
                lowmid_freq: FloatParam::new(
                    "Low-Mid Freq",
                    250.0, // 250Hz default
                    FloatRange::Skewed {
                        min: 150.0,
                        max: 400.0,
                        factor: FloatRange::skew_factor(-0.3),
                    },
                )
                .with_step_size(5.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
                lowmid_gain: FloatParam::new(
                    "Low-Mid Cut",
                    -1.5, // -1.5dB default cut
                    FloatRange::Linear {
                        min: -6.0,
                        max: 0.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
                presence_freq: FloatParam::new(
                    "Presence Freq",
                    3000.0, // 3kHz default
                    FloatRange::Skewed {
                        min: 2000.0,
                        max: 5000.0,
                        factor: FloatRange::skew_factor(-0.3),
                    },
                )
                .with_step_size(10.0)
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_unit(" Hz"),
                presence_gain: FloatParam::new(
                    "Presence Gain",
                    1.0, // +1dB default
                    FloatRange::Linear {
                        min: 0.0,
                        max: 6.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dB"),
                air_gain: FloatParam::new(
                    "Air Gain",
                    3.0, // +3dB default (max for dynamic)
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

            tape_glue: TapeGlueParams {
                enable: BoolParam::new("Enable TapeGlue", true), // On by default
                warmth: FloatParam::new(
                    "Warmth",
                    0.3, // 30% default (subtle)
                    FloatRange::Linear {
                        min: 0.0,
                        max: 1.0,
                    },
                )
                .with_step_size(0.01)
                .with_value_to_string(formatters::v2s_f32_percentage(0)),
            },

            limiter: LimiterParams {
                enable: BoolParam::new("Enable Limiter", true), // On by default
                ceiling: FloatParam::new(
                    "Ceiling",
                    -1.0, // -1 dBTP default
                    FloatRange::Linear {
                        min: -6.0,
                        max: 0.0,
                    },
                )
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1))
                .with_unit(" dBTP"),
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
        let correction_a_gain = self.correction_a_gain_db.clone();
        let correction_b_gain = self.correction_b_gain_db.clone();
        let deesser_gain = self.deesser_gain_db.clone();
        let peakcomp_gain = self.peakcomp_gain_db.clone();
        let limiter_gain = self.limiter_gain_db.clone();

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

                                // De-Esser
                                ui.heading("De-Esser");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
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
                                ui.label("Q:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.deesser.q,
                                    setter,
                                ));
                                ui.label("Strength:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.deesser.strength,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // Peak Compressor
                                ui.heading("Peak Comp (VCA)");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.peakcomp.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Threshold:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.peakcomp.threshold,
                                    setter,
                                ));
                                ui.label("Ratio:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.peakcomp.ratio,
                                    setter,
                                ));
                                ui.label("Attack:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.peakcomp.attack,
                                    setter,
                                ));
                                ui.label("Release:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.peakcomp.release,
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

                                // Enhance EQ (Low-Mid Cut + Presence + Dynamic Air)
                                ui.heading("Enhance EQ");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.enhance_eq.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Low-Mid Freq:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.enhance_eq.lowmid_freq,
                                    setter,
                                ));
                                ui.label("Low-Mid Cut:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.enhance_eq.lowmid_gain,
                                    setter,
                                ));
                                ui.label("Presence Freq:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.enhance_eq.presence_freq,
                                    setter,
                                ));
                                ui.label("Presence Gain:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.enhance_eq.presence_gain,
                                    setter,
                                ));
                                ui.label("Air Gain (dynamic):");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.enhance_eq.air_gain,
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

                                ui.add_space(15.0);
                                ui.separator();

                                // TapeGlue (subtle tape saturation)
                                ui.heading("TapeGlue");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.tape_glue.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Warmth:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.tape_glue.warmth,
                                    setter,
                                ));

                                ui.add_space(15.0);
                                ui.separator();

                                // Limiter
                                ui.heading("Limiter");
                                ui.add_space(5.0);

                                ui.horizontal(|ui| {
                                    ui.label("Enable:");
                                    ui.add(widgets::ParamSlider::for_param(
                                        &params.limiter.enable,
                                        setter,
                                    ));
                                });
                                ui.label("Ceiling:");
                                ui.add(widgets::ParamSlider::for_param(
                                    &params.limiter.ceiling,
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

                            ui.add_space(10.0);

                            // De-Esser Gain Reduction Meter
                            ui.heading("De-Esser Gain Reduction");
                            ui.add_space(5.0);
                            let deesser_db = deesser_gain.lock().map(|g| *g).unwrap_or(0.0);
                            let deesser_reduction = -deesser_db;

                            ui.horizontal(|ui| {
                                ui.label(format!("{:.1} dB", deesser_db));
                                let max_reduction = 8.0; // Max is -8dB
                                let ratio = (deesser_reduction / max_reduction).clamp(0.0, 1.0);
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

                                // Meter bar (cyan for de-esser)
                                if ratio > 0.0 {
                                    let bar_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(rect.width() * ratio, rect.height()),
                                    );
                                    let color = if ratio > 0.8 {
                                        egui::Color32::from_rgb(50, 200, 220) // Bright cyan
                                    } else if ratio > 0.5 {
                                        egui::Color32::from_rgb(80, 220, 230) // Light cyan
                                    } else {
                                        egui::Color32::from_rgb(120, 230, 240) // Pale cyan
                                    };
                                    ui.painter().rect_filled(bar_rect, 4.0, color);
                                }
                            });

                            ui.add_space(10.0);

                            // Peak Comp Gain Reduction Meter
                            ui.heading("Peak Comp Gain Reduction");
                            ui.add_space(5.0);
                            let peakcomp_db = peakcomp_gain.lock().map(|g| *g).unwrap_or(0.0);
                            let peakcomp_reduction = -peakcomp_db;

                            ui.horizontal(|ui| {
                                ui.label(format!("{:.1} dB", peakcomp_db));
                                let max_reduction = 12.0; // Max is -12dB
                                let ratio = (peakcomp_reduction / max_reduction).clamp(0.0, 1.0);
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

                                // Meter bar (magenta for peak comp)
                                if ratio > 0.0 {
                                    let bar_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(rect.width() * ratio, rect.height()),
                                    );
                                    let color = if ratio > 0.8 {
                                        egui::Color32::from_rgb(255, 50, 150) // Bright magenta
                                    } else if ratio > 0.5 {
                                        egui::Color32::from_rgb(255, 100, 180) // Light magenta
                                    } else {
                                        egui::Color32::from_rgb(255, 150, 200) // Pale magenta
                                    };
                                    ui.painter().rect_filled(bar_rect, 4.0, color);
                                }
                            });

                            ui.add_space(10.0);

                            // Limiter Gain Reduction Meter
                            ui.heading("Limiter Gain Reduction");
                            ui.add_space(5.0);
                            let limiter_db = limiter_gain.lock().map(|g| *g).unwrap_or(0.0);
                            let limiter_reduction = -limiter_db;

                            ui.horizontal(|ui| {
                                ui.label(format!("{:.1} dB", limiter_db));
                                let max_reduction = 12.0; // Max display is -12dB
                                let ratio = (limiter_reduction / max_reduction).clamp(0.0, 1.0);
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

                                // Meter bar (red/orange for limiter)
                                if ratio > 0.0 {
                                    let bar_rect = egui::Rect::from_min_size(
                                        rect.min,
                                        egui::vec2(rect.width() * ratio, rect.height()),
                                    );
                                    let color = if ratio > 0.8 {
                                        egui::Color32::from_rgb(255, 50, 50) // Bright red
                                    } else if ratio > 0.5 {
                                        egui::Color32::from_rgb(255, 100, 50) // Orange-red
                                    } else {
                                        egui::Color32::from_rgb(255, 150, 50) // Orange
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
        self.denoiser = Stereo::from_pair(
            StreamingDenoiser::new(buffer_config.sample_rate as u32),
            StreamingDenoiser::new(buffer_config.sample_rate as u32),
        );

        // Reinitialize limiter with correct sample rate
        self.limiter = StereoRealtimeLimiter::new(-1.0, 5.0, 100.0, buffer_config.sample_rate);

        true
    }

    fn reset(&mut self) {
        self.denoiser.reset();
        self.limiter.reset();
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
            self.filter = Stereo::from_pair(
                FilterChain::new(self.sample_rate, current_hp_slope),
                FilterChain::new(self.sample_rate, current_hp_slope),
            );
            self.prev_hp_slope = current_hp_slope;
        }

        // Check if reset button was pressed (any edge detection)
        let reset_state = self.params.reset_noise.value();
        if reset_state != self.prev_reset_state {
            // State changed (either edge) - reset everything
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

        // Report latency (lookahead needed for STFT)
        context.set_latency_samples(self.denoiser.left.latency_samples() as u32);

        // Process based on channel count
        match num_channels {
            1 => {
                // Mono processing
                self.denoiser.left.set_params(params);
                self.process_mono_channel(buffer, 0);
            }
            2 => {
                // Stereo L/R independent
                self.denoiser.left.set_params(params.clone());
                self.denoiser.right.set_params(params);
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
        // =================================================================
        // Read ALL parameters ONCE per buffer (not per sample!)
        // =================================================================
        let viz_enabled = self.params.editor_state.is_open();
        let filter_enabled = self.params.filters.enable.value();

        // Peak Compressor params
        let peakcomp_enabled = self.params.peakcomp.enable.value();
        let peakcomp_threshold = self.params.peakcomp.threshold.value();
        let peakcomp_ratio = self.params.peakcomp.ratio.value();
        let peakcomp_attack = self.params.peakcomp.attack.value();
        let peakcomp_release = self.params.peakcomp.release.value();

        // FixEq params - De-mud
        let demud_enabled = self.params.demud.enable.value();
        let demud_freq = self.params.demud.frequency.value();
        let demud_strength = if demud_enabled {
            self.params.demud.macro_val.value()
        } else {
            0.0
        };

        // FixEq params - Correction A
        let corr_a_enabled = self.params.correction_a.enable.value();
        let corr_a_freq = self.params.correction_a.frequency.value();
        let corr_a_strength = if corr_a_enabled {
            self.params.correction_a.macro_val.value()
        } else {
            0.0
        };

        // FixEq params - Correction B
        let corr_b_enabled = self.params.correction_b.enable.value();
        let corr_b_freq = self.params.correction_b.frequency.value();
        let corr_b_strength = if corr_b_enabled {
            self.params.correction_b.macro_val.value()
        } else {
            0.0
        };

        // De-Esser params
        let deesser_enabled = self.params.deesser.enable.value();
        let deesser_freq = self.params.deesser.frequency.value();
        let deesser_q = self.params.deesser.q.value();
        let deesser_strength = self.params.deesser.strength.value();

        // Channel9 params
        let channel9_enabled = self.params.channel9.enable.value();
        let channel9_drive = self.params.channel9.drive.value();

        // EnhanceEQ params
        let enhance_eq_enabled = self.params.enhance_eq.enable.value();
        let enhance_lowmid_freq = self.params.enhance_eq.lowmid_freq.value();
        let enhance_lowmid_gain = self.params.enhance_eq.lowmid_gain.value();
        let enhance_presence_freq = self.params.enhance_eq.presence_freq.value();
        let enhance_presence_gain = self.params.enhance_eq.presence_gain.value();
        let enhance_air_gain = self.params.enhance_eq.air_gain.value();

        // ButterComp params
        let buttercomp_enabled = self.params.buttercomp.enable.value();
        let buttercomp_compress = self.params.buttercomp.compress.value();

        // TapeGlue params
        let tape_glue_enabled = self.params.tape_glue.enable.value();
        let tape_glue_warmth = self.params.tape_glue.warmth.value() as f64;

        // Limiter params
        let limiter_enabled = self.params.limiter.enable.value();

        // =================================================================
        // Sync params to processors ONCE per buffer
        // =================================================================
        self.denoiser.left.set_visualization_enabled(viz_enabled);

        // Peak Compressor (linked stereo - set both channels)
        if peakcomp_enabled {
            self.peakcomp.set_threshold(peakcomp_threshold);
            self.peakcomp.set_ratio(peakcomp_ratio);
            self.peakcomp.set_attack(peakcomp_attack);
            self.peakcomp.set_release(peakcomp_release);
        }

        // FixEq (single instance, mono mode)
        self.fixeq.set_stereo(false);
        self.fixeq.set_demud_enabled(demud_enabled);
        if demud_enabled {
            self.fixeq.set_demud_frequency(demud_freq);
        }
        self.fixeq.set_demud_strength(demud_strength);

        self.fixeq.set_correction_a_enabled(corr_a_enabled);
        if corr_a_enabled {
            self.fixeq.set_correction_a_frequency(corr_a_freq);
        }
        self.fixeq.set_correction_a_strength(corr_a_strength);

        self.fixeq.set_correction_b_enabled(corr_b_enabled);
        if corr_b_enabled {
            self.fixeq.set_correction_b_frequency(corr_b_freq);
        }
        self.fixeq.set_correction_b_strength(corr_b_strength);

        // De-Esser (set both channels via set_both)
        if deesser_enabled {
            self.deesser.set_both(|d| {
                d.set_frequency(deesser_freq);
                d.set_q(deesser_q);
                d.set_strength(deesser_strength);
            });
        }

        // Channel9
        if channel9_enabled {
            self.channel9.left.set_drive(channel9_drive);
        }

        // EnhanceEQ
        if enhance_eq_enabled {
            self.enhance_eq.left.set_lowmid_freq(enhance_lowmid_freq);
            self.enhance_eq.left.set_lowmid_gain(enhance_lowmid_gain);
            self.enhance_eq.left.set_presence_freq(enhance_presence_freq);
            self.enhance_eq.left.set_presence_gain(enhance_presence_gain);
            self.enhance_eq.left.set_shelf_gain(enhance_air_gain);
        }

        // ButterComp
        if buttercomp_enabled {
            self.buttercomp.left.set_compress(buttercomp_compress);
        }

        // TapeGlue
        if tape_glue_enabled {
            self.tape_glue.left.set_warmth(tape_glue_warmth);
        }

        // =================================================================
        // Process samples (no parameter reads inside loop)
        // =================================================================
        let mut last_limiter_gr_db = 0.0;

        for mut channel_samples in buffer.iter_samples() {
            let input_sample = channel_samples.get_mut(0).copied().unwrap_or(0.0);

            // Apply filters before denoising
            let filtered_sample = if filter_enabled {
                self.filter.left.process(input_sample)
            } else {
                input_sample
            };

            // Process through streaming denoiser
            let mut output_sample = self.denoiser.left.process_sample(filtered_sample);

            // Apply Peak Compressor (mono uses left channel only)
            if peakcomp_enabled {
                output_sample = self.peakcomp.left.process(output_sample);
            }

            // Apply FixEq (uses process_mono for mono input)
            output_sample = self.fixeq.process(output_sample);

            // Apply De-Esser
            if deesser_enabled {
                output_sample = self.deesser.left.process(output_sample);
            }

            // Apply Channel9
            if channel9_enabled {
                output_sample = self.channel9.left.process(output_sample);
            }

            // Apply EnhanceEQ
            if enhance_eq_enabled {
                output_sample = self.enhance_eq.left.process(output_sample);
            }

            // Apply ButterComp
            if buttercomp_enabled {
                output_sample = self.buttercomp.left.process(output_sample);
            }

            // Apply TapeGlue
            if tape_glue_enabled {
                output_sample = self.tape_glue.left.process(output_sample);
            }

            // Apply Limiter (final stage)
            if limiter_enabled {
                let (out, _, gr) = self.limiter.process(output_sample, output_sample);
                output_sample = out;
                last_limiter_gr_db = gr;
            }

            if let Some(sample) = channel_samples.get_mut(0) {
                *sample = output_sample;
            }
        }

        // =================================================================
        // Update visualization ONCE after processing buffer
        // =================================================================
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
            if let Ok(mut gain) = self.peakcomp_gain_db.try_lock() {
                *gain = self.peakcomp.get_gain_reduction_db();
            }
            if let Ok(mut gain) = self.limiter_gain_db.try_lock() {
                *gain = last_limiter_gr_db;
            }
        }
    }

    fn process_stereo_lr(&mut self, buffer: &mut Buffer) {
        // =================================================================
        // Read ALL parameters ONCE per buffer (not per sample!)
        // =================================================================
        let viz_enabled = self.params.editor_state.is_open();
        let filter_enabled = self.params.filters.enable.value();

        // Peak Compressor params
        let peakcomp_enabled = self.params.peakcomp.enable.value();
        let peakcomp_threshold = self.params.peakcomp.threshold.value();
        let peakcomp_ratio = self.params.peakcomp.ratio.value();
        let peakcomp_attack = self.params.peakcomp.attack.value();
        let peakcomp_release = self.params.peakcomp.release.value();

        // FixEq params - De-mud
        let demud_enabled = self.params.demud.enable.value();
        let demud_freq = self.params.demud.frequency.value();
        let demud_strength = if demud_enabled {
            self.params.demud.macro_val.value()
        } else {
            0.0
        };

        // FixEq params - Correction A
        let corr_a_enabled = self.params.correction_a.enable.value();
        let corr_a_freq = self.params.correction_a.frequency.value();
        let corr_a_strength = if corr_a_enabled {
            self.params.correction_a.macro_val.value()
        } else {
            0.0
        };

        // FixEq params - Correction B
        let corr_b_enabled = self.params.correction_b.enable.value();
        let corr_b_freq = self.params.correction_b.frequency.value();
        let corr_b_strength = if corr_b_enabled {
            self.params.correction_b.macro_val.value()
        } else {
            0.0
        };

        // De-Esser params
        let deesser_enabled = self.params.deesser.enable.value();
        let deesser_freq = self.params.deesser.frequency.value();
        let deesser_q = self.params.deesser.q.value();
        let deesser_strength = self.params.deesser.strength.value();

        // Channel9 params
        let channel9_enabled = self.params.channel9.enable.value();
        let channel9_drive = self.params.channel9.drive.value();

        // EnhanceEQ params
        let enhance_eq_enabled = self.params.enhance_eq.enable.value();
        let enhance_lowmid_freq = self.params.enhance_eq.lowmid_freq.value();
        let enhance_lowmid_gain = self.params.enhance_eq.lowmid_gain.value();
        let enhance_presence_freq = self.params.enhance_eq.presence_freq.value();
        let enhance_presence_gain = self.params.enhance_eq.presence_gain.value();
        let enhance_air_gain = self.params.enhance_eq.air_gain.value();

        // ButterComp params
        let buttercomp_enabled = self.params.buttercomp.enable.value();
        let buttercomp_compress = self.params.buttercomp.compress.value();

        // TapeGlue params
        let tape_glue_enabled = self.params.tape_glue.enable.value();
        let tape_glue_warmth = self.params.tape_glue.warmth.value() as f64;

        // Limiter params
        let limiter_enabled = self.params.limiter.enable.value();

        // =================================================================
        // Sync params to processors ONCE per buffer
        // =================================================================
        self.denoiser.left.set_visualization_enabled(viz_enabled);
        self.denoiser.right.set_visualization_enabled(false);

        // Peak Compressor (linked stereo - set both channels)
        if peakcomp_enabled {
            self.peakcomp.set_threshold(peakcomp_threshold);
            self.peakcomp.set_ratio(peakcomp_ratio);
            self.peakcomp.set_attack(peakcomp_attack);
            self.peakcomp.set_release(peakcomp_release);
        }

        // FixEq (single instance, stereo mode)
        self.fixeq.set_stereo(true);
        self.fixeq.set_demud_enabled(demud_enabled);
        if demud_enabled {
            self.fixeq.set_demud_frequency(demud_freq);
        }
        self.fixeq.set_demud_strength(demud_strength);

        self.fixeq.set_correction_a_enabled(corr_a_enabled);
        if corr_a_enabled {
            self.fixeq.set_correction_a_frequency(corr_a_freq);
        }
        self.fixeq.set_correction_a_strength(corr_a_strength);

        self.fixeq.set_correction_b_enabled(corr_b_enabled);
        if corr_b_enabled {
            self.fixeq.set_correction_b_frequency(corr_b_freq);
        }
        self.fixeq.set_correction_b_strength(corr_b_strength);

        // De-Esser (both channels via set_both)
        if deesser_enabled {
            self.deesser.set_both(|d| {
                d.set_frequency(deesser_freq);
                d.set_q(deesser_q);
                d.set_strength(deesser_strength);
            });
        }

        // Channel9 (both channels)
        if channel9_enabled {
            self.channel9.set_both(|c| c.set_drive(channel9_drive));
        }

        // EnhanceEQ (uses StereoEnhanceEq setters)
        if enhance_eq_enabled {
            self.enhance_eq.set_lowmid_freq(enhance_lowmid_freq);
            self.enhance_eq.set_lowmid_gain(enhance_lowmid_gain);
            self.enhance_eq.set_presence_freq(enhance_presence_freq);
            self.enhance_eq.set_presence_gain(enhance_presence_gain);
            self.enhance_eq.set_shelf_gain(enhance_air_gain);
        }

        // ButterComp (both channels)
        if buttercomp_enabled {
            self.buttercomp.set_both(|c| c.set_compress(buttercomp_compress));
        }

        // TapeGlue (both channels)
        if tape_glue_enabled {
            self.tape_glue.set_both(|t| t.set_warmth(tape_glue_warmth));
        }

        // =================================================================
        // Process samples (no parameter reads inside loop)
        // =================================================================
        let mut last_limiter_gr_db = 0.0;

        for mut channel_samples in buffer.iter_samples() {
            let left_in = channel_samples.get_mut(0).copied().unwrap_or(0.0);
            let right_in = channel_samples.get_mut(1).copied().unwrap_or(0.0);

            // Apply filters before denoising
            let filtered_left = if filter_enabled {
                self.filter.left.process(left_in)
            } else {
                left_in
            };
            let filtered_right = if filter_enabled {
                self.filter.right.process(right_in)
            } else {
                right_in
            };

            // Process through streaming denoiser
            let mut left_out = self.denoiser.left.process_sample(filtered_left);
            let mut right_out = self.denoiser.right.process_sample(filtered_right);

            // Apply Peak Compressor (linked stereo detection)
            if peakcomp_enabled {
                (left_out, right_out) = self.peakcomp.process_sample_stereo(left_out, right_out);
            }

            // Apply FixEq (stereo mode, uses separate L/R bands)
            left_out = self.fixeq.process_sample_left(left_out);
            right_out = self.fixeq.process_sample_right(right_out);

            // Apply De-Esser
            if deesser_enabled {
                left_out = self.deesser.left.process(left_out);
                right_out = self.deesser.right.process(right_out);
            }

            // Apply Channel9
            if channel9_enabled {
                left_out = self.channel9.left.process(left_out);
                right_out = self.channel9.right.process(right_out);
            }

            // Apply EnhanceEQ
            if enhance_eq_enabled {
                left_out = self.enhance_eq.left.process(left_out);
                right_out = self.enhance_eq.right.process(right_out);
            }

            // Apply ButterComp
            if buttercomp_enabled {
                left_out = self.buttercomp.left.process(left_out);
                right_out = self.buttercomp.right.process(right_out);
            }

            // Apply TapeGlue
            if tape_glue_enabled {
                left_out = self.tape_glue.left.process(left_out);
                right_out = self.tape_glue.right.process(right_out);
            }

            // Apply Limiter (final stage)
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

        // =================================================================
        // Update visualization ONCE after processing buffer
        // =================================================================
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
            if let Ok(mut gain) = self.peakcomp_gain_db.try_lock() {
                *gain = self.peakcomp.get_gain_reduction_db();
            }
            if let Ok(mut gain) = self.limiter_gain_db.try_lock() {
                *gain = last_limiter_gr_db;
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
