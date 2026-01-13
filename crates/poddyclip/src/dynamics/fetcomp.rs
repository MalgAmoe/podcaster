//! FET-style Compressor with Feedback Topology & Anti-Aliasing
//!
//! Features:
//! - Analytical Derivative Newton-Raphson Solver (Artifact-free)
//! - 2x Oversampling (Reduces aliasing from fast attack/saturation)
//! - JFET VCR Gain Law: G = 1 / (1 + k*v)
//! - Program-Dependent Ballistics inside the Feedback Loop
//! - C1-Continuous Saturation Curves
//!
//! Based on research from:
//! - Giannoulis et al. JAES Paper (envelope followers)
//! - JFET VCR physics and voltage divider topology
//! - 1176 circuit analysis

use crate::analysis::utils::{db_to_linear, linear_to_db};
use crate::traits::StereoProcessor;

// =============================================================================
// Constants
// =============================================================================

/// Newton-Raphson iterations (increased for precision with analytical derivative)
const NR_ITERATIONS: usize = 5;

/// Minimum signal level to avoid log(0)
const MIN_LEVEL: f32 = 1e-7;

/// Maximum gain reduction in dB (prevents runaway)
const MAX_GR_DB: f32 = 60.0;

/// Constant: 20 / ln(10) for dB derivative calculation
const DB_DERIVATIVE_CONST: f32 = 8.685889;

/// Convergence threshold for N-R solver
const NR_CONVERGENCE: f32 = 1e-5;

// =============================================================================
// FetComp Presets (1176-style: fast, colorful, saturates)
// =============================================================================

/// FetComp preset parameters
#[derive(Clone, Copy, Debug)]
pub struct FetCompPreset {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub input_drive: f32,
    pub output_drive: f32,
}

/// Preset names (1-5)
pub const FETCOMP_PRESET_NAMES: [&str; 5] = ["Gentle", "Light", "Moderate", "Strong", "Aggressive"];

/// FetComp presets (1-5 scale) - 1176-style character
pub const FETCOMP_PRESETS: [FetCompPreset; 5] = [
    // 1: Gentle - light leveling, minimal color
    FetCompPreset {
        threshold_db: -10.0,
        ratio: 2.0,
        attack_ms: 3.0,
        release_ms: 150.0,
        input_drive: 0.0,
        output_drive: 0.0,
    },
    // 2: Light - subtle warmth
    FetCompPreset {
        threshold_db: -12.0,
        ratio: 3.0,
        attack_ms: 1.5,
        release_ms: 120.0,
        input_drive: 0.1,
        output_drive: 0.0,
    },
    // 3: Moderate (default)
    FetCompPreset {
        threshold_db: -14.0,
        ratio: 4.0,
        attack_ms: 0.8,
        release_ms: 100.0,
        input_drive: 0.15,
        output_drive: 0.1,
    },
    // 4: Strong - punchy, colored
    FetCompPreset {
        threshold_db: -16.0,
        ratio: 6.0,
        attack_ms: 0.5,
        release_ms: 80.0,
        input_drive: 0.25,
        output_drive: 0.15,
    },
    // 5: Aggressive - heavy squash, saturated
    FetCompPreset {
        threshold_db: -18.0,
        ratio: 8.0,
        attack_ms: 0.2,
        release_ms: 60.0,
        input_drive: 0.4,
        output_drive: 0.25,
    },
];

/// Get preset name by level (1-5), returns "Unknown" for invalid levels
pub fn get_fetcomp_preset_name(level: u8) -> &'static str {
    FETCOMP_PRESET_NAMES
        .get((level as usize).saturating_sub(1))
        .unwrap_or(&"Unknown")
}

// =============================================================================
// FET Compressor (Mono) - Feedback Topology with Oversampling
// =============================================================================

/// FET-style compressor with true feedback topology and 2x oversampling
///
/// Uses analytical derivatives in Newton-Raphson for artifact-free operation.
/// 2x oversampling prevents aliasing from fast attack and saturation.
#[derive(Clone, Debug)]
pub struct FetCompressor {
    sample_rate: f32,

    // User Parameters
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_width_db: f32,
    input_drive: f32,
    output_drive: f32,

    // Derived Coefficients (computed for 2x oversampled rate)
    attack_coeff: f32,
    fast_release_coeff: f32,
    slow_release_coeff: f32,
    jfet_k: f32,

