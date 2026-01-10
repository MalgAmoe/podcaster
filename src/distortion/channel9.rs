//! Channel9 - Neve transformer emulation
//!
//! Ported from Airwindows Channel9 (MIT License)
//! Original by Chris Johnson
//!
//! Signal chain:
//! - Biquad lowpass (golden ratio Q = 1.618)
//! - Dielectric-scaled IIR highpass
//! - Spiral + Phat saturation
//! - Slew rate limiting (golden ratio coefficients)
//! - Biquad lowpass (golden ratio Q = 0.618)

#![allow(dead_code)]

use std::f32::consts::PI;

/// Neve console preset values
const NEVE_IIR_AMOUNT: f32 = 0.005832;
const NEVE_THRESHOLD: f32 = 0.33362176;
const NEVE_CUTOFF: f32 = 28811.0;

/// Golden ratio constants
const PHI: f32 = 1.618033988749895;
const PHI_INV: f32 = 0.6180339887498948;

/// Neve transformer emulation processor
#[derive(Clone, Debug)]
pub struct Channel9 {
    sample_rate: f32,

    // Biquad filter coefficients and state
    biquad_a: BiquadState,
    biquad_b: BiquadState,

    // IIR highpass state (alternating A/B)
    iir_sample_a: f32,
    iir_sample_b: f32,

    // Slew limiting state
    last_sample_a: f32,
    last_sample_b: f32,
    last_sample_c: f32,

    // Alternating filter selector
    flip: bool,

    // Parameters
    drive: f32,  // 0.0 - 1.0 (displayed as 0-200%)
    output: f32, // 0.0 - 1.0
}

#[derive(Clone, Debug, Default)]
struct BiquadState {
    // Coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // State (x = input history, y = output history)
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

impl Channel9 {
    pub fn new(sample_rate: f32) -> Self {
        let mut ch = Self {
            sample_rate,
            biquad_a: BiquadState::default(),
            biquad_b: BiquadState::default(),
            iir_sample_a: 0.0,
            iir_sample_b: 0.0,
            last_sample_a: 0.0,
            last_sample_b: 0.0,
            last_sample_c: 0.0,
            flip: false,
            drive: 0.0,
            output: 1.0,
        };
        ch.update_coefficients();
        ch
    }

    /// Set drive amount (0.0 - 1.0, maps to 0-200%)
    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.clamp(0.0, 1.0);
    }

    /// Get current drive
    pub fn get_drive(&self) -> f32 {
        self.drive
    }

    /// Set output level (0.0 - 1.0)
    pub fn set_output(&mut self, output: f32) {
        self.output = output.clamp(0.0, 1.0);
    }

    /// Get current output level
    pub fn get_output(&self) -> f32 {
        self.output
    }

    fn update_coefficients(&mut self) {
        let normalized_cutoff = NEVE_CUTOFF / self.sample_rate;

        // Only apply filter if cutoff is below Nyquist
        if normalized_cutoff >= 0.49999 {
            return;
        }

        let k = (PI * normalized_cutoff).tan();

        // Biquad A - Q = PHI (1.618)
        let norm_a = 1.0 / (1.0 + k / PHI + k * k);
        self.biquad_a.b0 = k * k * norm_a;
        self.biquad_a.b1 = 2.0 * self.biquad_a.b0;
        self.biquad_a.b2 = self.biquad_a.b0;
        self.biquad_a.a1 = 2.0 * (k * k - 1.0) * norm_a;
        self.biquad_a.a2 = (1.0 - k / PHI + k * k) * norm_a;

        // Biquad B - Q = PHI_INV (0.618)
        let norm_b = 1.0 / (1.0 + k / PHI_INV + k * k);
        self.biquad_b.b0 = k * k * norm_b;
        self.biquad_b.b1 = 2.0 * self.biquad_b.b0;
        self.biquad_b.b2 = self.biquad_b.b0;
        self.biquad_b.a1 = 2.0 * (k * k - 1.0) * norm_b;
        self.biquad_b.a2 = (1.0 - k / PHI_INV + k * k) * norm_b;
    }

