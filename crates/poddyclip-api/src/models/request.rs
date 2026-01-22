use serde::{Deserialize, Serialize};

/// Output audio format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Wav,
    #[default]
    Mp3,
}

/// Compressor type selection
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CompressorType {
    #[default]
    Peak,
    Fet,
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
    Studio,
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
    pub depeak: bool,
    pub depeak_max_db: f32,

    // Filters
    pub filters_enabled: bool,
    pub hp_slope: u8,
    pub hp_cutoff: f32,

    // Repair
    pub declick: bool,

    // Dynamics
    pub expander_enabled: bool,
    pub expander_preset: u8,

    pub compressor_enabled: bool,
    pub compressor_type: CompressorType,
    pub compressor_preset: u8,

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
            denoiser_preset: 2,
            dereverb: 0,
            spectral_gate: 0,
            depeak: false,
            depeak_max_db: 18.0,
            filters_enabled: true,
            hp_slope: 24,
            hp_cutoff: 90.0, // Voice default
            declick: false,
            expander_enabled: true,
            expander_preset: 2,
            compressor_enabled: true,
            compressor_type: CompressorType::Peak,
            compressor_preset: 2,
            fixeq_enabled: true,
            fixeq_preset: 1,
            deesser_enabled: true,
            enhanceeq_enabled: true,
            enhanceeq_preset: 2,
            saturation_enabled: true,
            saturation_preset: 2,
            tape_enabled: true,
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
    pub fn from_dynamic(
        category: Option<&str>,
        mode: Option<&str>,
        strength: Option<u8>,
    ) -> Self {
        let category = match category {
            Some("mixed") => Category::Mixed,
            _ => Category::Voice,
        };
        let mode = match mode {
            Some("studio") => ProcessingMode::Studio,
            _ => ProcessingMode::Natural, // "repair" falls back to Natural
        };
        let strength = strength.unwrap_or(2).clamp(1, 3);

        Self::build_config(category, mode, strength)
    }

    fn build_config(category: Category, mode: ProcessingMode, strength: u8) -> Self {
        match (category, mode) {
            // =================================================================
            // VOICE CONFIGS
            // =================================================================
            (Category::Voice, ProcessingMode::Natural) => Self {
                output_format: OutputFormat::Mp3,
                mp3_bitrate: 192,
                chain: None,

                // Filters
                filters_enabled: true,
                hp_slope: 24,
                hp_cutoff: 90.0,

                // Noise reduction
                denoiser_preset: strength,
                dereverb: 0,
                spectral_gate: 0,
                declick: true,
                depeak: false,
                depeak_max_db: 18.0,
                ai_denoise: false,

                // Dynamics
                expander_enabled: true,
                expander_preset: strength,
                compressor_enabled: true,
                compressor_type: CompressorType::Peak,
                compressor_preset: strength,

                // EQ
                fixeq_enabled: true,
                fixeq_preset: strength,
                deesser_enabled: true,
                enhanceeq_enabled: true,
                enhanceeq_preset: strength,

                // Saturation
                saturation_enabled: strength >= 2,
                saturation_preset: strength,
                tape_enabled: false,
                tape_preset: strength,
                buttercomp_enabled: strength >= 2,
                buttercomp_preset: strength,

                // Output
                output_enabled: true,
                lufs_target: -16.0,
                radio: false,
                radio_amount: 1.0,
            },

            (Category::Voice, ProcessingMode::Studio) => Self {
                output_format: OutputFormat::Mp3,
                mp3_bitrate: 192,
                chain: None,

                // Filters
                filters_enabled: true,
                hp_slope: 24,
                hp_cutoff: 90.0,

                // Noise reduction
                denoiser_preset: strength,
                dereverb: 0,
                spectral_gate: 0,
                declick: true,
                depeak: false,
                depeak_max_db: 18.0,
                ai_denoise: false,

                // Dynamics
                expander_enabled: true,
                expander_preset: strength,
                compressor_enabled: true,
                compressor_type: CompressorType::Fet,
                compressor_preset: strength,

                // EQ
                fixeq_enabled: true,
                fixeq_preset: strength,
                deesser_enabled: true,
                enhanceeq_enabled: false,
                enhanceeq_preset: strength,

                // Saturation
                saturation_enabled: true,
                saturation_preset: strength,
                tape_enabled: false,
                tape_preset: strength,
                buttercomp_enabled: true,
                buttercomp_preset: strength,

                // Output
                output_enabled: true,
                lufs_target: -14.0,
                radio: true,
                radio_amount: 1.0,
            },

            // =================================================================
            // MIXED AUDIO CONFIGS (same as voice for now, different HP)
            // =================================================================
            (Category::Mixed, ProcessingMode::Natural) => Self {
                output_format: OutputFormat::Mp3,
                mp3_bitrate: 192,
                chain: None,

                // Filters - gentler for mixed content
                filters_enabled: true,
                hp_slope: 24,
                hp_cutoff: 65.0,

                // Noise reduction
                denoiser_preset: strength,
                dereverb: 0,
                spectral_gate: 0,
                declick: true,
                depeak: false,
                depeak_max_db: 18.0,
                ai_denoise: false,

                // Dynamics
                expander_enabled: true,
                expander_preset: strength,
                compressor_enabled: true,
                compressor_type: CompressorType::Peak,
                compressor_preset: strength,

                // EQ
                fixeq_enabled: true,
                fixeq_preset: strength,
                deesser_enabled: true,
                enhanceeq_enabled: true,
                enhanceeq_preset: strength,

                // Saturation
                saturation_enabled: strength >= 2,
                saturation_preset: strength,
                tape_enabled: false,
                tape_preset: strength,
                buttercomp_enabled: strength >= 2,
                buttercomp_preset: strength,

                // Output
                output_enabled: true,
                lufs_target: -16.0,
                radio: false,
                radio_amount: 1.0,
            },

            (Category::Mixed, ProcessingMode::Studio) => Self {
                output_format: OutputFormat::Mp3,
                mp3_bitrate: 192,
                chain: None,

                // Filters - gentler for mixed content
                filters_enabled: true,
                hp_slope: 24,
                hp_cutoff: 65.0,

                // Noise reduction
                denoiser_preset: strength,
                dereverb: 0,
                spectral_gate: 0,
                declick: true,
                depeak: false,
                depeak_max_db: 18.0,
                ai_denoise: false,

                // Dynamics
                expander_enabled: true,
                expander_preset: strength,
                compressor_enabled: true,
                compressor_type: CompressorType::Fet,
                compressor_preset: strength,

                // EQ
                fixeq_enabled: true,
                fixeq_preset: strength,
                deesser_enabled: true,
                enhanceeq_enabled: false,
                enhanceeq_preset: strength,

                // Saturation
                saturation_enabled: true,
                saturation_preset: strength,
                tape_enabled: false,
                tape_preset: strength,
                buttercomp_enabled: true,
                buttercomp_preset: strength,

                // Output
                output_enabled: true,
                lufs_target: -14.0,
                radio: false,
                radio_amount: 1.0,
            },
        }
    }
}
