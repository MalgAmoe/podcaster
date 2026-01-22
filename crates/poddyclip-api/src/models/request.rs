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
    Repair,
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
            Some("repair") => ProcessingMode::Repair,
            Some("studio") => ProcessingMode::Studio,
            _ => ProcessingMode::Natural,
        };
        let strength = strength.unwrap_or(2).clamp(1, 3);

        Self::build_config(category, mode, strength)
    }

    fn build_config(category: Category, mode: ProcessingMode, strength: u8) -> Self {
        let mut c = Self::default();

        // Don't use chain presets - we're building from scratch
        c.chain = None;

        // Category settings - affects filter slopes
        match category {
            Category::Voice => {
                c.filters_enabled = true;
                c.hp_slope = 24; // Steeper for voice isolation
            }
            Category::Mixed => {
                c.filters_enabled = true;
                c.hp_slope = 12; // Gentler for music content
            }
        }

        // Mode settings - determines processing approach
        match mode {
            ProcessingMode::Repair => {
                // Heavy cleanup, minimal coloring
                c.denoiser_preset = strength;
                c.dereverb = strength;
                c.spectral_gate = strength;
                c.declick = true;
                c.depeak = true;
                c.depeak_max_db = 18.0 - (strength as f32 * 2.0); // More aggressive at higher strength

                c.expander_enabled = true;
                c.expander_preset = strength;

                c.compressor_enabled = true;
                c.compressor_type = CompressorType::Peak;
                c.compressor_preset = strength;

                c.fixeq_enabled = true;
                c.fixeq_preset = strength;
                c.deesser_enabled = true;

                // No saturation/enhancement in repair mode
                c.saturation_enabled = false;
                c.tape_enabled = false;
                c.buttercomp_enabled = false;
                c.enhanceeq_enabled = false;

                c.output_enabled = true;
                c.lufs_target = -18.0; // More headroom for repaired audio
                c.radio = false;
            }
            ProcessingMode::Natural => {
                // Balanced processing - clean but not sterile
                c.denoiser_preset = strength;
                c.dereverb = 0; // Off
                c.spectral_gate = 0;
                c.declick = false;
                c.depeak = false;

                c.expander_enabled = true;
                c.expander_preset = strength;

                c.compressor_enabled = true;
                c.compressor_type = CompressorType::Peak;
                c.compressor_preset = strength;

                c.fixeq_enabled = true;
                c.fixeq_preset = strength;
                c.deesser_enabled = true;

                // Light saturation at higher strengths (enabled at 2 and 3)
                c.saturation_enabled = strength >= 2;
                c.saturation_preset = strength;
                c.tape_enabled = false;
                c.buttercomp_enabled = strength >= 2;
                c.buttercomp_preset = strength;

                c.enhanceeq_enabled = true;
                c.enhanceeq_preset = strength;

                c.output_enabled = true;
                c.lufs_target = -16.0;
                c.radio = false;
            }
            ProcessingMode::Studio => {
                // Full polish, rich sound
                c.denoiser_preset = strength;
                c.dereverb = 0;
                c.spectral_gate = 0;
                c.declick = false;
                c.depeak = false;

                c.expander_enabled = true;
                c.expander_preset = strength;

                c.compressor_enabled = true;
                c.compressor_type = CompressorType::Fet; // Character compression
                c.compressor_preset = strength;

                c.fixeq_enabled = true;
                c.fixeq_preset = strength;
                c.deesser_enabled = true;

                // Full saturation chain for warmth
                c.saturation_enabled = true;
                c.saturation_preset = strength;
                c.tape_enabled = true;
                c.tape_preset = strength;
                c.buttercomp_enabled = true;
                c.buttercomp_preset = strength;

                c.enhanceeq_enabled = true;
                c.enhanceeq_preset = strength;

                c.output_enabled = true;
                c.lufs_target = -14.0; // Louder, broadcast-style
                c.radio = false;
            }
        }

        c
    }
}
