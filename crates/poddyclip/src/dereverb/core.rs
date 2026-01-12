//! De-Reverb Processor
//!
//! Spectral gating de-reverb that tracks reverb energy per FFT bin
//! and attenuates bins where reverb dominates over direct sound.

#![allow(dead_code)]

use super::common::*;
use crate::analysis::ReverbAnalysis;
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

// =============================================================================
// De-Reverb Processor
// =============================================================================

/// Spectral gating de-reverb processor
pub struct DeReverbProcessor {
    sample_rate: u32,
    window_size: usize,
    hop_size: usize,
    n_bins: usize,

    // Parameters
    params: DeReverbParams,

    // Window
    window: Vec<f32>,

    // Reverb tracking per bin
    prev_frame_power: Vec<f32>,
    reverb_estimate: Vec<f32>,
    decay_per_bin: Vec<f32>,

    // Gain smoothing
    prev_gain: Vec<f32>,

    // Overlap buffer
    overlap_buffer: Vec<f32>,

    // FFT
    fft: Arc<dyn rustfft::Fft<f32>>,
    ifft: Arc<dyn rustfft::Fft<f32>>,
    fft_scratch: Vec<Complex<f32>>,

    // Statistics for display
    max_gain_reduction_db: f32,
}

impl DeReverbProcessor {
    /// Create with default parameters
    pub fn new(sample_rate: u32) -> Self {
        Self::new_with_params(sample_rate, DeReverbParams::default())
    }

    /// Create with preset level (1-5)
    pub fn new_with_preset(sample_rate: u32, preset: u8) -> Option<Self> {
        DeReverbParams::from_preset(preset).map(|params| Self::new_with_params(sample_rate, params))
    }

    /// Create with custom parameters
    pub fn new_with_params(sample_rate: u32, params: DeReverbParams) -> Self {
        let window_size = WINDOW_SIZE;
        let hop_size = HOP_SIZE;
        let n_bins = N_BINS;

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(window_size);
        let ifft = planner.plan_fft_inverse(window_size);
        let fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

        // Default decay: assume 300ms RT60 at all frequencies
        let default_rt60_ms = 300.0;
        let hop_time_ms = (hop_size as f32 / sample_rate as f32) * 1000.0;
        let default_decay = 10.0f32.powf(-3.0 * hop_time_ms / default_rt60_ms);

        Self {
            sample_rate,
            window_size,
            hop_size,
            n_bins,
            params,
            window: create_sqrt_hann_window(window_size),
            prev_frame_power: vec![0.0; n_bins],
            reverb_estimate: vec![0.0; n_bins],
            decay_per_bin: vec![default_decay; n_bins],
            prev_gain: vec![1.0; n_bins],
            overlap_buffer: vec![0.0; window_size],
            fft,
            ifft,
            fft_scratch,
            max_gain_reduction_db: 0.0,
        }
    }

    /// Initialize with reverb analysis (recommended for best results)
    pub fn init_with_analysis(&mut self, analysis: &ReverbAnalysis) {
        let bin_freq = self.sample_rate as f32 / self.window_size as f32;

        // Compute per-bin decay rates from analysis
        self.decay_per_bin = analysis.compute_decay_per_bin(self.n_bins, self.hop_size, bin_freq);

        // Apply decay multiplier from params
        for decay in &mut self.decay_per_bin {
            // Higher multiplier = assume faster decay = more aggressive gating
            *decay = decay.powf(self.params.decay_multiplier);
        }
    }

    /// Set parameters
    pub fn set_params(&mut self, params: DeReverbParams) {
        self.params = params;
    }

    /// Get current max gain reduction (for display)
    pub fn get_max_gain_reduction_db(&self) -> f32 {
        self.max_gain_reduction_db
    }

    /// Process entire audio buffer (batch mode)
    pub fn process(&mut self, audio: &[f32]) -> Vec<f32> {
        if audio.is_empty() {
            return Vec::new();
        }

        let mut output = vec![0.0; audio.len()];
        self.max_gain_reduction_db = 0.0;

        // Process frame by frame with overlap-add
        let mut pos = 0;
        while pos + self.window_size <= audio.len() {
            let frame = &audio[pos..pos + self.window_size];
            let processed = self.process_frame(frame);

            // Overlap-add
            for (i, &sample) in processed.iter().enumerate() {
                if pos + i < output.len() {
                    output[pos + i] += sample;
                }
            }

            pos += self.hop_size;
        }

        // Handle final partial frame if any
        if pos < audio.len() {
            let remaining = audio.len() - pos;
            let mut padded = vec![0.0; self.window_size];
            padded[..remaining].copy_from_slice(&audio[pos..]);

            let processed = self.process_frame(&padded);
            for (i, &sample) in processed[..remaining].iter().enumerate() {
                output[pos + i] += sample;
            }
        }

        output
    }

