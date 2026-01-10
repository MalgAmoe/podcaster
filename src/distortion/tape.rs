//! Tape Hysteresis - Simplified Jiles-Atherton magnetic hysteresis model
//!
//! Based on the physics described in:
//! "Real-Time Physical Modelling for Analog Tape Machines"
//! Jatin Chowdhury, DAFx 2019
//!
//! This is a simplified extraction of the core hysteresis algorithm
//! for subtle, high-quality tape saturation without wow/flutter/loss effects.

#![allow(dead_code)]

use std::f64::consts::PI;

/// Langevin function: L(x) = coth(x) - 1/x
/// Approximated for numerical stability
#[inline]
fn langevin(x: f64) -> f64 {
    let abs_x = x.abs();
    if abs_x < 1e-4 {
        // Taylor series for small x: L(x) ≈ x/3 - x³/45 + ...
        x / 3.0
    } else if abs_x > 10.0 {
        // For large |x|: coth(x) → sign(x), so L(x) → sign(x) - 1/x
        x.signum() - 1.0 / x
    } else {
        // Standard formula: coth(x) - 1/x
        let exp_2x = (2.0 * x).exp();
        let coth = (exp_2x + 1.0) / (exp_2x - 1.0);
        coth - 1.0 / x
    }
}

/// Derivative of Langevin function: L'(x) = 1/x² - csch²(x)
#[inline]
fn langevin_deriv(x: f64) -> f64 {
    let abs_x = x.abs();
    if abs_x < 1e-4 {
        // Taylor series: L'(x) ≈ 1/3 - x²/15 + ...
        1.0 / 3.0
    } else if abs_x > 10.0 {
        // For large |x|: approaches 1/x²
        1.0 / (x * x)
    } else {
        let csch = 1.0 / x.sinh();
        1.0 / (x * x) - csch * csch
    }
}

/// State for the hysteresis processor
#[derive(Clone, Debug)]
struct HysteresisState {
    /// Magnetization state
    m_n1: f64,
    /// Previous input field
    h_n1: f64,
    /// Previous input field derivative
    h_d_n1: f64,
}

impl Default for HysteresisState {
    fn default() -> Self {
        Self {
            m_n1: 0.0,
            h_n1: 0.0,
            h_d_n1: 0.0,
        }
    }
}

/// Jiles-Atherton hysteresis parameters
#[derive(Clone, Debug)]
struct HysteresisParams {
    /// Saturation magnetization
    m_s: f64,
    /// Shape parameter (anhysteretic)
    a: f64,
    /// Mean field parameter
    alpha: f64,
    /// Pinning coefficient
    k: f64,
    /// Reversibility coefficient (0-1)
    c: f64,
}

impl HysteresisParams {
    /// Create parameters for subtle tape saturation
    /// Values from AnalogTapeModel reference implementation
    fn for_subtle_tape() -> Self {
        Self {
            m_s: 1.0,
            a: 0.25,      // Shape factor - CRITICAL: must be small (~0.1-0.5)
            alpha: 1.6e-3, // Coupling constant (reference value)
            k: 0.47875,   // Reversibility coefficient (reference value)
            c: 0.17,      // Irreversibility coefficient (reference value)
        }
    }

    /// Compute the hysteresis derivative dM/dt
    #[inline]
    fn hysteresis_func(&self, m: f64, h: f64, h_d: f64) -> f64 {
        // Effective field
        let h_eff = h + self.alpha * m;

        // Prevent division by zero
        let a_safe = self.a.max(1e-6);

        // Anhysteretic magnetization
        let m_an = self.m_s * langevin(h_eff / a_safe);

        // Anhysteretic magnetization derivative
        let m_an_prime = self.m_s * langevin_deriv(h_eff / a_safe) / a_safe;

        // Delta M
        let delta_m = m_an - m;

        // Sign of dH/dt determines which branch we're on
        let delta_s = if h_d >= 0.0 { 1.0 } else { -1.0 };

        // Prevent instability when delta_m and delta_s have opposite signs
        let delta_m_safe = if delta_s * delta_m < 0.0 {
            0.0
        } else {
            delta_m
        };

        // k parameter adjusted by sign
        let k_delta = self.k * delta_s;

        // Denominator
        let denom = 1.0 - self.c * self.alpha * m_an_prime;
        let denom_safe = if denom.abs() < 1e-10 {
            1e-10 * denom.signum()
        } else {
            denom
        };

        // dM/dH
        let dm_dh = (delta_m_safe / (k_delta - self.alpha * delta_m_safe)
            + self.c * m_an_prime)
            / denom_safe;

        // dM/dt = dM/dH * dH/dt
        dm_dh * h_d
    }
}

