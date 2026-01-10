//! Shared audio analysis module
//!
//! Computes spectral analysis once for use by multiple processors.

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

const WINDOW_SIZE: usize = 4096;
const HOP_SIZE: usize = 2048;
const SMOOTHING_WINDOW: usize = 15;

/// Shared spectral analysis data
#[derive(Clone, Debug)]
pub struct SpectralAnalysis {
    /// Average power spectrum (linear, not dB)
    pub avg_power: Vec<f32>,
    /// Smoothed power spectrum (for deviation analysis)
    pub smoothed: Vec<f32>,
    /// Hz per FFT bin
    pub bin_freq: f32,
    /// Sample rate
    pub sample_rate: u32,
    /// Number of valid bins
    pub n_bins: usize,
}

impl SpectralAnalysis {
    /// Compute spectral analysis from mono audio
    pub fn new(audio: &[f32], sample_rate: u32) -> Self {
        let n_bins = WINDOW_SIZE / 2 + 1;
        let bin_freq = sample_rate as f32 / WINDOW_SIZE as f32;

        if audio.is_empty() {
            return Self {
                avg_power: vec![0.0; n_bins],
                smoothed: vec![0.0; n_bins],
                bin_freq,
                sample_rate,
                n_bins,
            };
        }

        // FFT setup
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(WINDOW_SIZE);
        let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

        // Hann window
        let window: Vec<f32> = (0..WINDOW_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (WINDOW_SIZE - 1) as f32).cos()))
            .collect();

        // Accumulate power spectrum across frames
        let mut avg_power = vec![0.0f32; n_bins];
        let mut frame_count = 0usize;

        let mut i = 0;
        while i + WINDOW_SIZE <= audio.len() {
            let frame = &audio[i..i + WINDOW_SIZE];

            let windowed: Vec<f32> = frame
                .iter()
                .zip(window.iter())
                .map(|(&s, &w)| s * w)
                .collect();

            let mut spectrum: Vec<Complex<f32>> =
                windowed.iter().map(|&s| Complex::new(s, 0.0)).collect();
            fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

            for (j, c) in spectrum[..n_bins].iter().enumerate() {
                avg_power[j] += c.norm_sqr();
            }
            frame_count += 1;
            i += HOP_SIZE;
        }

        if frame_count > 0 {
            for p in &mut avg_power {
                *p /= frame_count as f32;
            }
        }

        // Compute smoothed spectrum
        let smoothed = Self::compute_smoothed(&avg_power, SMOOTHING_WINDOW);

        Self {
            avg_power,
            smoothed,
            bin_freq,
            sample_rate,
            n_bins,
        }
    }

    /// Compute smoothed spectrum using moving average
    fn compute_smoothed(power: &[f32], window_size: usize) -> Vec<f32> {
        let mut smoothed = vec![0.0f32; power.len()];
        let half_window = window_size / 2;

        for bin in 0..power.len() {
            let start = bin.saturating_sub(half_window);
            let end = (bin + half_window + 1).min(power.len());
            let sum: f32 = power[start..end].iter().sum();
            let count = (end - start) as f32;
            smoothed[bin] = sum / count;
        }
        smoothed
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

    /// Get average energy (dB) in a frequency range
    pub fn band_energy_db(&self, min_hz: f32, max_hz: f32) -> f32 {
        let min_bin = self.freq_to_bin(min_hz);
        let max_bin = self.freq_to_bin(max_hz);

        if min_bin >= max_bin {
            return -60.0;
        }

        let sum: f32 = self.avg_power[min_bin..=max_bin].iter().sum();
        let count = (max_bin - min_bin + 1) as f32;
        let avg_power = sum / count;

        10.0 * avg_power.max(1e-12).log10()
    }

    /// Compute spectral tilt in dB (positive = bright, negative = dark)
    /// Compares low-mid (200-500Hz) to presence/air (2-8kHz)
    pub fn tilt_db(&self) -> f32 {
        let low_energy = self.band_energy_db(200.0, 500.0);
        let high_energy = self.band_energy_db(2000.0, 8000.0);
        high_energy - low_energy
    }

    /// Find peak deviation in a frequency range
    /// Returns (freq, energy_db, confidence)
    pub fn find_peak_deviation(
        &self,
        min_hz: f32,
        max_hz: f32,
        default_freq: f32,
    ) -> (f32, f32, f32) {
        let min_bin = self.freq_to_bin(min_hz);
        let max_bin = self.freq_to_bin(max_hz);

        if min_bin >= max_bin {
            return (default_freq, -60.0, 0.0);
        }

        let mut peak_bin = min_bin;
        let mut peak_deviation = 0.0f32;

        for bin in min_bin..=max_bin {
            let deviation = if self.smoothed[bin] > 1e-12 {
                self.avg_power[bin] / self.smoothed[bin]
            } else {
                1.0
            };

            if deviation > peak_deviation {
                peak_deviation = deviation;
                peak_bin = bin;
            }
        }

        let freq = self.bin_to_freq(peak_bin);
        let energy_db = 10.0 * self.avg_power[peak_bin].max(1e-12).log10();
        // Confidence: deviation of 1.5 = 50%, 2.0 = 100%
        let confidence = ((peak_deviation - 1.0) / 1.0).clamp(0.0, 1.0);

        (freq, energy_db, confidence)
    }

    /// Find two peaks with minimum spacing between them
    pub fn find_two_peaks_deviation(
        &self,
        min_hz: f32,
        max_hz: f32,
        min_spacing_hz: f32,
        default_freq_a: f32,
        default_freq_b: f32,
    ) -> ((f32, f32, f32), (f32, f32, f32)) {
        let min_bin = self.freq_to_bin(min_hz);
        let max_bin = self.freq_to_bin(max_hz);
        let min_spacing_bins = (min_spacing_hz / self.bin_freq).ceil() as usize;

        if min_bin >= max_bin {
            return (
                (default_freq_a, -60.0, 0.0),
                (default_freq_b, -60.0, 0.0),
            );
        }

        // Compute deviation for range
        let deviation: Vec<f32> = (0..self.n_bins)
            .map(|bin| {
                if bin >= min_bin && bin <= max_bin && self.smoothed[bin] > 1e-12 {
                    self.avg_power[bin] / self.smoothed[bin]
                } else {
                    0.0
                }
            })
            .collect();

        // Find first peak
        let mut peak1_bin = min_bin;
        let mut peak1_dev = deviation[min_bin];

        for bin in min_bin..=max_bin {
            if deviation[bin] > peak1_dev {
                peak1_dev = deviation[bin];
                peak1_bin = bin;
            }
        }

        // Find second peak with spacing
        let mut peak2_bin = min_bin;
        let mut peak2_dev = 0.0f32;

        for bin in min_bin..=max_bin {
            let distance = (bin as isize - peak1_bin as isize).unsigned_abs();
            if distance >= min_spacing_bins && deviation[bin] > peak2_dev {
                peak2_dev = deviation[bin];
                peak2_bin = bin;
            }
        }

        let build = |bin: usize, dev: f32| {
            (
                self.bin_to_freq(bin),
                10.0 * self.avg_power[bin].max(1e-12).log10(),
                ((dev - 1.0) / 1.0).clamp(0.0, 1.0),
            )
        };

        let result1 = build(peak1_bin, peak1_dev);
        let result2 = if peak2_dev > 1.0 {
            build(peak2_bin, peak2_dev)
        } else {
            (default_freq_b, -60.0, 0.0)
        };

        // Return sorted by frequency
        if result1.0 < result2.0 {
            (result1, result2)
        } else {
            (result2, result1)
        }
    }
}

/// Mix stereo to mono for analysis
pub fn mix_to_mono(left: &[f32], right: &[f32]) -> Vec<f32> {
    left.iter()
        .zip(right.iter())
        .map(|(&l, &r)| (l + r) * 0.5)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_audio() {
        let analysis = SpectralAnalysis::new(&[], 48000);
        // Empty audio has zero power, which is -120dB (10 * log10(1e-12))
        assert!(analysis.band_energy_db(100.0, 1000.0) < -100.0);
    }

    #[test]
    fn test_mix_to_mono() {
        let left = vec![1.0, 0.5, 0.0];
        let right = vec![0.0, 0.5, 1.0];
        let mono = mix_to_mono(&left, &right);
        assert_eq!(mono, vec![0.5, 0.5, 0.5]);
    }
}