    /// Process a single frame
    fn process_frame(&mut self, frame: &[f32]) -> Vec<f32> {
        // Apply analysis window
        let mut windowed: Vec<Complex<f32>> = frame
            .iter()
            .zip(self.window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();

        // Forward FFT
        self.fft
            .process_with_scratch(&mut windowed, &mut self.fft_scratch);

        // Compute power spectrum
        let power: Vec<f32> = windowed[..self.n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        // Update reverb estimate
        self.update_reverb_estimate(&power);

        // Compute de-reverb gain
        let gain = self.compute_dereverb_gain(&power);

        // Apply gain to spectrum
        for (i, g) in gain.iter().enumerate() {
            windowed[i] = windowed[i] * g;
            // Mirror for negative frequencies
            if i > 0 && i < self.n_bins - 1 {
                windowed[self.window_size - i] = windowed[self.window_size - i] * g;
            }
        }

        // Inverse FFT
        self.ifft
            .process_with_scratch(&mut windowed, &mut self.fft_scratch);

        // Normalize and apply synthesis window
        let scale = 1.0 / self.window_size as f32;
        windowed
            .iter()
            .zip(self.window.iter())
            .map(|(c, &w)| c.re * scale * w)
            .collect()
    }

    /// Update reverb estimate based on previous frame energy
    fn update_reverb_estimate(&mut self, power: &[f32]) {
        for k in 0..self.n_bins {
            // Reverb decays exponentially from previous frame
            // reverb[k] = decay[k] * reverb[k] + (1-decay[k]) * prev_power[k]
            let decay = self.decay_per_bin[k];
            self.reverb_estimate[k] =
                decay * self.reverb_estimate[k] + (1.0 - decay) * self.prev_frame_power[k];
        }

        // Store current frame for next iteration
        self.prev_frame_power.copy_from_slice(power);
    }

    /// Compute gain curve for reverb suppression
    ///
    /// Key insight: Reverb tails occur when current power DROPS but reverb estimate
    /// (from previous frames) remains HIGH. We want to attenuate these tails.
    ///
    /// Strategy: Compare current power to reverb estimate. If current << reverb,
    /// we're likely in a reverb tail and should attenuate.
    fn compute_dereverb_gain(&mut self, power: &[f32]) -> Vec<f32> {
        let floor = db_to_linear(self.params.gate_threshold_db);
        let strength = self.params.strength;
        let smoothing = self.params.smoothing;

        let mut gain = vec![1.0; self.n_bins];
        let mut max_reduction = 0.0f32;

        for k in 0..self.n_bins {
            // Reverb tail detection:
            // If reverb_estimate > power, we're likely in a decay tail
            // The ratio indicates how much reverb dominates over direct sound
            let direct_to_reverb = power[k] / (self.reverb_estimate[k] + EPSILON);

            // When direct_to_reverb < 1.0, reverb dominates -> attenuate
            // When direct_to_reverb > 1.0, direct sound dominates -> preserve
            let tail_indicator = (1.0 - direct_to_reverb).max(0.0);

            // Apply strength scaling
            let attenuation = (tail_indicator * strength * 2.0).clamp(0.0, 1.0);

            // Target gain with floor
            let target_gain = (1.0 - attenuation).max(floor);

            // Smooth gain changes to prevent pumping
            gain[k] = smoothing * self.prev_gain[k] + (1.0 - smoothing) * target_gain;

            // Track max reduction
            let reduction_db = linear_to_db(gain[k]);
            if reduction_db < max_reduction {
                max_reduction = reduction_db;
            }
        }

        // Update state
        self.prev_gain.copy_from_slice(&gain);
        self.max_gain_reduction_db = max_reduction;

        gain
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.prev_frame_power.fill(0.0);
        self.reverb_estimate.fill(0.0);
        self.prev_gain.fill(1.0);
        self.overlap_buffer.fill(0.0);
        self.max_gain_reduction_db = 0.0;
    }
}

impl Clone for DeReverbProcessor {
    fn clone(&self) -> Self {
        // Re-create FFT plans since they're not Clone
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.window_size);
        let ifft = planner.plan_fft_inverse(self.window_size);
        let fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

        Self {
            sample_rate: self.sample_rate,
            window_size: self.window_size,
            hop_size: self.hop_size,
            n_bins: self.n_bins,
            params: self.params.clone(),
            window: self.window.clone(),
            prev_frame_power: self.prev_frame_power.clone(),
            reverb_estimate: self.reverb_estimate.clone(),
            decay_per_bin: self.decay_per_bin.clone(),
            prev_gain: self.prev_gain.clone(),
            overlap_buffer: self.overlap_buffer.clone(),
            fft,
            ifft,
            fft_scratch,
            max_gain_reduction_db: self.max_gain_reduction_db,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_processor() {
        let proc = DeReverbProcessor::new(48000);
        assert_eq!(proc.window_size, WINDOW_SIZE);
        assert_eq!(proc.n_bins, N_BINS);
    }

    #[test]
    fn test_preset_creation() {
        for level in 1..=5 {
            let proc = DeReverbProcessor::new_with_preset(48000, level);
            assert!(proc.is_some());
        }
        let invalid = DeReverbProcessor::new_with_preset(48000, 0);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_process_empty() {
        let mut proc = DeReverbProcessor::new(48000);
        let output = proc.process(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn test_process_preserves_length() {
        let mut proc = DeReverbProcessor::new(48000);
        let input = vec![0.0; 48000]; // 1 second
        let output = proc.process(&input);
        assert_eq!(output.len(), input.len());
    }
}
