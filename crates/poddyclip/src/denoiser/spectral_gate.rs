//! Non-Stationary Spectral Gating
//!
//! Complements spectral subtraction by handling intermittent noise
//! (fan cycling, HVAC, etc.) with fast-adapting per-bin noise floor.

use std::f32::consts::PI;

use crate::stft::{StftProcessor, EPSILON, RT_HOP_SIZE, RT_N_BINS, RT_WINDOW_SIZE};

/// Parameters for spectral gating
#[derive(Clone, Debug)]
pub struct SpectralGateParams {
    /// Enable/disable gating
    pub enabled: bool,
    /// Base threshold in dB (SNR below which gating starts)
    pub threshold_db: f32,
    /// Knee width in dB (soft transition region)
    pub knee_db: f32,
    /// Gate floor (minimum gain, prevents complete silence)
    pub floor: f32,
    /// Ratio (1.0 = full gate, 0.5 = half attenuation)
    pub ratio: f32,
    /// Attack time in ms
    pub attack_ms: f32,
    /// Release time in ms
    pub release_ms: f32,
    /// Noise floor adaptation speed (fast down)
    pub alpha_fast_down: f32,
    /// Noise floor adaptation speed (slow up)
    pub alpha_fast_up: f32,
}

impl Default for SpectralGateParams {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold_db: 6.0,
            knee_db: 6.0,
            floor: 0.05,
            ratio: 0.7,
            attack_ms: 5.0,
            release_ms: 50.0,
            alpha_fast_down: 0.7,
            alpha_fast_up: 0.95,
        }
    }
}

impl SpectralGateParams {
    /// Create params from preset level (1-3)
    pub fn from_preset(level: u8) -> Option<Self> {
        if level == 0 || level > 3 {
            return None;
        }
        SPECTRAL_GATE_PRESETS
            .get(level.saturating_sub(1) as usize)
            .cloned()
    }
}

/// Preset definitions (1-3)
pub const SPECTRAL_GATE_PRESETS: [SpectralGateParams; 3] = [
    // 1: Subtle - minimal gating
    SpectralGateParams {
        enabled: true,
        threshold_db: 3.0,
        knee_db: 8.0,
        floor: 0.1,
        ratio: 0.5,
        attack_ms: 3.0,
        release_ms: 80.0,
        alpha_fast_down: 0.6,
        alpha_fast_up: 0.97,
    },
    // 2: Balanced (default)
    SpectralGateParams {
        enabled: true,
        threshold_db: 4.0,
        knee_db: 7.0,
        floor: 0.08,
        ratio: 0.6,
        attack_ms: 4.0,
        release_ms: 70.0,
        alpha_fast_down: 0.65,
        alpha_fast_up: 0.96,
    },
    // 3: Intense
    SpectralGateParams {
        enabled: true,
        threshold_db: 6.0,
        knee_db: 6.0,
        floor: 0.05,
        ratio: 0.7,
        attack_ms: 5.0,
        release_ms: 50.0,
        alpha_fast_down: 0.7,
        alpha_fast_up: 0.95,
    },
];

/// Preset names for display
pub const SPECTRAL_GATE_PRESET_NAMES: [&str; 3] = ["Subtle", "Balanced", "Intense"];

/// Get preset name by level (1-3)
pub fn get_gate_preset_name(level: u8) -> &'static str {
    SPECTRAL_GATE_PRESET_NAMES
        .get(level.saturating_sub(1) as usize)
        .unwrap_or(&"Unknown")
}

const SPIKE_THRESHOLD: f32 = 5.0; // Ignore signals 5x above noise floor

/// Non-stationary spectral gate processor
pub struct SpectralGate {
    // STFT processor (handles FFT, IFFT, overlap-add)
    stft: StftProcessor,

    // Parameters
    params: SpectralGateParams,

    // Fast-adapting noise floor (per bin)
    fast_noise_floor: Vec<f32>,

    // Smoothed gain (attack/release)
    smoothed_gain: Vec<f32>,

    // Cached coefficients
    attack_coeff: f32,
    release_coeff: f32,
}

impl SpectralGate {
    /// Create new spectral gate
    pub fn new(sample_rate: u32) -> Self {
        Self::new_with_params(sample_rate, SpectralGateParams::default())
    }

    /// Create with preset
    pub fn new_with_preset(sample_rate: u32, preset: u8) -> Option<Self> {
        let params = SpectralGateParams::from_preset(preset)?;
        Some(Self::new_with_params(sample_rate, params))
    }

    /// Create with custom params
    pub fn new_with_params(sample_rate: u32, params: SpectralGateParams) -> Self {
        let stft = StftProcessor::new_realtime(sample_rate);
        let n_bins = stft.n_bins();

        let mut gate = Self {
            stft,
            params,
            fast_noise_floor: vec![EPSILON; n_bins],
            smoothed_gain: vec![1.0; n_bins],
            attack_coeff: 0.0,
            release_coeff: 0.0,
        };
        gate.update_coefficients();
        gate
    }

    /// Set parameters
    pub fn set_params(&mut self, params: SpectralGateParams) {
        self.params = params;
        self.update_coefficients();
    }

    /// Update time-domain coefficients from ms values
    fn update_coefficients(&mut self) {
        let hop_time_s = RT_HOP_SIZE as f32 / self.stft.sample_rate() as f32;

        let attack_tc = self.params.attack_ms / 1000.0;
        let release_tc = self.params.release_ms / 1000.0;

        self.attack_coeff = (-hop_time_s / attack_tc).exp();
        self.release_coeff = (-hop_time_s / release_tc).exp();
    }

