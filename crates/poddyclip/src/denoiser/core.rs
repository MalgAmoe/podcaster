//! Unified Spectral Subtraction Denoiser

#![allow(dead_code)]

use super::common::*;
use crate::stft::StftProcessor;
use std::collections::VecDeque;

// Re-export shared types
pub use super::common::{DenoiserParams, BANDS, NUM_BANDS};

// =============================================================================
// Visualization Data
// =============================================================================

/// Data for real-time visualization in the GUI
#[derive(Clone, Debug)]
pub struct VisualizationData {
    /// Current power spectrum (1025 bins for 2048 FFT)
    pub current_spectrum: Vec<f32>,

    /// Noise floor spectrum (1025 bins)
    pub noise_spectrum: Vec<f32>,

    /// Per-band gain reduction in dB (9 bands)
    /// Negative values indicate attenuation
    pub band_gain_db: [f32; NUM_BANDS],

    /// Per-band SNR in dB (9 bands)
    pub band_snr_db: [f32; NUM_BANDS],

    /// Sample rate (for frequency axis calculation)
    pub sample_rate: u32,
}

impl Default for VisualizationData {
    fn default() -> Self {
        Self {
            current_spectrum: vec![0.0; 1025],
            noise_spectrum: vec![0.0; 1025],
            band_gain_db: [0.0; NUM_BANDS],
            band_snr_db: [0.0; NUM_BANDS],
            sample_rate: 48000,
        }
    }
}

// =============================================================================
// Real-time Denoiser
// =============================================================================

pub struct RealtimeDenoiser {
    // STFT processor (handles FFT, IFFT, overlap-add)
    pub(crate) stft: StftProcessor,

    // Parameters (can be updated)
    params: DenoiserParams,

    // State
    noise_pow: Vec<f32>,
    prev_gain: Vec<f32>,
    prev_sfm_decision: bool,
    frames_processed: usize,
    needs_initialization: bool,

    // Cached gamma curve (rebuild when params change)
    gamma_curve: Vec<f32>,
    gamma_dirty: bool,

    // Visualization (optional, only populated when GUI is open)
    visualization_enabled: bool,
    cached_viz_data: VisualizationData,
}

impl RealtimeDenoiser {
    pub fn new(sample_rate: u32) -> Self {
        Self::new_with_params(sample_rate, DenoiserParams::default())
    }

    /// Create denoiser with a preset level (1-5, like CLI)
    pub fn new_with_preset(sample_rate: u32, preset: usize) -> Option<Self> {
        DenoiserParams::from_preset(preset).map(|params| Self::new_with_params(sample_rate, params))
    }

    /// Create denoiser with custom params
    pub fn new_with_params(sample_rate: u32, params: DenoiserParams) -> Self {
        let stft = StftProcessor::new_realtime(sample_rate);
        let window_size = stft.window_size();
        let n_bins = stft.n_bins();

        let gamma_curve = compute_gamma_curve(window_size, sample_rate, &params.gamma);

        Self {
            stft,
            params,
            noise_pow: vec![EPSILON; n_bins], // Small non-zero placeholder
            prev_gain: vec![1.0; n_bins],
            prev_sfm_decision: true,
            frames_processed: 0,
            needs_initialization: true, // Will initialize from first frame
            gamma_curve,
            gamma_dirty: false,
            visualization_enabled: false,
            cached_viz_data: VisualizationData::default(),
        }
    }