/// Tape Hysteresis processor
///
/// Implements Jiles-Atherton magnetic hysteresis for subtle tape saturation.
/// Designed for use on mix buses where transparent coloration is desired.
#[derive(Clone, Debug)]
pub struct TapeHysteresis {
    sample_rate: f64,
    os_sample_rate: f64,

    // Processing state (per channel handled externally for stereo)
    state: HysteresisState,

    // Hysteresis parameters
    params: HysteresisParams,

    // Time step
    t: f64,

    // Oversampling
    os_factor: usize,

    // Oversampling filter state
    os_filter: OversamplingFilter,

    // User parameters
    drive: f64,      // 0.0 - 1.0
    saturation: f64, // 0.0 - 1.0
    bias: f64,       // 0.0 - 1.0
    makeup: f64,     // Automatic gain compensation
}

/// Simple 2x oversampling filter (half-band)
#[derive(Clone, Debug)]
struct OversamplingFilter {
    // Upsampling filter state
    up_z: [f64; 4],
    // Downsampling filter state
    down_z: [f64; 4],
}

impl Default for OversamplingFilter {
    fn default() -> Self {
        Self {
            up_z: [0.0; 4],
            down_z: [0.0; 4],
        }
    }
}

impl OversamplingFilter {
    /// Half-band filter coefficients for 2x oversampling
    /// Simple 4-tap FIR
    const COEFFS: [f64; 4] = [0.0625, 0.25, 0.375, 0.25];

    fn reset(&mut self) {
        self.up_z = [0.0; 4];
        self.down_z = [0.0; 4];
    }

    /// Upsample by 2x with interpolation
    #[inline]
    fn upsample(&mut self, input: f64) -> [f64; 2] {
        // Shift buffer
        self.up_z[3] = self.up_z[2];
        self.up_z[2] = self.up_z[1];
        self.up_z[1] = self.up_z[0];
        self.up_z[0] = input;

        // Interpolated sample
        let interp = self.up_z[0] * Self::COEFFS[0]
            + self.up_z[1] * Self::COEFFS[1]
            + self.up_z[2] * Self::COEFFS[2]
            + self.up_z[3] * Self::COEFFS[3];

        // Return [interpolated, original] - the original sample goes second
        // to maintain phase alignment
        [interp * 2.0, input]
    }

    /// Downsample by 2x with anti-aliasing
    #[inline]
    fn downsample(&mut self, samples: [f64; 2]) -> f64 {
        // Process both samples through lowpass
        self.down_z[3] = self.down_z[2];
        self.down_z[2] = self.down_z[1];
        self.down_z[1] = self.down_z[0];
        self.down_z[0] = samples[0];

        let _out1 = self.down_z[0] * Self::COEFFS[0]
            + self.down_z[1] * Self::COEFFS[1]
            + self.down_z[2] * Self::COEFFS[2]
            + self.down_z[3] * Self::COEFFS[3];

        self.down_z[3] = self.down_z[2];
        self.down_z[2] = self.down_z[1];
        self.down_z[1] = self.down_z[0];
        self.down_z[0] = samples[1];

        let out2 = self.down_z[0] * Self::COEFFS[0]
            + self.down_z[1] * Self::COEFFS[1]
            + self.down_z[2] * Self::COEFFS[2]
            + self.down_z[3] * Self::COEFFS[3];

        // Return second sample (maintains phase)
        out2
    }
}

impl TapeHysteresis {
    /// Create a new hysteresis processor
    pub fn new(sample_rate: f64) -> Self {
        let os_factor = 2;
        let os_sample_rate = sample_rate * os_factor as f64;

        let mut processor = Self {
            sample_rate,
            os_sample_rate,
            state: HysteresisState::default(),
            params: HysteresisParams::for_subtle_tape(),
            t: 1.0 / os_sample_rate,
            os_factor,
            os_filter: OversamplingFilter::default(),
            drive: 0.5,
            saturation: 0.5,
            bias: 0.5,
            makeup: 1.0,
        };
        processor.update_params();
        processor
    }

    /// Set drive amount (0.0 - 1.0)
    /// Higher values = more saturation
    pub fn set_drive(&mut self, drive: f64) {
        self.drive = drive.clamp(0.0, 1.0);
        self.update_params();
    }