    /// Initialize noise floor from analysis pass
    pub fn init_noise_floor(&mut self, noise_floor: &[f32]) {
        let n_bins = self.stft.n_bins();
        let len = n_bins.min(noise_floor.len());
        self.fast_noise_floor[..len].copy_from_slice(&noise_floor[..len]);
    }

    /// Process entire audio buffer (batch mode, for CLI usage)
    pub fn process(&mut self, audio: &[f32]) -> Vec<f32> {
        let original_len = audio.len();
        let window_size = self.stft.window_size();
        let hop_size = self.stft.hop_size();
        let n_bins = self.stft.n_bins();

        // Pre-pad for first window overlap
        let pre_pad = window_size - hop_size;

        // Pad to multiple of hop size
        let pad_len = (hop_size - audio.len() % hop_size) % hop_size;
        let mut padded = audio.to_vec();
        padded.resize(audio.len() + pad_len, 0.0);

        // Post-pad to ensure enough frames for output
        padded.resize(padded.len() + pre_pad, 0.0);

        // Add pre-pad
        let mut input = vec![0.0; pre_pad];
        input.extend(padded);

        let mut output = Vec::new();

        // Process frame by frame
        let mut i = 0;
        while i + window_size <= input.len() {
            let frame = &input[i..i + window_size];

            // Forward FFT
            let mut spectrum = self.stft.forward_fft(frame);

            // Compute power
            let power = self.stft.compute_power(&spectrum);

            // Compute and apply gate gains
            let gains = self.compute_gate_gains(&power);
            for k in 0..n_bins {
                spectrum[k] *= gains[k];
            }

            // Ensure symmetry
            self.stft.ensure_symmetry(&mut spectrum);

            // Inverse FFT
            let synthesized = self.stft.inverse_fft(&mut spectrum);

            // Overlap-add
            let out_frame = self.stft.overlap_add(&synthesized);
            output.extend(out_frame);

            i += hop_size;
        }

        // Remove pre-padding and trim to original length
        if output.len() > pre_pad {
            output = output[pre_pad..].to_vec();
        }
        output.truncate(original_len);

        output
    }

    /// Compute gate gains for current frame
    fn compute_gate_gains(&mut self, power: &[f32]) -> Vec<f32> {
        let n_bins = self.stft.n_bins();

        if !self.params.enabled {
            return vec![1.0; n_bins];
        }

        let threshold_db = self.params.threshold_db;
        let knee_db = self.params.knee_db;
        let floor = self.params.floor;
        let ratio = self.params.ratio;
        let alpha_down = self.params.alpha_fast_down;
        let alpha_up = self.params.alpha_fast_up;

        let mut gains = vec![1.0; n_bins];

        for k in 0..n_bins.min(power.len()) {
            let p = power[k];

            // Update fast-adapting noise floor
            if p < self.fast_noise_floor[k] {
                // Noise dropped - fast adaptation down
                self.fast_noise_floor[k] =
                    alpha_down * self.fast_noise_floor[k] + (1.0 - alpha_down) * p;
            } else if p < SPIKE_THRESHOLD * self.fast_noise_floor[k] {
                // Not a spike - slow upward adaptation
                self.fast_noise_floor[k] =
                    alpha_up * self.fast_noise_floor[k] + (1.0 - alpha_up) * p;
            }
            // If spike detected, don't update noise floor

            // Compute SNR
            let snr = p / (self.fast_noise_floor[k] + EPSILON);
            let snr_db = 10.0 * snr.max(EPSILON).log10();

            // Compute gate gain with soft knee
            let half_knee = knee_db / 2.0;
            let raw_gain = if snr_db >= threshold_db + half_knee {
                // Fully open
                1.0
            } else if snr_db <= threshold_db - half_knee {
                // Fully closed (to floor)
                floor
            } else {
                // Soft knee region - raised cosine interpolation
                let x = (snr_db - threshold_db) / knee_db; // -0.5 to +0.5
                let t = 0.5 * (1.0 + (PI * x).cos()); // 1.0 at -0.5, 0.0 at +0.5
                floor + (1.0 - floor) * (1.0 - t)
            };

            // Apply ratio (partial gating)
            let target_gain = 1.0 - (1.0 - raw_gain) * ratio;

            // Temporal smoothing (attack/release)
            let coeff = if target_gain > self.smoothed_gain[k] {
                self.attack_coeff // Gate opening
            } else {
                self.release_coeff // Gate closing
            };

            self.smoothed_gain[k] = coeff * self.smoothed_gain[k] + (1.0 - coeff) * target_gain;

            gains[k] = self.smoothed_gain[k];
        }

        gains
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.fast_noise_floor.fill(EPSILON);
        self.smoothed_gain.fill(1.0);
        self.stft.reset();
    }

    /// Get current max gain reduction for display
    pub fn get_max_gain_reduction_db(&self) -> f32 {
        let min_gain = self.smoothed_gain.iter().cloned().fold(1.0f32, f32::min);
        20.0 * min_gain.max(EPSILON).log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_spectral_gate() {
        let gate = SpectralGate::new(48000);
        assert_eq!(gate.stft.n_bins(), RT_N_BINS);
    }

    #[test]
    fn test_preset_creation() {
        for level in 1..=3 {
            let gate = SpectralGate::new_with_preset(48000, level);
            assert!(gate.is_some());
        }
        let invalid = SpectralGate::new_with_preset(48000, 0);
        assert!(invalid.is_none());
        let invalid = SpectralGate::new_with_preset(48000, 4);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_process_audio() {
        let mut gate = SpectralGate::new(48000);

        // Create test signal (1 second of audio)
        let samples: Vec<f32> = (0..48000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();

        let output = gate.process(&samples);

        // Output should be same length as input
        assert_eq!(output.len(), samples.len());
    }
}