    /// Initialize with pre-computed noise floor from analysis pass (CLI usage)
    pub fn init_with_noise_floor(&mut self, noise_floor: &[f32]) {
        assert_eq!(
            noise_floor.len(),
            self.stft.n_bins(),
            "Noise floor size mismatch"
        );
        self.noise_pow.copy_from_slice(noise_floor);
        self.needs_initialization = false;
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
        let mut frame_idx = 0usize;
        while i + window_size <= input.len() {
            let frame = &input[i..i + window_size];

            // Forward FFT
            let mut spectrum = self.stft.forward_fft(frame);

            // Compute power spectrum
            let power = self.stft.compute_power(&spectrum);

            // Initialize noise estimate from first frame
            if self.needs_initialization {
                self.initialize_noise_from_first_frame(&power);
            }

            // Get adaptive lambda for faster initial convergence
            let lambda = self.get_adaptive_lambda();

            // Bootstrap noise estimate with first WARMUP_FRAMES (warmup period)
            if self.frames_processed < WARMUP_FRAMES {
                // Force update with adaptive lambda during initial frames
                self.update_noise_estimate_with_lambda(&power, true, lambda);
                self.frames_processed += 1;
            } else {
                // Normal SFM-based VAD for noise estimation
                let sfm = compute_sfm(&power);

                if sfm > self.params.sfm_noise {
                    self.update_noise_estimate(&power);
                    self.prev_sfm_decision = true;
                } else if sfm < self.params.sfm_speech {
                    self.prev_sfm_decision = false;
                } else if self.prev_sfm_decision {
                    self.update_noise_estimate(&power);
                }
            }

            // Compute adaptive alpha
            let snr = self.compute_snr_per_bin(&power);
            let alpha = compute_alpha_curve(
                window_size,
                self.stft.sample_rate(),
                &snr,
                &self.params.delta,
                self.params.alpha_base,
                self.params.alpha_min,
                self.params.alpha_max,
            );

            // Compute and smooth gain
            let gain = self.compute_gain(&power, &alpha);
            let final_gain = self.smooth_gain(&gain);

            // Update visualization data if enabled
            if self.visualization_enabled {
                self.update_visualization_data(&power, &snr, &final_gain);
            }

            // Apply gain to spectrum
            for k in 0..n_bins {
                spectrum[k] *= final_gain[k];
            }

            // Ensure symmetry for real-valued output
            self.stft.ensure_symmetry(&mut spectrum);

            // Inverse FFT
            let synthesized = self.stft.inverse_fft(&mut spectrum);

            // Overlap-add
            let out_frame = self.stft.overlap_add(&synthesized);

            // Short cubic fade-in to avoid artifacts from unstable noise estimate
            if frame_idx < 4 {
                let t = (frame_idx + 1) as f32 / 4.0;
                let fade = t * t * t * t * t;
                output.extend(out_frame.iter().map(|&s| s * fade));
            } else {
                output.extend(out_frame);
            }

            frame_idx += 1;
            i += hop_size;
        }

        // Remove pre-padding and trim to original length
        if output.len() > pre_pad {
            output = output[pre_pad..].to_vec();
        }
        output.truncate(original_len);

        output
    }

    pub fn set_params(&mut self, params: DenoiserParams) {
        self.params = params;
        self.gamma_dirty = true;
    }

    pub fn set_visualization_enabled(&mut self, enabled: bool) {
        self.visualization_enabled = enabled;
        let n_bins = self.stft.n_bins();
        if enabled && self.cached_viz_data.current_spectrum.len() != n_bins {
            // Initialize with correct size
            self.cached_viz_data = VisualizationData {
                current_spectrum: vec![0.0; n_bins],
                noise_spectrum: vec![0.0; n_bins],
                band_gain_db: [0.0; NUM_BANDS],
                band_snr_db: [0.0; NUM_BANDS],
                sample_rate: self.stft.sample_rate(),
            };
        }
    }

    fn rebuild_gamma_curve(&mut self) {
        if self.gamma_dirty {
            self.gamma_curve = compute_gamma_curve(
                self.stft.window_size(),
                self.stft.sample_rate(),
                &self.params.gamma,
            );
            self.gamma_dirty = false;
        }
    }

