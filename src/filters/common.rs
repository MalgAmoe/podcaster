//! Common filter implementations shared between CLI and plugin
//! Uses State Variable Filters (SVF) for numerical stability

use std::f32::consts::PI;

#[cfg(feature = "plugin")]
use nih_plug::prelude::Enum;

// =============================================================================
// Constants
// =============================================================================

/// Fixed high-pass frequency (removes rumble and plosives)
pub const HP_FREQUENCY: f32 = 80.0;

/// Fixed low-pass frequency (removes hiss and aliasing)
pub const LP_FREQUENCY: f32 = 15500.0;

/// Quality factor for filters (0.707 = Butterworth response)
const Q_BUTTERWORTH: f32 = 0.70710678;

/// Small epsilon to prevent denormal numbers
const DENORMAL_OFFSET: f32 = 1.0e-25;

// =============================================================================
// 2nd Order State Variable Filter (SVF)
// =============================================================================

/// Single 2nd order SVF biquad section
/// Provides simultaneous LP, BP, HP outputs with numerical stability
#[derive(Clone, Debug)]
pub struct SvfBiquad {
    // State variables (integrated state for trapezoidal SVF)
    ic1eq: f32, // Integrator 1 state
    ic2eq: f32, // Integrator 2 state

    // Optimized Coefficients (Andrew Simper form)
    a1: f32,
    a2: f32,
    a3: f32,

    // Standard params stored for updates
    g: f32,
    k: f32,
}

impl SvfBiquad {
    /// Create a new SVF biquad
    pub fn new(cutoff_hz: f32, sample_rate: f32, q_factor: f32) -> Self {
        let mut filter = Self {
            ic1eq: 0.0,
            ic2eq: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            g: 0.0,
            k: 0.0,
        };
        filter.update(cutoff_hz, sample_rate, q_factor);
        filter
    }

    /// Update coefficients based on new parameters
    pub fn update(&mut self, cutoff_hz: f32, sample_rate: f32, q_factor: f32) {
        // Pre-warp frequency
        self.g = (PI * cutoff_hz / sample_rate).tan();

        // Damping: k = 1/Q
        self.k = 1.0 / q_factor;

        // Calculate optimized coefficients
        // Reference: Andrew Simper, "Linear Trapezoidal Integrated SVF"
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    /// Process one sample through the filter
    /// Returns (low_pass, band_pass, high_pass)
    #[inline]
    pub fn process(&mut self, input: f32) -> (f32, f32, f32) {
        // 1. Anti-Denormal: Add tiny DC offset to input to keep FP unit active
        // Alternatively, use proper CPU flags (DAZ/FTZ) in the audio thread setup.
        let v0 = input + DENORMAL_OFFSET;

        // 2. Solve the linear system
        let v3 = v0 - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;

        // 3. Update state variables
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        // 4. Calculate outputs
        let lp = v2;
        let bp = v1;
        let hp = v0 - self.k * v1 - v2;

        (lp, bp, hp)
    }

    /// Reset filter state to zero (e.g., on playback stop/seek)
    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }
}

// =============================================================================
// High Pass Filter (12 dB/oct or 24 dB/oct)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "plugin", derive(Enum))]
pub enum HighPassSlope {
    /// 12 dB/octave (1 biquad)
    #[cfg_attr(feature = "plugin", name = "12 dB/oct")]
    Slope12dB,
    /// 24 dB/octave (2 biquads in series)
    #[cfg_attr(feature = "plugin", name = "24 dB/oct")]
    Slope24dB,
}

#[derive(Clone, Debug)]
pub struct HighPassFilter {
    stage1: SvfBiquad,
    stage2: Option<SvfBiquad>,
    #[cfg_attr(feature = "cli", allow(dead_code))]
    sample_rate: f32,
}

impl HighPassFilter {
    pub fn new(sample_rate: f32, slope: HighPassSlope) -> Self {
        let stage1 = SvfBiquad::new(HP_FREQUENCY, sample_rate, Q_BUTTERWORTH);
        let stage2 = match slope {
            HighPassSlope::Slope12dB => None,
            HighPassSlope::Slope24dB => {
                Some(SvfBiquad::new(HP_FREQUENCY, sample_rate, Q_BUTTERWORTH))
            }
        };

        Self { stage1, stage2, sample_rate }
    }

    /// Must be called if the host changes sample rate
    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn set_sample_rate(&mut self, new_rate: f32) {
        if (self.sample_rate - new_rate).abs() > 0.1 {
            self.sample_rate = new_rate;
            self.stage1.update(HP_FREQUENCY, new_rate, Q_BUTTERWORTH);
            if let Some(s2) = &mut self.stage2 {
                s2.update(HP_FREQUENCY, new_rate, Q_BUTTERWORTH);
            }
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let (_, _, hp1) = self.stage1.process(input);

        if let Some(stage2) = &mut self.stage2 {
            let (_, _, hp2) = stage2.process(hp1);
            hp2
        } else {
            hp1
        }
    }

    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn reset(&mut self) {
        self.stage1.reset();
        if let Some(stage2) = &mut self.stage2 {
            stage2.reset();
        }
    }
}

// =============================================================================
// Low Pass Filter (12 dB/oct)
// =============================================================================

#[derive(Clone, Debug)]
pub struct LowPassFilter {
    biquad: SvfBiquad,
    #[cfg_attr(feature = "cli", allow(dead_code))]
    sample_rate: f32,
}

impl LowPassFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            biquad: SvfBiquad::new(LP_FREQUENCY, sample_rate, Q_BUTTERWORTH),
            sample_rate,
        }
    }

    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn set_sample_rate(&mut self, new_rate: f32) {
        if (self.sample_rate - new_rate).abs() > 0.1 {
            self.sample_rate = new_rate;
            self.biquad.update(LP_FREQUENCY, new_rate, Q_BUTTERWORTH);
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let (lp, _, _) = self.biquad.process(input);
        lp
    }

    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn reset(&mut self) {
        self.biquad.reset();
    }
}

// =============================================================================
// Filter Chain (HP + LP)
// =============================================================================

#[derive(Clone, Debug)]
pub struct FilterChain {
    hp: HighPassFilter,
    lp: LowPassFilter,
}

impl FilterChain {
    pub fn new(sample_rate: f32, hp_slope: HighPassSlope) -> Self {
        Self {
            hp: HighPassFilter::new(sample_rate, hp_slope),
            lp: LowPassFilter::new(sample_rate),
        }
    }

    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.hp.set_sample_rate(sample_rate);
        self.lp.set_sample_rate(sample_rate);
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let after_hp = self.hp.process(input);
        self.lp.process(after_hp)
    }

    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn reset(&mut self) {
        self.hp.reset();
        self.lp.reset();
    }
}

// =============================================================================
// Stereo Chain
// =============================================================================

#[derive(Clone, Debug)]
#[cfg_attr(not(feature = "cli"), allow(dead_code))]
pub struct StereoFilterChain {
    left: FilterChain,
    right: FilterChain,
}

#[cfg_attr(not(feature = "cli"), allow(dead_code))]
impl StereoFilterChain {
    pub fn new(sample_rate: f32, hp_slope: HighPassSlope) -> Self {
        Self {
            left: FilterChain::new(sample_rate, hp_slope),
            right: FilterChain::new(sample_rate, hp_slope),
        }
    }

    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.left.set_sample_rate(sample_rate);
        self.right.set_sample_rate(sample_rate);
    }

    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.left.process(*sample);
        }
    }

    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l = self.left.process(*l);
            *r = self.right.process(*r);
        }
    }

    #[cfg_attr(feature = "cli", allow(dead_code))]
    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}
