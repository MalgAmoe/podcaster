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
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Mp3,
            mp3_bitrate: 192,
            chain: None,
            denoiser_preset: 3,
            dereverb: 0,
            spectral_gate: 0,
            depeak: false,
            depeak_max_db: 18.0,
            filters_enabled: true,
            hp_slope: 24,
            declick: false,
            expander_enabled: true,
            expander_preset: 3,
            compressor_enabled: true,
            compressor_type: CompressorType::Peak,
            compressor_preset: 3,
            fixeq_enabled: true,
            fixeq_preset: 1,
            deesser_enabled: true,
            enhanceeq_enabled: true,
            enhanceeq_preset: 3,
            saturation_enabled: true,
            saturation_preset: 3,
            tape_enabled: true,
            tape_preset: 3,
            buttercomp_enabled: true,
            buttercomp_preset: 3,
            output_enabled: true,
            lufs_target: -16.0,
            radio: false,
            radio_amount: 1.0,
        }
    }
}