    fn compute_snr_per_bin(&self, power: &[f32]) -> Vec<f32> {
        // Explicit for-loop enables LLVM auto-vectorization
        let len = power.len();
        let mut snr = vec![0.0; len];
        for i in 0..len {
            let snr_linear = power[i] / (self.noise_pow[i] + EPSILON);
            snr[i] = 10.0 * (snr_linear + EPSILON).log10();
        }
        snr
    }

    fn update_noise_estimate(&mut self, power: &[f32]) {
        let n_bins = self.stft.n_bins();
        for k in 0..n_bins {
            // Spike protection
            if power[k] > self.params.spike_threshold * self.noise_pow[k] {
                continue;
            }
            // Recursive update
            self.noise_pow[k] =
                self.params.lambda * self.noise_pow[k] + (1.0 - self.params.lambda) * power[k];
        }
    }

    fn update_noise_estimate_with_lambda(
        &mut self,
        power: &[f32],
        force_update: bool,
        lambda: f32,
    ) {
        let n_bins = self.stft.n_bins();
        for k in 0..n_bins {
            // Spike protection
            if !force_update && power[k] > self.params.spike_threshold * self.noise_pow[k] {
                continue;
            }
            // Recursive update with custom lambda
            self.noise_pow[k] = lambda * self.noise_pow[k] + (1.0 - lambda) * power[k];
        }
    }

    fn initialize_noise_from_first_frame(&mut self, power: &[f32]) {
        // Use bottom 20th percentile of first frame as initial guess
        let mut sorted = power.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let percentile_20 = sorted[sorted.len() / 5];

        let n_bins = self.stft.n_bins();
        for k in 0..n_bins {
            // Initialize conservatively: minimum of bin power or 20th percentile * 1.5
            self.noise_pow[k] = power[k].min(percentile_20 * 1.5).max(EPSILON);
        }
        self.needs_initialization = false;
    }

    fn get_adaptive_lambda(&self) -> f32 {
        // Fast convergence in first few frames
        if self.frames_processed < 1 {
            0.5 // Very fast initial convergence
        } else if self.frames_processed < WARMUP_FRAMES {
            0.75 // Medium convergence
        } else {
            self.params.lambda // Normal - maintains adaptation
        }
    }

    fn compute_gain(&self, power: &[f32], alpha: &[f32]) -> Vec<f32> {
        // Explicit for-loop enables LLVM auto-vectorization
        let len = power.len();
        let mut gain = vec![0.0; len];
        let beta = self.params.beta;
        for i in 0..len {
            let p = power[i];
            let a = alpha[i];
            let n = self.noise_pow[i];
            let subtracted = p - a * n;
            let floored = beta * p;
            let numerator = subtracted.max(floored);
            let g = (numerator / (p + EPSILON)).sqrt();
            gain[i] = g.clamp(0.0, 1.0);
        }
        gain
    }

    fn smooth_gain(&mut self, gain: &[f32]) -> Vec<f32> {
        self.rebuild_gamma_curve();

        // Explicit for-loop enables LLVM auto-vectorization
        let len = self.gamma_curve.len();
        let mut smoothed = vec![0.0; len];
        for i in 0..len {
            let g = self.gamma_curve[i];
            smoothed[i] = g * self.prev_gain[i] + (1.0 - g) * gain[i];
        }

        self.prev_gain.copy_from_slice(&smoothed);
        smoothed
    }

