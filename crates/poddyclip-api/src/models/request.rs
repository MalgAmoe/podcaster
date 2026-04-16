use serde::{Deserialize, Serialize};

/// Output audio format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Wav,
    #[default]
    Mp3,
}

/// Configuration for audio processing
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessConfig {
    // Output format
    pub output_format: OutputFormat,
    pub mp3_bitrate: u32,

    // Filters
    pub filters_enabled: bool,
    pub hp_slope: u8,
    pub hp_cutoff: f32,

    // Noise reduction
    pub denoiser_preset: u8,
    pub declick: bool,
    pub ai_denoise: bool,

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

    // Output
    pub output_enabled: bool,
    pub lufs_target: f32,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessConfig {
    pub fn new() -> Self {
        Self {
            output_format: OutputFormat::Mp3,
            mp3_bitrate: 192,

            filters_enabled: true,
            hp_slope: 24,
            hp_cutoff: 75.0,

            denoiser_preset: 1,
            declick: true,
            ai_denoise: false,

            peakcomp_enabled: true,
            peakcomp_preset: 1,
            fetcomp_enabled: true,
            fetcomp_preset: 1,

            fixeq_enabled: true,
            fixeq_preset: 1,
            deesser_enabled: true,
            enhanceeq_enabled: true,
            enhanceeq_preset: 1,

            output_enabled: true,
            lufs_target: -16.0,
        }
    }
}
