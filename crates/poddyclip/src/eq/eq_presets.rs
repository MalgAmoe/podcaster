//! EQ presets for EnhanceEQ gains

/// EQ preset parameters (for EnhanceEQ scaling)
#[derive(Clone, Copy, Debug)]
pub struct EqPreset {
    /// Low-mid cut in dB (negative = cut)
    pub lowmid_cut_db: f32,
    /// Presence boost in dB
    pub presence_boost_db: f32,
    /// Air boost in dB
    pub air_boost_db: f32,
}

/// Preset names (1-5)
pub const EQ_PRESET_NAMES: [&str; 5] = ["Gentle", "Light", "Moderate", "Strong", "Aggressive"];

/// EQ presets (1-5 scale) - applied as gain scaling for EnhanceEQ
pub const EQ_PRESETS: [EqPreset; 5] = [
    // 1: Gentle - minimal shaping
    EqPreset {
        lowmid_cut_db: -1.0,
        presence_boost_db: 1.0,
        air_boost_db: 1.0,
    },
    // 2: Light
    EqPreset {
        lowmid_cut_db: -2.0,
        presence_boost_db: 2.0,
        air_boost_db: 2.0,
    },
    // 3: Moderate (current defaults)
    EqPreset {
        lowmid_cut_db: -3.0,
        presence_boost_db: 3.0,
        air_boost_db: 3.0,
    },
    // 4: Strong
    EqPreset {
        lowmid_cut_db: -4.0,
        presence_boost_db: 4.0,
        air_boost_db: 4.0,
    },
    // 5: Aggressive - broadcast style
    EqPreset {
        lowmid_cut_db: -5.0,
        presence_boost_db: 5.0,
        air_boost_db: 5.0,
    },
];

/// Get preset name by level (1-5), returns "Unknown" for invalid levels
pub fn get_eq_preset_name(level: u8) -> &'static str {
    EQ_PRESET_NAMES
        .get((level as usize).saturating_sub(1))
        .unwrap_or(&"Unknown")
}

/// Get EQ preset by level (1-5)
pub fn get_eq_preset(level: u8) -> Option<&'static EqPreset> {
    EQ_PRESETS.get((level as usize).saturating_sub(1))
}