    pub fn process_frame(&mut self, frame: &[f32]) -> Vec<f32> {
        let window_size = self.stft.window_size();
        let n_bins = self.stft.n_bins();
        assert_eq!(frame.len(), window_size);

        // Forward FFT
        let mut spectrum = self.stft.forward_fft(frame);

        // Compute power spectrum
        let power = self.stft.compute_power(&spectrum);

        // Initialize noise estimate from first frame
        if self.needs_initialization {
            self.initialize_noise_from_first_frame(&power);
        }

        // Get adaptive lambda for faster initial convergence
        let lambda = self.get_adaptive_lambda();

        // Bootstrap noise estimate with first WARMUP_FRAMES (warmup period)
        if self.frames_processed < WARMUP_FRAMES {
            // Force update with adaptive lambda during initial frames
            self.update_noise_estimate_with_lambda(&power, true, lambda);
            self.frames_processed += 1;
        } else {
            // Normal SFM-based VAD for noise estimation
            let sfm = compute_sfm(&power);

            if sfm > self.params.sfm_noise {
                self.update_noise_estimate(&power);
                self.prev_sfm_decision = true;
            } else if sfm < self.params.sfm_speech {
                self.prev_sfm_decision = false;
            } else if self.prev_sfm_decision {
                self.update_noise_estimate(&power);
            }
        }

        // Compute adaptive alpha
        let snr = self.compute_snr_per_bin(&power);
        let alpha = compute_alpha_curve(
            window_size,
            self.stft.sample_rate(),
            &snr,
            &self.params.delta,
            self.params.alpha_base,
            self.params.alpha_min,
            self.params.alpha_max,
        );

        // Compute and smooth gain - apply at full strength from frame 0
        let gain = self.compute_gain(&power, &alpha);
        let final_gain = self.smooth_gain(&gain);

        // Update visualization data if enabled
        if self.visualization_enabled {
            self.update_visualization_data(&power, &snr, &final_gain);
        }

        // Apply gain to spectrum
        for k in 0..n_bins {
            spectrum[k] *= final_gain[k];
        }

        // Ensure symmetry for real-valued output
        self.stft.ensure_symmetry(&mut spectrum);

        // Inverse FFT
        let synthesized = self.stft.inverse_fft(&mut spectrum);

        // Overlap-add
        self.stft.overlap_add(&synthesized)
    }

    fn update_visualization_data(&mut self, power: &[f32], snr: &[f32], gain: &[f32]) {
        let n_bins = self.stft.n_bins();
        let window_size = self.stft.window_size();
        let sample_rate = self.stft.sample_rate();

        // Copy current power spectrum (for blue line)
        self.cached_viz_data.current_spectrum[..n_bins].copy_from_slice(power);

        // Copy noise floor (for red line)
        self.cached_viz_data.noise_spectrum[..n_bins].copy_from_slice(&self.noise_pow);

        // Compute per-band averages
        for (band_idx, &(start_hz, end_hz)) in BANDS.iter().enumerate() {
            let start_bin = hz_to_bin(start_hz, window_size, sample_rate);
            let end_bin = hz_to_bin(end_hz, window_size, sample_rate).min(n_bins);

            if start_bin >= end_bin {
                continue;
            }

            // Average SNR in this band
            let snr_sum: f32 = snr[start_bin..end_bin].iter().sum();
            let snr_avg = snr_sum / (end_bin - start_bin) as f32;
            self.cached_viz_data.band_snr_db[band_idx] = snr_avg;

            // Average gain in this band, convert to dB
            let gain_sum: f32 = gain[start_bin..end_bin].iter().sum();
            let gain_avg = gain_sum / (end_bin - start_bin) as f32;
            // Convert linear gain to dB attenuation (negative values)
            let gain_db = if gain_avg > 0.0 {
                20.0 * gain_avg.log10()
            } else {
                -60.0 // Floor at -60dB
            };
            self.cached_viz_data.band_gain_db[band_idx] = gain_db;
        }
    }

    pub fn get_visualization_data(&self) -> VisualizationData {
        self.cached_viz_data.clone()
    }

    pub fn reset(&mut self) {
        // Reset noise estimation state
        self.noise_pow.fill(EPSILON);
        self.prev_gain.fill(1.0);
        self.prev_sfm_decision = true;
        self.frames_processed = 0;
        self.needs_initialization = true;

        // Reset STFT processing buffers
        self.stft.reset();

        // Reset parameter cache - force gamma curve rebuild
        self.gamma_dirty = true;

        // Reset visualization data to show clean state
        self.cached_viz_data = VisualizationData::default();
    }