    // State
    envelope_db: f32,
    last_output_abs: f32,
    gain_reduction_db: f32,

    // Oversampling filter state (simple 2-tap for up/down)
    os_upsample_z1: f32,
    os_downsample_z1: f32,
}

impl FetCompressor {
    /// Create a new FET compressor with specified parameters
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        input_drive: f32,
        output_drive: f32,
        sample_rate: f32,
    ) -> Self {
        let mut comp = Self {
            sample_rate,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_width_db: 6.0,
            input_drive,
            output_drive,
            attack_coeff: 0.0,
            fast_release_coeff: 0.0,
            slow_release_coeff: 0.0,
            jfet_k: 0.0,
            envelope_db: -100.0,
            last_output_abs: MIN_LEVEL,
            gain_reduction_db: 0.0,
            os_upsample_z1: 0.0,
            os_downsample_z1: 0.0,
        };
        comp.update_coefficients();
        comp
    }

    /// Create with default parameters optimized for voice
    pub fn new_default(sample_rate: f32) -> Self {
        Self::new(
            -18.0, // threshold
            4.0,   // ratio 4:1
            0.8,   // attack 0.8ms (1176 fast attack)
            50.0,  // release 50ms
            0.0,   // input drive off by default
            0.0,   // output drive off by default
            sample_rate,
        )
    }

    /// Update all derived coefficients (computed for 2x oversampled rate)
    fn update_coefficients(&mut self) {
        // Coefficients are computed for 2x oversampled rate
        let os_rate = self.sample_rate * 2.0;

        // 1-exp(-1/tau) format for envelope follower
        self.attack_coeff = 1.0 - (-1.0 / (self.attack_ms * 0.001 * os_rate)).exp();
        self.fast_release_coeff = 1.0 - (-1.0 / (self.release_ms * 0.001 * os_rate)).exp();

        // Slow release is ~10x slower for program-dependent behavior
        self.slow_release_coeff = 1.0 - (-1.0 / (self.release_ms * 0.010 * os_rate)).exp();

        // JFET K factor: at max compression (v=1), gain = 1/ratio
        // 1/ratio = 1/(1+k) => k = ratio - 1
        self.jfet_k = (self.ratio - 1.0).max(0.0);
    }

    /// Set threshold in dB
    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.threshold_db = threshold_db;
    }

    /// Set ratio (typically 4, 8, 12, or 20 for 1176-style)
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
        self.jfet_k = (self.ratio - 1.0).max(0.0);
    }

    /// Set attack time in ms
    pub fn set_attack(&mut self, attack_ms: f32) {
        self.attack_ms = attack_ms;
        self.update_coefficients();
    }

    /// Set release time in ms
    pub fn set_release(&mut self, release_ms: f32) {
        self.release_ms = release_ms;
        self.update_coefficients();
    }

    /// Set input drive (pre-compression saturation)
    pub fn set_input_drive(&mut self, drive: f32) {
        self.input_drive = drive.clamp(0.0, 1.0);
    }

    /// Set output drive (post-compression saturation)
    pub fn set_output_drive(&mut self, drive: f32) {
        self.output_drive = drive.clamp(0.0, 1.0);
    }

    // =========================================================================
    // Core DSP: Analytical Gain Computer with Exact Derivative
    // =========================================================================

    /// Computes JFET gain AND its exact analytical derivative with respect to envelope (dB).
    ///
    /// This eliminates solver jitter caused by finite difference estimation.
    /// Returns (Gain, dGain/dEnv_db)
    #[inline]
    fn compute_gain_and_gradient(&self, env_db: f32) -> (f32, f32) {
        let over = env_db - self.threshold_db;
        let w = self.knee_width_db / 2.0;

        // V_control (0.0 to ~1.0+) and its derivative w.r.t env_db
        // Designed for C1 continuity at knee boundaries
        let (v_ctrl, dv_denv) = if over <= -w {
            // Below knee: no compression
            (0.0, 0.0)
        } else if over >= w {
            // Above knee: linear region (C1 continuous with quadratic)
            // At over=w: quadratic gives w/12, derivative is 1/12
            // Linear: v = (over/12) ensures continuity
            let scale = 1.0 / 12.0;
            let v = over * scale;
            (v, scale)
        } else {
            // Inside soft knee: quadratic interpolation
            // v = (over + w)² / (4 * w * 12)
            // At over=-w: v=0, dv=0
            // At over=+w: v=w/12, dv=1/12
            let numerator = over + w;
            let denominator = 4.0 * w * 12.0;
            let v = (numerator * numerator) / denominator;
            let dv = (2.0 * numerator) / denominator;
            (v, dv)
        };

        // JFET Transfer Function: G = 1 / (1 + k * v)
        let denom = 1.0 + self.jfet_k * v_ctrl;
        let gain = (1.0 / denom).clamp(db_to_linear(-MAX_GR_DB), 1.0);

        // Analytical Derivative using Chain Rule:
        // dG/dEnv = dG/dV * dV/dEnv
        // dG/dV = -k / (1 + kv)² = -k * G²
        let dg_dv = -self.jfet_k * (gain * gain);
        let dg_denv = dg_dv * dv_denv;

        (gain, dg_denv)
    }

    /// Get adaptive release coefficient based on GR depth
    #[inline]
    fn get_adaptive_release_coeff(&self) -> f32 {
        let gr_depth = (self.gain_reduction_db / 10.0).clamp(0.0, 1.0);
        self.fast_release_coeff + gr_depth * (self.slow_release_coeff - self.fast_release_coeff)
    }

    // =========================================================================
    // Feedback Solver with 2x Oversampling
    // =========================================================================

    /// Process a single sample with 2x oversampling
    pub fn process(&mut self, input: f32) -> f32 {
        // =====================================================================
        // Upsample 2x (linear interpolation for simplicity)
        // =====================================================================
        let x1 = 0.5 * (self.os_upsample_z1 + input);
        let x2 = input;
        self.os_upsample_z1 = input;

        // Process both sub-samples at 2x rate
        let y1 = self.process_subsample(x1);
        let y2 = self.process_subsample(x2);

        // =====================================================================
        // Downsample 2x (simple averaging with state)
        // =====================================================================
        let y_out = 0.5 * (y1 + y2);

        // Apply a simple lowpass to catch any remaining HF
        let filtered = 0.5 * (self.os_downsample_z1 + y_out);
        self.os_downsample_z1 = y_out;

        filtered
    }

    /// Process a single sub-sample (called at 2x rate)
    fn process_subsample(&mut self, input: f32) -> f32 {
        // =====================================================================
        // Stage 1: Input Saturation (Transformer - C1 continuous)
        // =====================================================================
        let x = self.saturate_transformer(input);
        let x_abs = x.abs();

        // =====================================================================
        // Stage 2: Newton-Raphson Feedback Solver with Analytical Derivative
        // =====================================================================
        let prev_env = self.envelope_db;
        let mut y_guess = self.last_output_abs.max(MIN_LEVEL);

        for _ in 0..NR_ITERATIONS {
            let y_db = linear_to_db(y_guess.max(MIN_LEVEL));

            // Determine ballistics coefficient (feedback: based on output level)
            let coeff = if y_db > prev_env {
                self.attack_coeff
            } else {
                self.get_adaptive_release_coeff()
            };

            // Predict envelope state
            let pred_env = prev_env + coeff * (y_db - prev_env);

            // Get gain and ANALYTICAL derivative
            let (gain, dg_denv) = self.compute_gain_and_gradient(pred_env);

            // F(y) = y - x * G(envelope(y))
            let f_y = y_guess - x_abs * gain;

            // F'(y) = 1 - x * (dG/dEnv * dEnv/dY_db * dY_db/dY)
            let dy_db_dy = DB_DERIVATIVE_CONST / y_guess.max(MIN_LEVEL);
            let total_deriv = dg_denv * coeff * dy_db_dy;
            let f_prime = 1.0 - x_abs * total_deriv;

            // Newton step with stability guard
            let step = f_y / (f_prime + 1e-9);
            y_guess = (y_guess - step).max(MIN_LEVEL);

            // Check convergence
            if step.abs() < NR_CONVERGENCE {
                break;
            }
        }

        // =====================================================================
        // Stage 3: Commit Final State
        // =====================================================================
        self.last_output_abs = y_guess;
        let final_y_db = linear_to_db(y_guess.max(MIN_LEVEL));

        // Final coefficient determination
        let coeff = if final_y_db > prev_env {
            self.attack_coeff
        } else {
            self.get_adaptive_release_coeff()
        };

        // Commit envelope state
        self.envelope_db = prev_env + coeff * (final_y_db - prev_env);

        // Get final gain
        let (final_gain, _) = self.compute_gain_and_gradient(self.envelope_db);
        self.gain_reduction_db = -linear_to_db(final_gain);

        // Apply gain
        let compressed = x * final_gain;

        // =====================================================================
        // Stage 4: Output Saturation (C1 continuous)
        // =====================================================================
        self.saturate_output(compressed)
    }

    // =========================================================================
    // Saturation (C1-Continuous / Smooth)
    // =========================================================================

    /// Input transformer saturation - polynomial soft clipper (C1 continuous)
    /// Adds odd harmonics without hard edges
    #[inline]
    fn saturate_transformer(&self, x: f32) -> f32 {
        if self.input_drive <= 0.0 {
            return x;
        }

        let drive = 1.0 + self.input_drive * 2.0;
        let x_driven = x * drive;

        // Polynomial soft clip: x - x³/3 (C1 continuous at boundaries)
        let saturated = if x_driven > 1.0 {
            2.0 / 3.0
        } else if x_driven < -1.0 {
            -2.0 / 3.0
        } else {
            x_driven - (x_driven * x_driven * x_driven) / 3.0
        };

        saturated / drive
    }

    /// Output amplifier saturation - fast sigmoid (cleaner than tanh)
    #[inline]
    fn saturate_output(&self, x: f32) -> f32 {
        if self.output_drive <= 0.0 {
            return x;
        }

        let x_driven = x * (1.0 + self.output_drive);

        // Fast sigmoid: x / (1 + |x|)
        let sat = x_driven / (1.0 + x_driven.abs());

        // Mix dry/wet based on drive
        x * (1.0 - self.output_drive * 0.5) + sat * (self.output_drive * 0.5)
    }

    /// Get current gain reduction in dB (for metering)
    pub fn get_gain_reduction_db(&self) -> f32 {
        -self.gain_reduction_db
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.envelope_db = -100.0;
        self.last_output_abs = MIN_LEVEL;
        self.gain_reduction_db = 0.0;
        self.os_upsample_z1 = 0.0;
        self.os_downsample_z1 = 0.0;
    }
}