    /// Set saturation amount (0.0 - 1.0)
    /// Controls the "hardness" of the saturation curve
    pub fn set_saturation(&mut self, saturation: f64) {
        self.saturation = saturation.clamp(0.0, 1.0);
        self.update_params();
    }

    /// Set bias (0.0 - 1.0)
    /// Lower values create a "deadzone" effect, higher values are cleaner
    pub fn set_bias(&mut self, bias: f64) {
        self.bias = bias.clamp(0.0, 1.0);
        self.update_params();
    }

    /// Update internal parameters based on user controls
    /// Scaling matches AnalogTapeModel reference implementation
    fn update_params(&mut self) {
        // Saturation affects M_s (0.5-2.0 range)
        // Higher saturation = lower M_s = more compression
        self.params.m_s = 0.5 + 1.5 * (1.0 - self.saturation);

        // Drive affects 'a' - CRITICAL formula from reference!
        // a = M_s / (0.01 + 6.0 * drive)
        // Higher drive = lower 'a' = more saturation
        // With drive=0.5: a ≈ 0.4, with drive=1.0: a ≈ 0.17
        self.params.a = self.params.m_s / (0.01 + 6.0 * self.drive);

        // Bias/width affects 'c' (irreversibility)
        // Higher bias = narrower hysteresis = cleaner sound
        let width = 1.0 - self.bias;
        self.params.c = (1.0 - width).sqrt().max(0.01) - 0.01;

        // k is fixed at reference value
        self.params.k = 0.47875;

        // No makeup gain needed with proper scaling
        self.makeup = 1.0;
    }

    /// Process using 4th-order Runge-Kutta solver
    #[inline]
    fn solve_rk4(&mut self, h: f64, h_d: f64) -> f64 {
        let t = self.t;
        let m_n1 = self.state.m_n1;
        let h_n1 = self.state.h_n1;
        let h_d_n1 = self.state.h_d_n1;

        // Interpolated values at midpoint
        let h_1_2 = (h + h_n1) * 0.5;
        let h_d_1_2 = (h_d + h_d_n1) * 0.5;

        // RK4 stages
        let k1 = self.params.hysteresis_func(m_n1, h_n1, h_d_n1) * t;
        let k2 = self.params.hysteresis_func(m_n1 + k1 * 0.5, h_1_2, h_d_1_2) * t;
        let k3 = self.params.hysteresis_func(m_n1 + k2 * 0.5, h_1_2, h_d_1_2) * t;
        let k4 = self.params.hysteresis_func(m_n1 + k3, h, h_d) * t;

        // Combine
        const ONE_SIXTH: f64 = 1.0 / 6.0;
        const ONE_THIRD: f64 = 1.0 / 3.0;

        let m = m_n1 + k1 * ONE_SIXTH + k2 * ONE_THIRD + k3 * ONE_THIRD + k4 * ONE_SIXTH;

        // Update state
        self.state.m_n1 = m;
        self.state.h_n1 = h;
        self.state.h_d_n1 = h_d;

        m
    }

    /// Process a single sample at the oversampled rate
    #[inline]
    fn process_sample_os(&mut self, input: f64) -> f64 {
        // Input is already the magnetic field H (audio-level, no extra scaling)
        let h = input;

        // Compute derivative (difference from last sample, scaled by sample rate)
        let h_d = (h - self.state.h_n1) * self.os_sample_rate;

        // Solve hysteresis - output is magnetization (similar level to input)
        self.solve_rk4(h, h_d)
    }

    /// Process a single sample (with 2x oversampling)
    pub fn process(&mut self, input: f32) -> f32 {
        let input = input as f64;

        // Upsample
        let os_samples = self.os_filter.upsample(input);

        // Process at oversampled rate
        let processed = [
            self.process_sample_os(os_samples[0]),
            self.process_sample_os(os_samples[1]),
        ];

        // Downsample
        let output = self.os_filter.downsample(processed);

        // Soft clip to prevent any nasty surprises
        let output = (output * 0.9).tanh() / 0.9_f64.tanh();

        output as f32
    }

    /// Process a mono buffer in-place
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Reset all state
    pub fn reset(&mut self) {
        self.state = HysteresisState::default();
        self.os_filter.reset();
    }
}

