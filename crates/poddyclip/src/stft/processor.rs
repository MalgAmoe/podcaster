//! STFT Processor - shared FFT infrastructure
//!
//! Provides reusable STFT (Short-Time Fourier Transform) processing
//! to eliminate FFT code duplication across processors.

use std::sync::Arc;

use rustfft::{num_complex::Complex, Fft, FftPlanner};

use super::types::StftConfig;
use super::window::create_window;

/// Shared STFT processor with FFT infrastructure and overlap-add
///
/// Processors compose with this struct instead of implementing their own FFT.
pub struct StftProcessor {
    // FFT infrastructure
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    fft_scratch: Vec<Complex<f32>>,

    // Window
    window: Vec<f32>,

    // Overlap-add buffer
    overlap_buffer: Vec<f32>,

    // Reusable work buffers (avoid per-frame allocations)
    work_spectrum: Vec<Complex<f32>>,
    work_output: Vec<f32>,
    work_power: Vec<f32>,

    // Configuration
    config: StftConfig,
    sample_rate: u32,
}

impl StftProcessor {
    /// Create a new STFT processor
    pub fn new(sample_rate: u32, config: StftConfig) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(config.window_size);
        let ifft = planner.plan_fft_inverse(config.window_size);
        let scratch_len = fft.get_inplace_scratch_len().max(ifft.get_inplace_scratch_len());
        let fft_scratch = vec![Complex::new(0.0, 0.0); scratch_len];
        let window = create_window(config.window_size, config.window_type);