impl crate::traits::AudioProcessor for FetCompressor {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for FetCompressor {}

impl crate::traits::Processor for FetCompressor {
    fn new(sample_rate: f32) -> Self {
        Self::new_default(sample_rate)
    }
}

// =============================================================================
// Stereo FET Compressor (Linked Feedback with Oversampling)
// =============================================================================

/// Stereo FET compressor with linked feedback detection and 2x oversampling
#[derive(Clone, Debug)]
pub struct StereoFetCompressor {
    sample_rate: f32,

    // User Parameters (shared)
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_width_db: f32,
    input_drive: f32,
    output_drive: f32,

    // Derived Coefficients
    attack_coeff: f32,
    fast_release_coeff: f32,
    slow_release_coeff: f32,
    jfet_k: f32,

    // State (shared for linked operation)
    envelope_db: f32,
    last_output_abs: f32,
    gain_reduction_db: f32,
    max_gain_reduction_db: f32,

    // Oversampling filter state (per channel)
    os_upsample_z1_l: f32,
    os_upsample_z1_r: f32,
    os_downsample_z1_l: f32,
    os_downsample_z1_r: f32,
}

impl StereoFetCompressor {
    /// Create a new stereo FET compressor
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        input_drive: f32,
        output_drive: f32,
        sample_rate: f32,
    ) -> Self {
        let mut comp = Self {
            sample_rate,
            threshold_db,
            ratio,
            attack_ms,
            release_ms,
            knee_width_db: 6.0,
            input_drive,
            output_drive,
            attack_coeff: 0.0,
            fast_release_coeff: 0.0,
            slow_release_coeff: 0.0,
            jfet_k: 0.0,
            envelope_db: -100.0,
            last_output_abs: MIN_LEVEL,
            gain_reduction_db: 0.0,
            max_gain_reduction_db: 0.0,
            os_upsample_z1_l: 0.0,
            os_upsample_z1_r: 0.0,
            os_downsample_z1_l: 0.0,
            os_downsample_z1_r: 0.0,
        };
        comp.update_coefficients();
        comp
    }