impl crate::traits::AudioProcessor for TapeHysteresis {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for TapeHysteresis {}

impl crate::traits::ProcessorF64 for TapeHysteresis {
    fn new(sample_rate: f64) -> Self {
        Self::new(sample_rate)
    }
}

// ============================================================================
// Simplified "Tape Glue" variant for very subtle use
// ============================================================================

/// Ultra-subtle tape saturation for podcast/broadcast use
///
/// This is a simplified version that prioritizes transparency
/// over full physical accuracy. Good for "set and forget" on a mix bus.
#[derive(Clone, Debug)]
pub struct TapeGlue {
    sample_rate: f64,

    // Simplified state
    last_sample: f64,
    dc_blocker_x: f64,
    dc_blocker_y: f64,

    // Parameters
    warmth: f64, // 0.0 - 1.0
}

impl TapeGlue {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            last_sample: 0.0,
            dc_blocker_x: 0.0,
            dc_blocker_y: 0.0,
            warmth: 0.3, // Default to subtle
        }
    }

    /// Set warmth amount (0.0 - 1.0)
    /// 0.0 = bypass, 1.0 = full effect (still subtle)
    pub fn set_warmth(&mut self, warmth: f64) {
        self.warmth = warmth.clamp(0.0, 1.0);
    }

    /// Soft saturation curve inspired by tape
    #[inline]
    fn soft_saturate(x: f64, amount: f64) -> f64 {
        if amount < 0.001 {
            return x;
        }

        // Blend between linear and tanh-like curve
        let sat = x / (1.0 + x.abs() * amount * 0.5);
        x * (1.0 - amount) + sat * amount
    }

    /// Simple one-pole lowpass for gentle HF rolloff
    #[inline]
    fn gentle_lowpass(&mut self, input: f64, freq: f64) -> f64 {
        let rc = 1.0 / (2.0 * PI * freq);
        let dt = 1.0 / self.sample_rate;
        let alpha = dt / (rc + dt);

        self.last_sample = self.last_sample + alpha * (input - self.last_sample);
        self.last_sample
    }

    /// DC blocker
    #[inline]
    fn dc_block(&mut self, input: f64) -> f64 {
        const R: f64 = 0.995;
        let output = input - self.dc_blocker_x + R * self.dc_blocker_y;
        self.dc_blocker_x = input;
        self.dc_blocker_y = output;
        output
    }

    /// Process a single sample
    pub fn process(&mut self, input: f32) -> f32 {
        let input = input as f64;

        // Very gentle saturation
        let saturated = Self::soft_saturate(input, self.warmth * 0.3);

        // Subtle HF rolloff (tape head loss simulation)
        let hf_freq = 18000.0 - self.warmth * 6000.0; // 18kHz down to 12kHz
        let filtered = self.gentle_lowpass(saturated, hf_freq);

        // Remove any DC offset
        let output = self.dc_block(filtered);

        output as f32
    }

    /// Process mono buffer
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Reset state
    pub fn reset(&mut self) {
        self.last_sample = 0.0;
        self.dc_blocker_x = 0.0;
        self.dc_blocker_y = 0.0;
    }
}

impl crate::traits::AudioProcessor for TapeGlue {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for TapeGlue {}

impl crate::traits::ProcessorF64 for TapeGlue {
    fn new(sample_rate: f64) -> Self {
        Self::new(sample_rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_langevin() {
        // L(0) should be 0
        assert!((langevin(0.0)).abs() < 1e-6);

        // L(x) should approach sign(x) for large |x|
        assert!((langevin(100.0) - 1.0).abs() < 0.02);
        assert!((langevin(-100.0) + 1.0).abs() < 0.02);
    }

    #[test]
    fn test_hysteresis_no_crash() {
        let mut hyst = TapeHysteresis::new(44100.0);

        // Process some samples
        for i in 0..1000 {
            let input = (i as f32 * 0.01).sin();
            let output = hyst.process(input);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_tape_glue_transparent() {
        let mut glue = TapeGlue::new(44100.0);
        glue.set_warmth(0.0);

        // With warmth at 0, should be nearly transparent
        // Note: Lowpass and DC blocker still cause minor deviations due to
        // phase shift and transient behavior
        for i in 0..100 {
            let input = (i as f32 * 0.1).sin() * 0.5;
            let output = glue.process(input);
            // Allow for filter settling and inherent filter deviation
            if i > 20 {
                assert!((output - input).abs() < 0.05,
                    "i={}, input={}, output={}, diff={}", i, input, output, (output - input).abs());
            }
        }
    }
}