    pub fn latency_samples(&self) -> u32 {
        // The latency is the window size minus the hop size
        // This is the lookahead needed for the STFT
        (self.stft.window_size() - self.stft.hop_size()) as u32
    }
}

impl crate::traits::FrameProcessor for RealtimeDenoiser {
    fn process_frame(&mut self, frame: &[f32]) -> Vec<f32> {
        RealtimeDenoiser::process_frame(self, frame)
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn window_size(&self) -> usize {
        self.stft.window_size()
    }

    fn hop_size(&self) -> usize {
        self.stft.hop_size()
    }

    fn latency_samples(&self) -> usize {
        self.stft.window_size() - self.stft.hop_size()
    }
}

// =============================================================================
// Streaming Denoiser (Buffer-based wrapper)
// =============================================================================

/// Buffer-based wrapper around RealtimeDenoiser.
/// Handles frame accumulation and output buffering internally,
/// providing a simple `process_buffer(&mut [f32])` interface.
pub struct StreamingDenoiser {
    denoiser: RealtimeDenoiser,
    input_ring: Vec<f32>,
    output_buffer: VecDeque<f32>,
    window_size: usize,
    hop_size: usize,
}

impl StreamingDenoiser {
    pub fn new(sample_rate: u32) -> Self {
        let denoiser = RealtimeDenoiser::new(sample_rate);
        let window_size = denoiser.stft.window_size();
        let hop_size = denoiser.stft.hop_size();

        Self {
            denoiser,
            input_ring: Vec::with_capacity(window_size),
            output_buffer: VecDeque::with_capacity(window_size),
            window_size,
            hop_size,
        }
    }

    /// Set denoiser parameters
    pub fn set_params(&mut self, params: DenoiserParams) {
        self.denoiser.set_params(params);
    }

    /// Enable/disable visualization data collection
    pub fn set_visualization_enabled(&mut self, enabled: bool) {
        self.denoiser.set_visualization_enabled(enabled);
    }

    /// Get visualization data (for GUI display)
    pub fn get_visualization_data(&self) -> VisualizationData {
        self.denoiser.get_visualization_data()
    }

    /// Get latency in samples
    pub fn latency_samples(&self) -> usize {
        self.window_size - self.hop_size
    }

    /// Process a single sample
    /// Returns the output sample (0.0 during initial latency period)
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // Accumulate input sample
        self.input_ring.push(input);

        // Process frame when we have enough samples
        if self.input_ring.len() >= self.window_size {
            let output_frame = self.denoiser.process_frame(&self.input_ring);
            self.output_buffer.extend(output_frame);

            // Shift input ring by hop_size (overlap-add)
            self.input_ring.drain(..self.hop_size);
        }

        // Output sample (0.0 during initial latency period)
        self.output_buffer.pop_front().unwrap_or(0.0)
    }
}

impl crate::traits::AudioProcessor for StreamingDenoiser {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    fn reset(&mut self) {
        self.denoiser.reset();
        self.input_ring.clear();
        self.output_buffer.clear();
    }

    fn latency_samples(&self) -> usize {
        self.window_size - self.hop_size
    }
}

impl crate::traits::MonoProcessor for StreamingDenoiser {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_preserves_length() {
        let mut denoiser = RealtimeDenoiser::new(48000);

        // Test various lengths including non-multiples of hop size
        for &len in &[48000, 44100, 10000, 3333, 1024, 2048, 5000] {
            denoiser.reset();
            let input: Vec<f32> = (0..len).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();
            let output = denoiser.process(&input);
            assert_eq!(
                output.len(),
                input.len(),
                "Length mismatch for input size {}",
                len
            );
        }
    }

    #[test]
    fn test_process_empty() {
        let mut denoiser = RealtimeDenoiser::new(48000);
        let output = denoiser.process(&[]);
        assert!(output.is_empty());
    }
}
