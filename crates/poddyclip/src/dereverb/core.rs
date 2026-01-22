//! De-Reverb Processor
//!
//! Spectral gating de-reverb that tracks reverb energy per FFT bin
//! and attenuates bins where reverb dominates over direct sound.

#![allow(dead_code)]

use super::common::*;
use crate::analysis::ReverbAnalysis;
use crate::stft::{StftProcessor, RT_HOP_SIZE, RT_N_BINS, RT_WINDOW_SIZE};

// =============================================================================
// De-Reverb Processor
// =============================================================================

/// Spectral gating de-reverb processor
pub struct DeReverbProcessor {
    // STFT processor (handles FFT, IFFT, overlap-add)
    stft: StftProcessor,

    // Parameters
    params: DeReverbParams,

    // Reverb tracking per bin
    prev_frame_power: Vec<f32>,
    reverb_estimate: Vec<f32>,
    decay_per_bin: Vec<f32>,

    // Gain smoothing
    prev_gain: Vec<f32>,

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
        let stft = StftProcessor::new_realtime(sample_rate);
        let n_bins = stft.n_bins();

        // Default decay: assume 300ms RT60 at all frequencies
        let default_rt60_ms = 300.0;
        let hop_time_ms = (RT_HOP_SIZE as f32 / sample_rate as f32) * 1000.0;
        let default_decay = 10.0f32.powf(-3.0 * hop_time_ms / default_rt60_ms);

        Self {
            stft,
            params,
            prev_frame_power: vec![0.0; n_bins],
            reverb_estimate: vec![0.0; n_bins],
            decay_per_bin: vec![default_decay; n_bins],
            prev_gain: vec![1.0; n_bins],
            max_gain_reduction_db: 0.0,
        }
    }

    /// Initialize with reverb analysis (recommended for best results)
    pub fn init_with_analysis(&mut self, analysis: &ReverbAnalysis) {
        let bin_freq = self.stft.sample_rate() as f32 / self.stft.window_size() as f32;
        let n_bins = self.stft.n_bins();
        let hop_size = self.stft.hop_size();

        // Compute per-bin decay rates from analysis
        self.decay_per_bin = analysis.compute_decay_per_bin(n_bins, hop_size, bin_freq);

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
        self.max_gain_reduction_db = 0.0;

        // Process frame by frame
        let mut i = 0;
        while i + window_size <= input.len() {
            let frame = &input[i..i + window_size];

            // Forward FFT
            let mut spectrum = self.stft.forward_fft(frame);

            // Compute power spectrum
            let power = self.stft.compute_power(&spectrum);

            // Update reverb estimate
            self.update_reverb_estimate(&power);

            // Compute de-reverb gain
            let gain = self.compute_dereverb_gain(&power);

            // Apply gain to spectrum
            for k in 0..n_bins {
                spectrum[k] *= gain[k];
            }

            // Ensure symmetry for real-valued output
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

    /// Update reverb estimate based on previous frame energy
    fn update_reverb_estimate(&mut self, power: &[f32]) {
        let n_bins = self.stft.n_bins();
        for k in 0..n_bins {
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
        let n_bins = self.stft.n_bins();
        let floor = db_to_linear(self.params.gate_threshold_db);
        let strength = self.params.strength;
        let smoothing = self.params.smoothing;

        let mut gain = vec![1.0; n_bins];
        let mut max_reduction = 0.0f32;

        for k in 0..n_bins {
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
        self.stft.reset();
        self.max_gain_reduction_db = 0.0;
    }
}

impl Clone for DeReverbProcessor {
    fn clone(&self) -> Self {
        // Create new processor and copy state
        let mut new_proc = Self::new_with_params(self.stft.sample_rate(), self.params.clone());
        new_proc.prev_frame_power.copy_from_slice(&self.prev_frame_power);
        new_proc.reverb_estimate.copy_from_slice(&self.reverb_estimate);
        new_proc.decay_per_bin.copy_from_slice(&self.decay_per_bin);
        new_proc.prev_gain.copy_from_slice(&self.prev_gain);
        new_proc.max_gain_reduction_db = self.max_gain_reduction_db;
        new_proc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_processor() {
        let proc = DeReverbProcessor::new(48000);
        assert_eq!(proc.stft.window_size(), RT_WINDOW_SIZE);
        assert_eq!(proc.stft.n_bins(), RT_N_BINS);
    }

    #[test]
    fn test_preset_creation() {
        for level in 1..=3 {
            let proc = DeReverbProcessor::new_with_preset(48000, level);
            assert!(proc.is_some());
        }
        let invalid = DeReverbProcessor::new_with_preset(48000, 0);
        assert!(invalid.is_none());
        let invalid = DeReverbProcessor::new_with_preset(48000, 4);
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
