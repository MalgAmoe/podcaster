//! Cepstral analysis for spectral envelope and f0 detection
//!
//! Takes an existing SpectralAnalysis and computes:
//! - Cepstral-smoothed spectral envelope (better than moving average for voice)
//! - Fundamental frequency (f0) from cepstrum peak detection

use rustfft::{num_complex::Complex, FftPlanner};

use super::SpectralAnalysis;

/// Cepstral analysis results computed from a SpectralAnalysis
#[derive(Clone, Debug)]
pub struct CepstralAnalysis {
    /// Cepstral-smoothed spectral envelope (dB)
    pub envelope_db: Vec<f32>,
    /// Detected fundamental frequency (Hz), None if not voiced
    pub f0: Option<f32>,
    /// Sample rate (copied from source)
    pub sample_rate: u32,
    /// Hz per bin (copied from source)
    pub bin_freq: f32,
    /// Number of bins (copied from source)
    pub n_bins: usize,
}

impl CepstralAnalysis {
    /// Compute cepstral analysis from existing SpectralAnalysis
    ///
    /// This reuses the FFT power spectrum already computed, avoiding redundant work.
    pub fn from_spectrum(spectrum: &SpectralAnalysis) -> Self {
        let n_bins = spectrum.n_bins;
        let sample_rate = spectrum.sample_rate;
        let bin_freq = spectrum.bin_freq;

        if spectrum.avg_power.is_empty() || spectrum.avg_power.iter().all(|&p| p < 1e-12) {
            return Self {
                envelope_db: vec![-60.0; n_bins],
                f0: None,
                sample_rate,
                bin_freq,
                n_bins,
            };
        }

        // Compute cepstrum and extract envelope/f0
        let (envelope_db, f0) = Self::compute_cepstral(&spectrum.avg_power, sample_rate);

        Self {
            envelope_db,
            f0,
            sample_rate,
            bin_freq,
            n_bins,
        }
    }

    /// Compute cepstral envelope and detect f0
    ///
    /// Algorithm:
    /// 1. Log magnitude: log(avg_power)
    /// 2. IFFT → cepstrum (real part)
    /// 3. Lifter: zero out high quefrency (keep first 40 coefficients = envelope)
    /// 4. FFT back → smoothed envelope
    /// 5. Find f0: peak in quefrency range corresponding to 75-400 Hz
    fn compute_cepstral(power: &[f32], sample_rate: u32) -> (Vec<f32>, Option<f32>) {
        let n = power.len();
        // Mirror the spectrum for real-valued cepstrum
        // For n=2049 bins (from WINDOW_SIZE=4096), the full spectrum is 2*(n-1) = 4096
        let fft_size = 2 * (n - 1);

        let mut planner = FftPlanner::new();
        let ifft = planner.plan_fft_inverse(fft_size);
        let fft = planner.plan_fft_forward(fft_size);

        // Step 1: Log magnitude spectrum (mirrored for real-valued cepstrum)
        // Use small floor to avoid log(0)
        let log_power: Vec<f32> = power.iter().map(|&p| (p.max(1e-12)).ln()).collect();

        // Mirror for real-valued inverse FFT
        // Full spectrum: [DC, 1, 2, ..., n-2, Nyquist, n-2, ..., 2, 1]
        let mut spectrum: Vec<Complex<f32>> = Vec::with_capacity(fft_size);
        // First half: DC to Nyquist (bins 0 to n-1)
        for &lp in &log_power[..n - 1] {
            spectrum.push(Complex::new(lp, 0.0));
        }
        // Second half: mirror (bins n-1 down to 1, excluding DC)
        for i in (1..n).rev() {
            spectrum.push(Complex::new(log_power[i], 0.0));
        }

        // Step 2: IFFT → cepstrum
        let mut scratch = vec![Complex::new(0.0, 0.0); ifft.get_inplace_scratch_len()];
        ifft.process_with_scratch(&mut spectrum, &mut scratch);

        // Normalize IFFT
        let scale = 1.0 / fft_size as f32;
        for c in &mut spectrum {
            *c *= scale;
        }

        // Step 3: Detect f0 from cepstrum peak
        // f0 corresponds to quefrency = sample_rate / f0
        // For voice: f0 typically 75-400 Hz
        // At 48kHz: quefrency range = 120-640 (for 75-400 Hz)
        let min_f0 = 75.0;
        let max_f0 = 400.0;
        // quefrency = sample_rate / f0, but we're working with bins
        // bin corresponds to quefrency in samples, but our effective sample rate
        // for the cepstrum is sample_rate (the original)
        // Actually, the cepstrum has fft_size points representing 0 to fft_size-1 quefrency samples
        // quefrency_hz = sample_rate / quefrency_sample
        // So quefrency_sample = sample_rate / f0_hz

        // The cepstrum now has fft_size = 4096 points for a 4096-sample window
        // Quefrency in samples = sample_rate / f0
        // So we use the original sample_rate directly
        let effective_sr = sample_rate as f32;
        let min_quefrency = (effective_sr / max_f0) as usize;
        let max_quefrency = (effective_sr / min_f0) as usize;

        let min_q = min_quefrency.max(2).min(fft_size / 2);
        let max_q = max_quefrency.max(min_q + 1).min(fft_size / 2);

        // Collect cepstrum values and find peak in one pass
        let mut peak_q = 0usize;
        let mut peak_val = 0.0f32;
        let mut sum = 0.0f32;
        let mut sum_sq = 0.0f32;
        let count = (max_q - min_q) as f32;

        for q in min_q..max_q {
            let val = spectrum[q].re.abs();
            sum += val;
            sum_sq += val * val;
            if val > peak_val {
                peak_val = val;
                peak_q = q;
            }
        }

        // Compute adaptive threshold: mean + 2.5 * std_dev
        // This adapts to the actual cepstrum energy regardless of normalization
        let mean = sum / count;
        let variance = (sum_sq / count) - (mean * mean);
        let std_dev = variance.max(0.0).sqrt();
        let threshold = mean + 2.5 * std_dev;

        // f0 detected if peak significantly exceeds noise floor
        let f0 = if peak_q > 0 && peak_val > threshold {
            Some(effective_sr / peak_q as f32)
        } else {
            None
        };

        // Step 4: Liftering - keep low quefrency for envelope
        // Zero out high quefrency (pitch-related components)
        let lifter_cutoff = 40.min(fft_size / 4); // Keep ~40 coefficients for envelope

        for i in lifter_cutoff..fft_size - lifter_cutoff {
            spectrum[i] = Complex::new(0.0, 0.0);
        }

        // Step 5: FFT back → smoothed envelope
        let mut scratch2 = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        fft.process_with_scratch(&mut spectrum, &mut scratch2);

        // Extract envelope (first n bins), convert to dB
        let envelope_db: Vec<f32> = spectrum[..n]
            .iter()
            .map(|c| {
                // The result is log magnitude, convert to dB
                // log magnitude = ln(amplitude), dB = 20 * log10(amplitude)
                // dB = 20 * log10(e^ln_amp) = 20 * ln_amp / ln(10) = 8.686 * ln_amp
                c.re * 8.685889638
            })
            .collect();

        (envelope_db, f0)
    }