    /// Create with default parameters optimized for voice
    pub fn new_default(sample_rate: f32) -> Self {
        Self::new(
            -12.0,  // threshold
            4.0,    // ratio
            0.5,    // attack
            126.0,  // release
            0.14,   // input drive
            0.26,   // output drive
            sample_rate,
        )
    }

    /// Create with preset (1-5)
    pub fn new_with_preset(sample_rate: f32, preset: u8) -> Option<Self> {
        let p = FETCOMP_PRESETS.get((preset as usize).saturating_sub(1))?;
        Some(Self::new(
            p.threshold_db,
            p.ratio,
            p.attack_ms,
            p.release_ms,
            p.input_drive,
            p.output_drive,
            sample_rate,
        ))
    }

    fn update_coefficients(&mut self) {
        let os_rate = self.sample_rate * 2.0;
        self.attack_coeff = 1.0 - (-1.0 / (self.attack_ms * 0.001 * os_rate)).exp();
        self.fast_release_coeff = 1.0 - (-1.0 / (self.release_ms * 0.001 * os_rate)).exp();
        self.slow_release_coeff = 1.0 - (-1.0 / (self.release_ms * 0.010 * os_rate)).exp();
        self.jfet_k = (self.ratio - 1.0).max(0.0);
    }

    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.threshold_db = threshold_db;
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
        self.jfet_k = (self.ratio - 1.0).max(0.0);
    }

    pub fn set_attack(&mut self, attack_ms: f32) {
        self.attack_ms = attack_ms;
        self.update_coefficients();
    }

    pub fn set_release(&mut self, release_ms: f32) {
        self.release_ms = release_ms;
        self.update_coefficients();
    }

    pub fn set_input_drive(&mut self, drive: f32) {
        self.input_drive = drive.clamp(0.0, 1.0);
    }

    pub fn set_output_drive(&mut self, drive: f32) {
        self.output_drive = drive.clamp(0.0, 1.0);
    }

    #[inline]
    fn compute_gain_and_gradient(&self, env_db: f32) -> (f32, f32) {
        let over = env_db - self.threshold_db;
        let w = self.knee_width_db / 2.0;

        // C1 continuous soft knee
        let (v_ctrl, dv_denv) = if over <= -w {
            (0.0, 0.0)
        } else if over >= w {
            let scale = 1.0 / 12.0;
            let v = over * scale;
            (v, scale)
        } else {
            let numerator = over + w;
            let denominator = 4.0 * w * 12.0;
            let v = (numerator * numerator) / denominator;
            let dv = (2.0 * numerator) / denominator;
            (v, dv)
        };

        let denom = 1.0 + self.jfet_k * v_ctrl;
        let gain = (1.0 / denom).clamp(db_to_linear(-MAX_GR_DB), 1.0);
        let dg_dv = -self.jfet_k * (gain * gain);
        let dg_denv = dg_dv * dv_denv;

        (gain, dg_denv)
    }

    #[inline]
    fn get_adaptive_release_coeff(&self) -> f32 {
        let gr_depth = (self.gain_reduction_db / 10.0).clamp(0.0, 1.0);
        self.fast_release_coeff + gr_depth * (self.slow_release_coeff - self.fast_release_coeff)
    }

    #[inline]
    fn saturate_transformer(&self, x: f32) -> f32 {
        if self.input_drive <= 0.0 {
            return x;
        }
        let drive = 1.0 + self.input_drive * 2.0;
        let x_driven = x * drive;
        let saturated = if x_driven > 1.0 {
            2.0 / 3.0
        } else if x_driven < -1.0 {
            -2.0 / 3.0
        } else {
            x_driven - (x_driven * x_driven * x_driven) / 3.0
        };
        saturated / drive
    }

    #[inline]
    fn saturate_output(&self, x: f32) -> f32 {
        if self.output_drive <= 0.0 {
            return x;
        }
        let x_driven = x * (1.0 + self.output_drive);
        let sat = x_driven / (1.0 + x_driven.abs());
        x * (1.0 - self.output_drive * 0.5) + sat * (self.output_drive * 0.5)
    }

    /// Process stereo sample pair with 2x oversampling
    pub fn process_sample(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Upsample left
        let l1 = 0.5 * (self.os_upsample_z1_l + left);
        let l2 = left;
        self.os_upsample_z1_l = left;

        // Upsample right
        let r1 = 0.5 * (self.os_upsample_z1_r + right);
        let r2 = right;
        self.os_upsample_z1_r = right;

        // Process both sub-samples
        let (y_l1, y_r1) = self.process_subsample(l1, r1);
        let (y_l2, y_r2) = self.process_subsample(l2, r2);

        // Downsample
        let y_l = 0.5 * (y_l1 + y_l2);
        let y_r = 0.5 * (y_r1 + y_r2);

        // Final lowpass
        let out_l = 0.5 * (self.os_downsample_z1_l + y_l);
        let out_r = 0.5 * (self.os_downsample_z1_r + y_r);
        self.os_downsample_z1_l = y_l;
        self.os_downsample_z1_r = y_r;

        (out_l, out_r)
    }

    /// Process stereo sub-sample (at 2x rate)
    fn process_subsample(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Input saturation
        let x_l = self.saturate_transformer(left);
        let x_r = self.saturate_transformer(right);

        // Linked detection: max of both channels
        let x_peak = x_l.abs().max(x_r.abs());

        // Newton-Raphson solver
        let prev_env = self.envelope_db;
        let mut y_guess = self.last_output_abs.max(MIN_LEVEL);

        for _ in 0..NR_ITERATIONS {
            let y_db = linear_to_db(y_guess.max(MIN_LEVEL));

            let coeff = if y_db > prev_env {
                self.attack_coeff
            } else {
                self.get_adaptive_release_coeff()
            };

            let pred_env = prev_env + coeff * (y_db - prev_env);
            let (gain, dg_denv) = self.compute_gain_and_gradient(pred_env);

            let f_y = y_guess - x_peak * gain;
            let dy_db_dy = DB_DERIVATIVE_CONST / y_guess.max(MIN_LEVEL);
            let f_prime = 1.0 - x_peak * dg_denv * coeff * dy_db_dy;

            let step = f_y / (f_prime + 1e-9);
            y_guess = (y_guess - step).max(MIN_LEVEL);

            if step.abs() < NR_CONVERGENCE {
                break;
            }
        }

        // Commit state
        self.last_output_abs = y_guess;
        let final_y_db = linear_to_db(y_guess.max(MIN_LEVEL));

        let coeff = if final_y_db > prev_env {
            self.attack_coeff
        } else {
            self.get_adaptive_release_coeff()
        };

        self.envelope_db = prev_env + coeff * (final_y_db - prev_env);
        let (final_gain, _) = self.compute_gain_and_gradient(self.envelope_db);
        self.gain_reduction_db = -linear_to_db(final_gain);
        if self.gain_reduction_db > self.max_gain_reduction_db {
            self.max_gain_reduction_db = self.gain_reduction_db;
        }

        // Apply same gain to both channels
        let compressed_l = x_l * final_gain;
        let compressed_r = x_r * final_gain;

        (
            self.saturate_output(compressed_l),
            self.saturate_output(compressed_r),
        )
    }

    /// Get maximum gain reduction that occurred during processing (positive dB)
    pub fn get_gain_reduction_db(&self) -> f32 {
        self.max_gain_reduction_db
    }

    pub fn reset(&mut self) {
        self.envelope_db = -100.0;
        self.last_output_abs = MIN_LEVEL;
        self.gain_reduction_db = 0.0;
        self.max_gain_reduction_db = 0.0;
        self.os_upsample_z1_l = 0.0;
        self.os_upsample_z1_r = 0.0;
        self.os_downsample_z1_l = 0.0;
        self.os_downsample_z1_r = 0.0;
    }
}