    /// Process a single sample
    pub fn process(&mut self, input: f32) -> f32 {
        let overallscale = self.sample_rate / 44100.0;
        let local_iir_amount = NEVE_IIR_AMOUNT / overallscale;
        let local_threshold = NEVE_THRESHOLD;

        // Drive maps 0-1 to 0-2 (density)
        let density = self.drive * 2.0;
        let phattity = (density - 1.0).max(0.0);
        let density_clamped = density.min(1.0);
        let non_lin = 5.0 - density_clamped;

        let normalized_cutoff = NEVE_CUTOFF / self.sample_rate;

        // Biquad A lowpass
        let mut sample = if normalized_cutoff < 0.49999 {
            self.biquad_a.process(input)
        } else {
            input
        };

        // Dielectric-scaled IIR highpass
        let dielectric_scale = (2.0 - ((sample + non_lin) / non_lin)).abs();

        if self.flip {
            self.iir_sample_a = self.iir_sample_a * (1.0 - local_iir_amount * dielectric_scale)
                + sample * local_iir_amount * dielectric_scale;
            sample -= self.iir_sample_a;
        } else {
            self.iir_sample_b = self.iir_sample_b * (1.0 - local_iir_amount * dielectric_scale)
                + sample * local_iir_amount * dielectric_scale;
            sample -= self.iir_sample_b;
        }

        // Store dry for blending
        let dry_sample = sample;

        // Clip to [-1, 1] for saturation
        sample = sample.clamp(-1.0, 1.0);

        // Phat saturation: sin(x * pi/2)
        let phat_sample = (sample * 1.57079633).sin();

        // Scale for Spiral saturation
        sample *= 1.2533141373155;

        // Spiral saturation: sin(x * |x|) / |x|
        let abs_sample = sample.abs();
        let dist_sample = if abs_sample == 0.0 {
            0.0
        } else {
            (sample * abs_sample).sin() / abs_sample
        };

        // Blend saturations
        sample = dist_sample;
        if density_clamped < 1.0 {
            sample = dry_sample * (1.0 - density_clamped) + dist_sample * density_clamped;
        }
        if phattity > 0.0 {
            sample = sample * (1.0 - phattity) + phat_sample * phattity;
        }

        // Slew rate limiting with golden ratio coefficients
        let clamp = (self.last_sample_b - self.last_sample_c) * 0.381966011250105
            - (self.last_sample_a - self.last_sample_b) * PHI_INV
            + sample
            - self.last_sample_a;

        self.last_sample_c = self.last_sample_b;
        self.last_sample_b = self.last_sample_a;
        self.last_sample_a = sample;

        if clamp > local_threshold {
            sample = self.last_sample_b + local_threshold;
        }
        if -clamp > local_threshold {
            sample = self.last_sample_b - local_threshold;
        }

        // Update buffer with blend of raw and smoothed
        self.last_sample_a = self.last_sample_a * 0.381966011250105 + sample * PHI_INV;

        self.flip = !self.flip;

        // Output gain
        if self.output < 1.0 {
            sample *= self.output;
        }

        // Biquad B lowpass
        if normalized_cutoff < 0.49999 {
            sample = self.biquad_b.process(sample);
        }

        sample
    }

    /// Process mono buffer in-place
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Reset all state
    pub fn reset(&mut self) {
        self.biquad_a.reset();
        self.biquad_b.reset();
        self.iir_sample_a = 0.0;
        self.iir_sample_b = 0.0;
        self.last_sample_a = 0.0;
        self.last_sample_b = 0.0;
        self.last_sample_c = 0.0;
        self.flip = false;
    }
}

impl crate::traits::AudioProcessor for Channel9 {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for Channel9 {}

impl crate::traits::Processor for Channel9 {
    fn new(sample_rate: f32) -> Self {
        Self::new(sample_rate)
    }
}