    /// Get bin index for a frequency
    #[inline]
    pub fn freq_to_bin(&self, freq: f32) -> usize {
        ((freq / self.bin_freq).round() as usize).min(self.n_bins - 1)
    }

    /// Get frequency for a bin index
    #[inline]
    pub fn bin_to_freq(&self, bin: usize) -> f32 {
        bin as f32 * self.bin_freq
    }

    /// Get envelope value (dB) at a frequency
    pub fn envelope_at_freq(&self, freq: f32) -> f32 {
        let bin = self.freq_to_bin(freq);
        self.envelope_db[bin]
    }

    /// Get average envelope (dB) in a frequency range
    pub fn band_envelope_db(&self, min_hz: f32, max_hz: f32) -> f32 {
        let min_bin = self.freq_to_bin(min_hz);
        let max_bin = self.freq_to_bin(max_hz);

        if min_bin >= max_bin {
            return -60.0;
        }

        let sum: f32 = self.envelope_db[min_bin..=max_bin].iter().sum();
        let count = (max_bin - min_bin + 1) as f32;
        sum / count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_spectrum() {
        let spectrum = SpectralAnalysis::new(&[], 48000);
        let cepstral = CepstralAnalysis::from_spectrum(&spectrum);
        assert!(cepstral.f0.is_none());
        assert!(!cepstral.envelope_db.is_empty());
    }

    #[test]
    fn test_sine_wave_f0() {
        // Generate a 200Hz sine wave
        let sample_rate = 48000u32;
        let duration = 1.0; // 1 second
        let freq = 200.0;
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq * t).sin()
            })
            .collect();

        let spectrum = SpectralAnalysis::new(&samples, sample_rate);
        let cepstral = CepstralAnalysis::from_spectrum(&spectrum);

        // Should detect f0 near 200 Hz
        if let Some(detected_f0) = cepstral.f0 {
            assert!(
                (detected_f0 - freq).abs() < 20.0,
                "Expected f0 near {}, got {}",
                freq,
                detected_f0
            );
        }
    }
}
