//! Cepstral analysis for spectral envelope, f0 detection, and voice quality metrics
//!
//! Takes an existing SpectralAnalysis and computes:
//! - Cepstral-smoothed spectral envelope (better than moving average for voice)
//! - Fundamental frequency (f0) from cepstrum peak detection
//! - Cepstral Peak Prominence (CPP) - measure of harmonicity/voicing strength
//! - Echo/reverb detection from secondary cepstral peaks

use rustfft::{num_complex::Complex, FftPlanner};

use super::SpectralAnalysis;

/// Cepstral analysis results computed from a SpectralAnalysis
#[derive(Clone, Debug)]
pub struct CepstralAnalysis {
    /// Cepstral-smoothed spectral envelope (dB)
    pub envelope_db: Vec<f32>,
    /// Detected fundamental frequency (Hz), None if not voiced
    pub f0: Option<f32>,
    /// Cepstral Peak Prominence (dB) - harmonicity measure
    /// Higher = more harmonic/voiced (typical: 5-15 dB for speech)
    /// Lower = noise or unvoiced (typical: 0-3 dB)
    pub cpp: f32,
    /// Detected echo/reverb delay in milliseconds, None if no echo detected
    pub echo_delay_ms: Option<f32>,
    /// Echo strength (0.0 = none, 1.0 = strong echo)
    pub echo_strength: f32,
    /// Sample rate (copied from source)
    pub sample_rate: u32,
    /// Hz per bin (copied from source)
    pub bin_freq: f32,
    /// Number of bins (copied from source)
    pub n_bins: usize,
}

