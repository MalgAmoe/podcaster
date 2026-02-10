use serde::{Deserialize, Serialize};

/// Output audio format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Wav,
    #[default]
    Mp3,
}

/// Audio category - determines filter settings
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    #[default]
    Voice,
    Mixed,
}

/// Processing mode - determines the overall processing approach
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProcessingMode {
    #[default]
    Natural,
}

/// Configuration for audio processing - mirrors CLI arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessConfig {
    // Output format
    pub output_format: OutputFormat,
    pub mp3_bitrate: u32,

    // Chain preset (overrides individual settings)
    pub chain: Option<String>,

    // Denoiser (always enabled)
    pub denoiser_preset: u8,

    // Optional spectral processors
    pub dereverb: u8,
    pub spectral_gate: u8,

    // Filters
    pub filters_enabled: bool,
    pub hp_slope: u8,
    pub hp_cutoff: f32,

    // Repair
    pub declick: bool,

    // Dynamics
    pub peakcomp_enabled: bool,
    pub peakcomp_preset: u8,
    pub fetcomp_enabled: bool,
    pub fetcomp_preset: u8,

    // EQ
    pub fixeq_enabled: bool,
    pub fixeq_preset: u8,
    pub deesser_enabled: bool,
    pub enhanceeq_enabled: bool,
    pub enhanceeq_preset: u8,

    // Saturation
    pub saturation_enabled: bool,
    pub saturation_preset: u8,
    pub tape_enabled: bool,
    pub tape_preset: u8,

    // Compression
    pub buttercomp_enabled: bool,
    pub buttercomp_preset: u8,

    // Output
    pub output_enabled: bool,
    pub lufs_target: f32,

    // Radio voice EQ
    pub radio: bool,
    pub radio_amount: f32,

    // AI Denoise (DeepFilterNet) - voice-focused
    pub ai_denoise: bool,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Mp3,
            mp3_bitrate: 192,
            chain: None,
            denoiser_preset: 1,
            dereverb: 0,
            spectral_gate: 0,
            filters_enabled: true,
            hp_slope: 24,
            hp_cutoff: 90.0, // Voice default
            declick: false,
            peakcomp_enabled: true,
            peakcomp_preset: 2,
            fetcomp_enabled: false,
            fetcomp_preset: 2,
            fixeq_enabled: true,
            fixeq_preset: 1,
            deesser_enabled: true,
            enhanceeq_enabled: false,
            enhanceeq_preset: 2,
            saturation_enabled: true,
            saturation_preset: 2,
            tape_enabled: false,
            tape_preset: 2,
            buttercomp_enabled: true,
            buttercomp_preset: 2,
            output_enabled: true,
            lufs_target: -16.0,
            radio: false,
            radio_amount: 1.0,
            ai_denoise: false, // Off by default, enabled in Repair mode
        }
    }
}

impl ProcessConfig {
    /// Build a ProcessConfig from dynamic category/mode/strength parameters.
    /// This replaces the chain preset system with a more intuitive UI.
    pub fn from_dynamic(category: Option<&str>, strength: Option<u8>) -> Self {
        let category = match category {
            Some("mixed") => Category::Mixed,
            _ => Category::Voice,
        };
        let _mode = ProcessingMode::Natural; // Only natural mode supported
        let strength = strength.unwrap_or(2).clamp(1, 3);

        Self::build_config(category, strength)
    }

    fn build_config(category: Category, strength: u8) -> Self {
        match category {
            // =================================================================
            // VOICE CONFIG
            // =================================================================
            Category::Voice => Self {
                output_format: OutputFormat::Mp3,
                mp3_bitrate: 192,
                chain: None,

                // Filters
                filters_enabled: true,
                hp_slope: 24,
                hp_cutoff: 90.0,

                // Noise reduction
                denoiser_preset: strength,
                dereverb: if strength == 3 { 3 } else { strength - 1},
                spectral_gate: 0,
                declick: strength > 1,
                ai_denoise: false,

                // Dynamics
                peakcomp_enabled: true,
                peakcomp_preset: strength,
                fetcomp_enabled: true,
                fetcomp_preset: strength,

                // EQ
                fixeq_enabled: strength >= 2,
                fixeq_preset: 1,
                deesser_enabled: true,
                enhanceeq_enabled: true,
                enhanceeq_preset: strength,

                // Saturation
                saturation_enabled: strength == 2,
                saturation_preset: 1,
                tape_enabled: strength == 3,
                tape_preset: 2,
                buttercomp_enabled: strength > 1,
                buttercomp_preset: strength - 1,

                // Output
                output_enabled: true,
                lufs_target: -16.0,
                radio: strength == 3,
                radio_amount: 1.0,
            },

            // =================================================================
            // MIXED AUDIO CONFIG (gentler, different HP)
            // =================================================================
            Category::Mixed => Self {
                output_format: OutputFormat::Mp3,
                mp3_bitrate: 192,
                chain: None,

                // Filters - gentler for mixed content
                filters_enabled: true,
                hp_slope: 24,
                hp_cutoff: 65.0,

                // Noise reduction
                denoiser_preset: strength,
                dereverb: if strength >= 2 { strength - 1 } else { 0 },
                spectral_gate: strength - 1,
                declick: strength > 1,
                ai_denoise: false,

                // Dynamics
                peakcomp_enabled: strength >= 2,
                peakcomp_preset: strength - 1,
                fetcomp_enabled: strength >= 2,
                fetcomp_preset: strength - 1,

                // EQ
                fixeq_enabled: strength >= 2,
                fixeq_preset: 1,
                deesser_enabled: true,
                enhanceeq_enabled: true,
                enhanceeq_preset: 1,

                // Saturation
                saturation_enabled: strength == 1,
                saturation_preset: 1,
                tape_enabled: strength >= 2,
                tape_preset: strength - 1,
                buttercomp_enabled: strength == 3,
                buttercomp_preset: 1,

                // Output
                output_enabled: true,
                lufs_target: -16.0,
                radio: strength == 3,
                radio_amount: 0.3,
            },
        }
    }
}