        Self {
            fft,
            ifft,
            fft_scratch,
            window,
            overlap_buffer: vec![0.0; config.window_size],
            // Pre-allocate work buffers
            work_spectrum: vec![Complex::new(0.0, 0.0); config.window_size],
            work_output: vec![0.0; config.window_size],
            work_power: vec![0.0; config.n_bins()],
            config,
            sample_rate,
        }
    }

    /// Create with real-time config (2048/1024)
    pub fn new_realtime(sample_rate: u32) -> Self {
        Self::new(sample_rate, StftConfig::realtime())
    }

    /// Create with analysis config (4096/2048)
    pub fn new_analysis(sample_rate: u32) -> Self {
        Self::new(sample_rate, StftConfig::analysis())
    }

    // =========================================================================
    // Getters
    // =========================================================================

    pub fn window_size(&self) -> usize {
        self.config.window_size
    }

    pub fn hop_size(&self) -> usize {
        self.config.hop_size
    }

    pub fn n_bins(&self) -> usize {
        self.config.n_bins()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn config(&self) -> &StftConfig {
        &self.config
    }

    pub fn window(&self) -> &[f32] {
        &self.window
    }

    // =========================================================================
    // FFT Operations
    // =========================================================================

    /// Apply window and perform forward FFT on a frame
    ///
    /// Returns complex spectrum (full size, not just positive frequencies)
    pub fn forward_fft(&mut self, frame: &[f32]) -> Vec<Complex<f32>> {
        debug_assert_eq!(frame.len(), self.config.window_size);

        // Explicit for-loop enables LLVM auto-vectorization
        let window_size = self.config.window_size;
        let mut spectrum = vec![Complex::new(0.0, 0.0); window_size];
        for i in 0..window_size {
            spectrum[i] = Complex::new(frame[i] * self.window[i], 0.0);
        }

        // Forward FFT
        self.fft
            .process_with_scratch(&mut spectrum, &mut self.fft_scratch);

        spectrum
    }

    /// Apply window and perform forward FFT on a frame using internal buffer
    ///
    /// Returns a reference to the internal spectrum buffer. This avoids allocation
    /// but the buffer is only valid until the next FFT operation.
    pub fn forward_fft_inplace(&mut self, frame: &[f32]) -> &[Complex<f32>] {
        debug_assert_eq!(frame.len(), self.config.window_size);

        // Explicit for-loop enables LLVM auto-vectorization
        let window_size = self.config.window_size;
        for i in 0..window_size {
            self.work_spectrum[i] = Complex::new(frame[i] * self.window[i], 0.0);
        }

        // Forward FFT
        self.fft
            .process_with_scratch(&mut self.work_spectrum, &mut self.fft_scratch);

        &self.work_spectrum
    }

    /// Get mutable access to internal spectrum buffer (for modification after forward_fft_inplace)
    pub fn spectrum_mut(&mut self) -> &mut [Complex<f32>] {
        &mut self.work_spectrum
    }

    /// Perform inverse FFT, apply window, and return time-domain samples
    ///
    /// Input spectrum is modified in place.
    pub fn inverse_fft(&mut self, spectrum: &mut [Complex<f32>]) -> Vec<f32> {
        debug_assert_eq!(spectrum.len(), self.config.window_size);

        // Inverse FFT
        self.ifft
            .process_with_scratch(spectrum, &mut self.fft_scratch);

        // Explicit for-loop enables LLVM auto-vectorization
        let window_size = self.config.window_size;
        let scale = 1.0 / window_size as f32;
        let mut output = vec![0.0; window_size];
        for i in 0..window_size {
            output[i] = spectrum[i].re * scale * self.window[i];
        }
        output
    }

    /// Perform inverse FFT using internal buffer, returns reference to output
    ///
    /// Uses the internal work_spectrum as input and work_output for result.
    pub fn inverse_fft_inplace(&mut self) -> &[f32] {
        // Inverse FFT on internal spectrum buffer
        self.ifft
            .process_with_scratch(&mut self.work_spectrum, &mut self.fft_scratch);

        // Explicit for-loop enables LLVM auto-vectorization
        let window_size = self.config.window_size;
        let scale = 1.0 / window_size as f32;
        for i in 0..window_size {
            self.work_output[i] = self.work_spectrum[i].re * scale * self.window[i];
        }
        &self.work_output
    }

    /// Compute power spectrum from complex spectrum
    ///
    /// Returns only positive frequencies (n_bins)
    pub fn compute_power(&self, spectrum: &[Complex<f32>]) -> Vec<f32> {
        // Explicit for-loop enables LLVM auto-vectorization
        let n_bins = self.n_bins();
        let mut power = vec![0.0; n_bins];
        for i in 0..n_bins {
            power[i] = spectrum[i].norm_sqr();
        }
        power
    }

    /// Compute power spectrum into internal buffer, returns reference
    pub fn compute_power_inplace(&mut self) -> &[f32] {
        let n_bins = self.n_bins();
        for i in 0..n_bins {
            self.work_power[i] = self.work_spectrum[i].norm_sqr();
        }
        &self.work_power
    }

    /// Ensure conjugate symmetry for real IFFT
    ///
    /// After modifying positive frequency bins, call this to maintain symmetry.
    pub fn ensure_symmetry(&self, spectrum: &mut [Complex<f32>]) {
        let n_bins = self.n_bins();
        let window_size = self.config.window_size;
        for k in n_bins..window_size {
            spectrum[k] = spectrum[window_size - k].conj();
        }
    }

    // =========================================================================
    // Overlap-Add
    // =========================================================================

    /// Add synthesized frame to overlap buffer and return output samples
    ///
    /// Returns hop_size samples of output.
    pub fn overlap_add(&mut self, synthesized: &[f32]) -> Vec<f32> {
        debug_assert_eq!(synthesized.len(), self.config.window_size);

        // Add to overlap buffer
        for i in 0..self.config.window_size {
            self.overlap_buffer[i] += synthesized[i];
        }

        // Extract output (first hop_size samples)
        let output: Vec<f32> = self.overlap_buffer[..self.config.hop_size].to_vec();

        // Shift buffer
        self.overlap_buffer.rotate_left(self.config.hop_size);
        let clear_start = self.config.window_size - self.config.hop_size;
        for i in clear_start..self.config.window_size {
            self.overlap_buffer[i] = 0.0;
        }

        output
    }

    /// Add synthesized frame (from internal work_output) to overlap buffer
    /// and copy output to provided slice
    ///
    /// This version avoids allocation by using caller-provided output buffer.
    pub fn overlap_add_inplace(&mut self, output: &mut [f32]) {
        debug_assert!(output.len() >= self.config.hop_size);

        // Add internal work_output to overlap buffer
        for i in 0..self.config.window_size {
            self.overlap_buffer[i] += self.work_output[i];
        }

        // Copy output (first hop_size samples)
        output[..self.config.hop_size].copy_from_slice(&self.overlap_buffer[..self.config.hop_size]);

        // Shift buffer
        self.overlap_buffer.rotate_left(self.config.hop_size);
        let clear_start = self.config.window_size - self.config.hop_size;
        for i in clear_start..self.config.window_size {
            self.overlap_buffer[i] = 0.0;
        }
    }

    /// Reset overlap buffer and work buffers
    pub fn reset(&mut self) {
        self.overlap_buffer.fill(0.0);
        for c in &mut self.work_spectrum {
            *c = Complex::new(0.0, 0.0);
        }
        self.work_output.fill(0.0);
        self.work_power.fill(0.0);
    }

    // =========================================================================
    // Batch Processing
    // =========================================================================

    /// Process entire audio buffer with a custom spectrum processor
    ///
    /// The processor function receives:
    /// - power: power spectrum (positive frequencies only)
    /// - spectrum: full complex spectrum (modify in place)
    ///
    /// # Example
    /// ```ignore
    /// let output = stft.process(&audio, |power, spectrum| {
    ///     // Modify spectrum based on power
    ///     for (k, s) in spectrum[..n_bins].iter_mut().enumerate() {
    ///         *s *= gain[k];
    ///     }
    /// });
    /// ```
    pub fn process<F>(&mut self, audio: &[f32], mut processor: F) -> Vec<f32>
    where
        F: FnMut(&[f32], &mut [Complex<f32>]),
    {
        let original_len = audio.len();
        let window_size = self.config.window_size;
        let hop_size = self.config.hop_size;

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
            let mut spectrum = self.forward_fft(frame);

            // Compute power for convenience
            let power = self.compute_power(&spectrum);

            // Apply processor
            processor(&power, &mut spectrum);

            // Ensure symmetry
            self.ensure_symmetry(&mut spectrum);

            // Inverse FFT
            let synthesized = self.inverse_fft(&mut spectrum);

            // Overlap-add
            let out_frame = self.overlap_add(&synthesized);
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

    // =========================================================================
    // Utility
    // =========================================================================

    /// Convert frequency in Hz to bin index
    pub fn hz_to_bin(&self, hz: f32) -> usize {
        (hz * self.config.window_size as f32 / self.sample_rate as f32) as usize
    }

    /// Convert bin index to frequency in Hz
    pub fn bin_to_hz(&self, bin: usize) -> f32 {
        bin as f32 * self.sample_rate as f32 / self.config.window_size as f32
    }

    /// Get frequency resolution (Hz per bin)
    pub fn freq_resolution(&self) -> f32 {
        self.sample_rate as f32 / self.config.window_size as f32
    }
}

impl Clone for StftProcessor {
    fn clone(&self) -> Self {
        // Re-create FFT plans (they're reference-counted internally)
        Self::new(self.sample_rate, self.config.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_realtime() {
        let stft = StftProcessor::new_realtime(48000);
        assert_eq!(stft.window_size(), 2048);
        assert_eq!(stft.hop_size(), 1024);
        assert_eq!(stft.n_bins(), 1025);
    }

    #[test]
    fn test_create_analysis() {
        let stft = StftProcessor::new_analysis(48000);
        assert_eq!(stft.window_size(), 4096);
        assert_eq!(stft.hop_size(), 2048);
        assert_eq!(stft.n_bins(), 2049);
    }

    #[test]
    fn test_hz_to_bin() {
        let stft = StftProcessor::new_realtime(48000);
        // At 48kHz with 2048 FFT, each bin is 23.4375 Hz
        assert_eq!(stft.hz_to_bin(0.0), 0);
        assert_eq!(stft.hz_to_bin(1000.0), 42); // 1000 / 23.4375 ≈ 42
    }

    #[test]
    fn test_process_passthrough() {
        let mut stft = StftProcessor::new_realtime(48000);

        // Create test signal (1 second)
        let samples: Vec<f32> = (0..48000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();

        // Process with identity (no modification)
        let output = stft.process(&samples, |_power, _spectrum| {
            // No modification - should reconstruct original
        });

        // Output should be same length
        assert_eq!(output.len(), samples.len());

        // Should be close to original (within overlap-add tolerance)
        let max_diff: f32 = samples
            .iter()
            .zip(output.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);

        assert!(
            max_diff < 0.1,
            "Passthrough reconstruction error too high: {}",
            max_diff
        );
    }

    #[test]
    fn test_process_gain() {
        let mut stft = StftProcessor::new_realtime(48000);
        let n_bins = stft.n_bins();

        // Create test signal
        let samples: Vec<f32> = (0..48000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();

        // Apply 0.5 gain to all bins
        let output = stft.process(&samples, |_power, spectrum| {
            for s in spectrum[..n_bins].iter_mut() {
                *s *= 0.5;
            }
        });

        // Output should be roughly half amplitude
        let input_rms: f32 = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        let output_rms: f32 = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();

        let ratio = output_rms / input_rms;
        assert!(
            (ratio - 0.5).abs() < 0.1,
            "Gain ratio should be ~0.5, got {}",
            ratio
        );
    }
}