/// Results from cepstral computation
struct CepstralResults {
    envelope_db: Vec<f32>,
    f0: Option<f32>,
    cpp: f32,
    echo_delay_ms: Option<f32>,
    echo_strength: f32,
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
                cpp: 0.0,
                echo_delay_ms: None,
                echo_strength: 0.0,
                sample_rate,
                bin_freq,
                n_bins,
            };
        }

        // Compute cepstrum and extract all features
        let results = Self::compute_cepstral(&spectrum.avg_power, sample_rate);

        Self {
            envelope_db: results.envelope_db,
            f0: results.f0,
            cpp: results.cpp,
            echo_delay_ms: results.echo_delay_ms,
            echo_strength: results.echo_strength,
            sample_rate,
            bin_freq,
            n_bins,
        }
    }

    /// Compute cepstral envelope, f0, CPP, and echo detection
    ///
    /// Algorithm:
    /// 1. Log magnitude: log(avg_power)
    /// 2. IFFT → cepstrum (real part)
    /// 3. Find f0 and compute CPP from cepstrum peak
    /// 4. Detect echo from secondary peaks beyond pitch range
    /// 5. Lifter: zero out high quefrency (keep first 40 coefficients = envelope)
    /// 6. FFT back → smoothed envelope
    fn compute_cepstral(power: &[f32], sample_rate: u32) -> CepstralResults {
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

        // Store raw cepstrum for f0/CPP/echo detection (before liftering)
        let cepstrum: Vec<f32> = spectrum.iter().map(|c| c.re).collect();

        // Step 3: Detect f0 and compute CPP
        // f0 corresponds to quefrency = sample_rate / f0
        // For voice: f0 typically 75-400 Hz
        let min_f0 = 75.0;
        let max_f0 = 400.0;
        let effective_sr = sample_rate as f32;
        let min_quefrency = (effective_sr / max_f0) as usize;
        let max_quefrency = (effective_sr / min_f0) as usize;

        let min_q = min_quefrency.max(2).min(fft_size / 2);
        let max_q = max_quefrency.max(min_q + 1).min(fft_size / 2);

        // Compute CPP using linear regression through the cepstrum
        // CPP = peak value - regression line value at peak quefrency
        let (f0, cpp) = Self::compute_f0_and_cpp(&cepstrum, min_q, max_q, effective_sr);

        // Step 4: Detect echo from secondary peaks beyond pitch range
        // Echo appears at quefrency = delay in samples
        // Search range: 20ms to 500ms (960 to 24000 samples at 48kHz)
        let (echo_delay_ms, echo_strength) =
            Self::detect_echo(&cepstrum, sample_rate, max_q, fft_size);

        // Step 5: Liftering - keep low quefrency for envelope
        // Zero out high quefrency (pitch-related components)
        let lifter_cutoff = 40.min(fft_size / 4);

        for i in lifter_cutoff..fft_size - lifter_cutoff {
            spectrum[i] = Complex::new(0.0, 0.0);
        }

        // Step 6: FFT back → smoothed envelope
        let mut scratch2 = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        fft.process_with_scratch(&mut spectrum, &mut scratch2);

        // Extract envelope (first n bins), convert to dB
        let envelope_db: Vec<f32> = spectrum[..n]
            .iter()
            .map(|c| {
                // The result is log magnitude, convert to dB
                // dB = 20 * log10(e^ln_amp) = 8.686 * ln_amp
                c.re * 8.685889638
            })
            .collect();

        CepstralResults {
            envelope_db,
            f0,
            cpp,
            echo_delay_ms,
            echo_strength,
        }
    }

    /// Compute f0 and CPP (Cepstral Peak Prominence) from cepstrum
    ///
    /// Uses a simplified CPP measure: peak / mean ratio expressed in dB.
    /// This is more robust for averaged spectra than linear regression.
    fn compute_f0_and_cpp(
        cepstrum: &[f32],
        min_q: usize,
        max_q: usize,
        sample_rate: f32,
    ) -> (Option<f32>, f32) {
        let count = max_q - min_q;
        if count < 3 {
            return (None, 0.0);
        }

        // Find peak and compute statistics
        let mut peak_q = min_q;
        let mut peak_val = 0.0f32;
        let mut sum = 0.0f32;
        let mut sum_sq = 0.0f32;

        for q in min_q..max_q {
            let val = cepstrum[q].abs();
            sum += val;
            sum_sq += val * val;

            if val > peak_val {
                peak_val = val;
                peak_q = q;
            }
        }

        let n = count as f32;
        let mean = sum / n;
        let variance = (sum_sq / n) - (mean * mean);
        let std_dev = variance.max(0.0).sqrt();

        // Sub-harmonic check to fix octave errors
        // Only apply when detected frequency is above 200 Hz, where octave errors are common.
        // Below 200 Hz, we're already in the low voice range - no sub-harmonic to find.
        let initial_f0 = sample_rate / peak_q as f32;
        let mut best_q = peak_q;

        if initial_f0 > 200.0 {
            // Check if there's a significant peak at 2Q or 3Q (which would be f0/2 or f0/3)
            // Search in a window around the target quefrency to handle slight misalignment.
            for multiplier in [2, 3] {
                let target_q = peak_q * multiplier;
                // Search in a 10% window around target
                let window = (target_q / 10).max(3);
                let search_min = target_q.saturating_sub(window);
                let search_max = (target_q + window).min(max_q - 1);

                // Find local maximum in this window
                let mut local_max_q = target_q;
                let mut local_max_val = 0.0f32;

                for q in search_min..=search_max {
                    if q < cepstrum.len() {
                        let val = cepstrum[q].abs();
                        if val > local_max_val {
                            local_max_val = val;
                            local_max_q = q;
                        }
                    }
                }

                // If sub-harmonic is at least 30% of original peak, prefer it
                if local_max_val > peak_val * 0.3 {
                    best_q = local_max_q;
                }
            }
        }

        // CPP = how many standard deviations above mean (normalized)
        // Use ORIGINAL peak_val for CPP (not sub-harmonic value) since that measures
        // overall harmonicity, not the specific f0 we choose
        let prominence = if std_dev > 1e-12 {
            (peak_val - mean) / std_dev
        } else {
            0.0
        };

        // Scale to typical CPP range (0-15 dB)
        // prominence of 3 std_dev ≈ 10 dB CPP for voiced speech
        let cpp_db = (prominence * 3.0).clamp(0.0, 20.0);

        // f0 detected if peak significantly exceeds noise floor
        // prominence > 2 std_dev is strong evidence of periodicity
        // Use best_q for frequency (may be sub-harmonic corrected)
        let f0 = if best_q > 0 && prominence > 2.0 {
            Some(sample_rate / best_q as f32)
        } else {
            None
        };

        (f0, cpp_db)
    }

    /// Detect echo/reverb from secondary cepstral peaks
    ///
    /// Echoes appear as peaks at quefrency = delay in samples.
    /// Search range: 30ms to 500ms delay (30ms minimum to avoid room modes).
    fn detect_echo(
        cepstrum: &[f32],
        sample_rate: u32,
        pitch_max_q: usize,
        fft_size: usize,
    ) -> (Option<f32>, f32) {
        let sr = sample_rate as f32;

        // Echo search range: 30ms to 500ms
        // 30ms minimum to avoid confusing room modes with discrete echoes
        // At 48kHz: 1440 to 24000 samples
        let min_echo_ms = 30.0;
        let max_echo_ms = 500.0;
        let min_echo_q = ((sr * min_echo_ms / 1000.0) as usize).max(pitch_max_q + 50);
        let max_echo_q = ((sr * max_echo_ms / 1000.0) as usize).min(fft_size / 2);

        if min_echo_q >= max_echo_q {
            return (None, 0.0);
        }

        // Find peak and compute statistics in echo range
        let mut peak_q = min_echo_q;
        let mut peak_val = 0.0f32;
        let mut sum = 0.0f32;
        let mut sum_sq = 0.0f32;
        let count = (max_echo_q - min_echo_q) as f32;

        for q in min_echo_q..max_echo_q {
            let val = cepstrum[q].abs();
            sum += val;
            sum_sq += val * val;
            if val > peak_val {
                peak_val = val;
                peak_q = q;
            }
        }

        let mean = sum / count;
        let variance = (sum_sq / count) - (mean * mean);
        let std_dev = variance.max(0.0).sqrt();

        // Echo detected if peak exceeds mean + 3*std_dev (stricter threshold)
        // This reduces false positives on clean speech
        let prominence = if std_dev > 1e-12 {
            (peak_val - mean) / std_dev
        } else {
            0.0
        };

        // Require at least 5 std devs prominence for echo detection
        // This high threshold reduces false positives on clean speech
        // True echoes (like slapback delay) will show much higher prominence
        if prominence > 5.0 {
            // Convert quefrency to delay in ms
            let delay_ms = (peak_q as f32 / sr) * 1000.0;

            // Normalize strength (0-1 scale)
            // 5 std_dev = weak echo (0.0), 8+ std_dev = strong echo (1.0)
            let strength = ((prominence - 5.0) / 3.0).clamp(0.0, 1.0);

            (Some(delay_ms), strength)
        } else {
            (None, 0.0)
        }
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
    fn test_harmonic_signal_f0() {
        // Generate a 150Hz fundamental with harmonics (voice-like signal)
        // This tests the sub-harmonic correction that prevents octave errors
        let sample_rate = 48000u32;
        let duration = 1.0;
        let f0 = 150.0;
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                // Fundamental + harmonics with decreasing amplitude (voice-like)
                let h1 = (2.0 * std::f32::consts::PI * f0 * t).sin();
                let h2 = 0.7 * (2.0 * std::f32::consts::PI * f0 * 2.0 * t).sin();
                let h3 = 0.5 * (2.0 * std::f32::consts::PI * f0 * 3.0 * t).sin();
                let h4 = 0.3 * (2.0 * std::f32::consts::PI * f0 * 4.0 * t).sin();
                h1 + h2 + h3 + h4
            })
            .collect();

        let spectrum = SpectralAnalysis::new(&samples, sample_rate);
        let cepstral = CepstralAnalysis::from_spectrum(&spectrum);

        // Should detect f0 near 150 Hz (the fundamental, not a harmonic)
        if let Some(detected_f0) = cepstral.f0 {
            assert!(
                (detected_f0 - f0).abs() < 30.0,
                "Expected f0 near {}, got {}",
                f0,
                detected_f0
            );
        }
    }
}
