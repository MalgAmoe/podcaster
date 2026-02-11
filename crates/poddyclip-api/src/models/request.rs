use serde::{Deserialize, Serialize};

/// Output audio format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Wav,
    #[default]
    Mp3,
}

/// Configuration for audio processing - mirrors CLI arguments
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessConfig {
    // Output format
    pub output_format: OutputFormat,
    pub mp3_bitrate: u32,

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
            denoiser_preset: 1,
            dereverb: 0,
            spectral_gate: 0,
            filters_enabled: true,
            hp_slope: 24,
            hp_cutoff: 90.0,
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
            ai_denoise: false,
        }
    }
}

impl ProcessConfig {
    /// Build a ProcessConfig from a strength parameter (1-3).
    /// Voice-only processing pipeline.
    pub fn from_strength(strength: Option<u8>) -> Self {
        let strength = strength.unwrap_or(2).clamp(1, 3);

        Self {
            output_format: OutputFormat::Mp3,
            mp3_bitrate: 192,

            // Filters
            filters_enabled: true,
            hp_slope: 24,
            hp_cutoff: 75.0,

            // Noise reduction
            denoiser_preset: strength,
            dereverb: if strength == 3 { 3 } else { strength - 1 },
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
        }
    }
}