impl StereoProcessor for StereoFetCompressor {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (out_l, out_r) = self.process_sample(*l, *r);
            *l = out_l;
            *r = out_r;
        }
    }

    fn process_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let (out, _) = self.process_sample(*sample, *sample);
            *sample = out;
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fet_compressor_creation() {
        let comp = FetCompressor::new_default(48000.0);
        assert_eq!(comp.threshold_db, -18.0);
        assert_eq!(comp.ratio, 4.0);
    }

    #[test]
    fn test_analytical_gain_below_threshold() {
        let comp = FetCompressor::new_default(48000.0);
        let (gain, dg) = comp.compute_gain_and_gradient(-30.0);
        assert!((gain - 1.0).abs() < 0.01, "Expected unity gain below threshold");
        assert!(dg.abs() < 0.001, "Expected zero derivative below threshold");
    }

    #[test]
    fn test_analytical_gain_above_threshold() {
        let comp = FetCompressor::new_default(48000.0);
        let (gain, dg) = comp.compute_gain_and_gradient(-6.0); // 12dB above -18
        assert!(gain < 1.0, "Expected compression above threshold");
        assert!(dg < 0.0, "Expected negative derivative (more input = less gain)");
    }

    #[test]
    fn test_gain_continuity_across_knee() {
        // Verify C1 continuity across knee boundaries
        let comp = FetCompressor::new_default(48000.0);
        let knee_start = comp.threshold_db - comp.knee_width_db / 2.0;
        let knee_end = comp.threshold_db + comp.knee_width_db / 2.0;

        // Sample points across knee
        let (g1, _) = comp.compute_gain_and_gradient(knee_start - 0.01);
        let (g2, _) = comp.compute_gain_and_gradient(knee_start + 0.01);
        let (g3, _) = comp.compute_gain_and_gradient(knee_end - 0.01);
        let (g4, _) = comp.compute_gain_and_gradient(knee_end + 0.01);

        // Should be continuous (no jumps)
        assert!((g1 - g2).abs() < 0.01, "Discontinuity at knee start");
        assert!((g3 - g4).abs() < 0.01, "Discontinuity at knee end");
    }

    #[test]
    fn test_envelope_tracking_not_waveform() {
        let mut comp = FetCompressor::new_default(48000.0);

        let freq = 100.0;
        let sample_rate = 48000.0;
        let mut last_gr: f32 = 0.0;
        let mut max_gr_change: f32 = 0.0;

        for i in 0..4800 {
            let t = i as f32 / sample_rate;
            let input = 0.7 * (2.0 * std::f32::consts::PI * freq * t).sin();
            let _ = comp.process(input);

            let gr_change = (comp.gain_reduction_db - last_gr).abs();
            if i > 200 {
                max_gr_change = max_gr_change.max(gr_change);
            }
            last_gr = comp.gain_reduction_db;
        }

        assert!(
            max_gr_change < 0.3,
            "GR changing too fast ({} dB/sample) - tracking waveform?",
            max_gr_change
        );
    }

    #[test]
    fn test_attack_release_behavior() {
        let mut comp = FetCompressor::new_default(48000.0);

        // Establish baseline with silence
        for _ in 0..2000 {
            comp.process(0.001);
        }
        let baseline_gr = comp.gain_reduction_db;

        // Hit with loud signal
        for _ in 0..200 {
            comp.process(0.9);
        }
        let attacked_gr = comp.gain_reduction_db;

        assert!(
            attacked_gr > baseline_gr + 1.0,
            "Attack not engaging: {} vs {}",
            baseline_gr,
            attacked_gr
        );

        // Release
        for _ in 0..20000 {
            comp.process(0.01);
        }
        let released_gr = comp.gain_reduction_db;

        assert!(
            released_gr < attacked_gr - 1.0,
            "Release not working: {} vs {}",
            attacked_gr,
            released_gr
        );
    }

    #[test]
    fn test_stereo_linked() {
        let mut comp = StereoFetCompressor::new_default(48000.0);

        // Warm up
        for _ in 0..200 {
            comp.process_sample(0.5, 0.5);
        }

        // Left loud
        for _ in 0..200 {
            comp.process_sample(0.8, 0.1);
        }
        let gr1 = comp.gain_reduction_db;

        comp.reset();
        for _ in 0..200 {
            comp.process_sample(0.5, 0.5);
        }

        // Right loud
        for _ in 0..200 {
            comp.process_sample(0.1, 0.8);
        }
        let gr2 = comp.gain_reduction_db;

        assert!(
            (gr1 - gr2).abs() < 1.0,
            "Linked detection failed: {} vs {}",
            gr1,
            gr2
        );
    }

    #[test]
    fn test_c1_saturation() {
        let comp = FetCompressor::new(
            -18.0, 4.0, 0.8, 50.0, 0.5, 0.5, 48000.0,
        );

        // Test input saturation is smooth around boundaries
        let s1 = comp.saturate_transformer(0.99);
        let s2 = comp.saturate_transformer(1.01);
        assert!(
            (s1 - s2).abs() < 0.05,
            "Input saturation discontinuity: {} vs {}",
            s1,
            s2
        );
    }
}
