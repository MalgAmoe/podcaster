//! Common constants and presets for de-reverb processing

use std::f32::consts::PI;

// =============================================================================
// Shared Constants (same as denoiser for compatibility)
// =============================================================================

pub const WINDOW_SIZE: usize = 2048;
pub const HOP_SIZE: usize = 1024;
pub const EPSILON: f32 = 1e-10;
pub const N_BINS: usize = WINDOW_SIZE / 2 + 1;

// =============================================================================
// De-Reverb Parameters
// =============================================================================

/// De-reverb processing parameters
#[derive(Clone, Debug)]
pub struct DeReverbParams {
    /// Overall reduction strength (0.0 = bypass, 1.0 = full)
    pub strength: f32,
    /// Gate threshold in dB (floor for gating)
    pub gate_threshold_db: f32,
    /// Temporal smoothing factor (0.0-1.0, higher = smoother)
    pub smoothing: f32,
    /// Decay rate multiplier (1.0 = use analyzed RT60, >1 = assume faster decay)
    pub decay_multiplier: f32,
}

impl Default for DeReverbParams {
    fn default() -> Self {
        Self {
            strength: 0.7,
            gate_threshold_db: -18.0,
            smoothing: 0.8,
            decay_multiplier: 1.0,
        }
    }
}

impl DeReverbParams {
    /// Create params from preset level (1-5)
    pub fn from_preset(level: u8) -> Option<Self> {
        if level == 0 || level > 5 {
            None
        } else {
            DEREVERB_PRESETS.get((level - 1) as usize).cloned()
        }
    }
}

// =============================================================================
// Presets (1-5, matching denoiser pattern)
// =============================================================================

/// Preset definitions
pub const DEREVERB_PRESETS: [DeReverbParams; 5] = [
    // 1: Gentle - minimal de-reverb, preserve room character
    DeReverbParams {
        strength: 0.3,
        gate_threshold_db: -30.0,
        smoothing: 0.9,
        decay_multiplier: 0.8,
    },
    // 2: Light - subtle reduction
    DeReverbParams {
        strength: 0.5,
        gate_threshold_db: -24.0,
        smoothing: 0.85,
        decay_multiplier: 1.0,
    },
    // 3: Moderate - balanced (default)
    DeReverbParams {
        strength: 0.7,
        gate_threshold_db: -18.0,
        smoothing: 0.8,
        decay_multiplier: 1.2,
    },
    // 4: Strong - noticeable reduction
    DeReverbParams {
        strength: 0.85,
        gate_threshold_db: -15.0,
        smoothing: 0.75,
        decay_multiplier: 1.5,
    },
    // 5: Aggressive - maximum removal
    DeReverbParams {
        strength: 1.0,
        gate_threshold_db: -12.0,
        smoothing: 0.7,
        decay_multiplier: 2.0,
    },
];

/// Preset names for display
pub const PRESET_NAMES: [&str; 5] = ["Gentle", "Light", "Moderate", "Strong", "Aggressive"];

/// Get preset name by level (1-5)
pub fn get_preset_name(level: u8) -> &'static str {
    PRESET_NAMES
        .get(level.saturating_sub(1) as usize)
        .unwrap_or(&"Unknown")
}

// =============================================================================
// Window Functions
// =============================================================================

/// Create sqrt-Hann window for overlap-add
pub fn create_sqrt_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let hann = 0.5 * (1.0 - (2.0 * PI * i as f32 / (size - 1) as f32).cos());
            hann.sqrt()
        })
        .collect()
}

// =============================================================================
// Utility Functions
// =============================================================================

/// Convert dB to linear gain
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

/// Convert linear gain to dB
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    20.0 * (linear.max(EPSILON)).log10()
}
