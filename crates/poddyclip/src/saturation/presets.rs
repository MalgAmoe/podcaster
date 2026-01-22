//! Saturation presets for Channel9 and TapeGlue

/// Saturation preset parameters
#[derive(Clone, Copy, Debug)]
pub struct SaturationPreset {
    /// Channel9 drive (0.0-1.0)
    pub channel9_drive: f32,
    /// TapeGlue warmth amount (0.0-1.0)
    pub tape_warmth: f64,
}

/// Preset names (1-3)
pub const SATURATION_PRESET_NAMES: [&str; 3] =
    ["Subtle", "Balanced", "Intense"];

/// Saturation presets (1-3 scale)
pub const SATURATION_PRESETS: [SaturationPreset; 3] = [
    // 1: Subtle - barely colored
    SaturationPreset {
        channel9_drive: 0.1,
        tape_warmth: 0.2,
    },
    // 2: Balanced (default)
    SaturationPreset {
        channel9_drive: 0.15,
        tape_warmth: 0.35,
    },
    // 3: Intense - noticeable warmth
    SaturationPreset {
        channel9_drive: 0.2,
        tape_warmth: 0.5,
    },
];

/// Get preset name by level (1-3), returns "Unknown" for invalid levels
pub fn get_saturation_preset_name(level: u8) -> &'static str {
    SATURATION_PRESET_NAMES
        .get((level as usize).saturating_sub(1))
        .unwrap_or(&"Unknown")
}

/// Get saturation preset by level (1-3)
pub fn get_saturation_preset(level: u8) -> Option<&'static SaturationPreset> {
    SATURATION_PRESETS.get((level as usize).saturating_sub(1))
}
