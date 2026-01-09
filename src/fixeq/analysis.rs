//! Audio analysis for FixEq
//!
//! Analyzes denoised audio to find optimal frequencies for dynamic EQ bands.
#![cfg_attr(all(feature = "cli", feature = "plugin"), allow(dead_code))]

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

const WINDOW_SIZE: usize = 4096; // Larger window for better low-freq resolution
const HOP_SIZE: usize = 2048;

/// Mud frequency range (Hz)
const MUD_FREQ_MIN: f32 = 150.0;
const MUD_FREQ_MAX: f32 = 500.0;

/// Correction frequency range (Hz) - mid-range for resonances/harshness
const CORRECTION_FREQ_MIN: f32 = 500.0;
const CORRECTION_FREQ_MAX: f32 = 5000.0;

/// Minimum spacing between correction peaks (in bins) to avoid picking adjacent frequencies
const CORRECTION_MIN_SPACING_BINS: usize = 20; // ~200Hz at 48kHz

/// Result of band frequency analysis
#[derive(Clone, Debug)]
pub struct BandAnalysis {
    /// Detected center frequency (Hz)
    pub center_freq: f32,
    /// Energy level at the detected frequency (dB)
    pub energy_db: f32,
    /// Confidence score (0.0 = uncertain, 1.0 = very confident)
    pub confidence: f32,
}

/// Combined analysis for all FixEq bands
#[derive(Clone, Debug)]
pub struct FixEqAnalysis {
    pub mud: BandAnalysis,
    pub correction_a: BandAnalysis,
    pub correction_b: BandAnalysis,
}

/// Mix stereo to mono for analysis
pub fn mix_to_mono(left: &[f32], right: &[f32]) -> Vec<f32> {
    left.iter()
        .zip(right.iter())
        .map(|(&l, &r)| (l + r) * 0.5)
        .collect()
}

/// Default correction frequencies (Hz)
pub const DEFAULT_CORRECTION_A_FREQ: f32 = 1000.0;
pub const DEFAULT_CORRECTION_B_FREQ: f32 = 3000.0;

/// Unified analysis for all FixEq bands (single FFT pass)
pub fn analyze_audio(audio: &[f32], sample_rate: u32) -> FixEqAnalysis {
    let default_analysis = || FixEqAnalysis {
        mud: BandAnalysis {
            center_freq: 300.0,
            energy_db: -60.0,
            confidence: 0.0,
        },
        correction_a: BandAnalysis {
            center_freq: DEFAULT_CORRECTION_A_FREQ,
            energy_db: -60.0,
            confidence: 0.0,
        },
        correction_b: BandAnalysis {
            center_freq: DEFAULT_CORRECTION_B_FREQ,
            energy_db: -60.0,
            confidence: 0.0,
        },
    };

    if audio.is_empty() {
        return default_analysis();
    }

    let n_bins = WINDOW_SIZE / 2 + 1;
    let bin_freq = sample_rate as f32 / WINDOW_SIZE as f32;

    // Calculate bin ranges
    let mud_min_bin = (MUD_FREQ_MIN / bin_freq).ceil() as usize;
    let mud_max_bin = (MUD_FREQ_MAX / bin_freq).floor() as usize;
    let mud_max_bin = mud_max_bin.min(n_bins - 1);

    let corr_min_bin = (CORRECTION_FREQ_MIN / bin_freq).ceil() as usize;
    let corr_max_bin = (CORRECTION_FREQ_MAX / bin_freq).floor() as usize;
    let corr_max_bin = corr_max_bin.min(n_bins - 1);

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

        let mut spectrum: Vec<Complex<f32>> = windowed
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();
        fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

        for (j, c) in spectrum[..n_bins].iter().enumerate() {
            avg_power[j] += c.norm_sqr();
        }
        frame_count += 1;
        i += HOP_SIZE;
    }

    if frame_count == 0 {
        return default_analysis();
    }

    for p in &mut avg_power {
        *p /= frame_count as f32;
    }

    // Compute smoothed spectrum once for all bands
    let smoothed = compute_smoothed_spectrum(&avg_power, SMOOTHING_WINDOW);

    // Analyze all bands using spectral deviation method
    let mud = find_peak_by_deviation(&avg_power, &smoothed, mud_min_bin, mud_max_bin, bin_freq, 300.0);
    let (correction_a, correction_b) = find_two_peaks_by_deviation(
        &avg_power,
        &smoothed,
        corr_min_bin,
        corr_max_bin,
        bin_freq,
        DEFAULT_CORRECTION_A_FREQ,
        DEFAULT_CORRECTION_B_FREQ,
    );

    FixEqAnalysis {
        mud,
        correction_a,
        correction_b,
    }
}

/// Smoothing window size for spectral deviation method (in bins)
const SMOOTHING_WINDOW: usize = 15;

/// Compute smoothed spectrum using moving average
fn compute_smoothed_spectrum(power: &[f32], window_size: usize) -> Vec<f32> {
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

/// Find the most prominent peak in a range using spectral deviation.
/// Compares actual spectrum to smoothed spectrum to find frequencies that "stick out".
fn find_peak_by_deviation(
    avg_power: &[f32],
    smoothed: &[f32],
    min_bin: usize,
    max_bin: usize,
    bin_freq: f32,
    default_freq: f32,
) -> BandAnalysis {
    if min_bin >= max_bin || max_bin >= avg_power.len() {
        return BandAnalysis {
            center_freq: default_freq,
            energy_db: -60.0,
            confidence: 0.0,
        };
    }

    // Compute deviation ratio in range and find peak
    let mut peak_bin = min_bin;
    let mut peak_deviation = 0.0f32;

    for bin in min_bin..=max_bin {
        let deviation = if smoothed[bin] > 1e-12 {
            avg_power[bin] / smoothed[bin]
        } else {
            1.0
        };

        if deviation > peak_deviation {
            peak_deviation = deviation;
            peak_bin = bin;
        }
    }

    let power = avg_power[peak_bin];

    BandAnalysis {
        center_freq: peak_bin as f32 * bin_freq,
        energy_db: 10.0 * power.max(1e-12).log10(),
        // Confidence: deviation of 1.5 = 50%, 2.0 = 100%
        confidence: ((peak_deviation - 1.0) / 1.0).clamp(0.0, 1.0),
    }
}

/// Find top 2 resonances using spectral deviation method.
/// Compares actual spectrum to smoothed spectrum to find peaks that "stick out".
/// Returns bands sorted by frequency (lower first).
fn find_two_peaks_by_deviation(
    avg_power: &[f32],
    smoothed: &[f32],
    min_bin: usize,
    max_bin: usize,
    bin_freq: f32,
    default_freq_a: f32,
    default_freq_b: f32,
) -> (BandAnalysis, BandAnalysis) {
    let default_a = BandAnalysis {
        center_freq: default_freq_a,
        energy_db: -60.0,
        confidence: 0.0,
    };
    let default_b = BandAnalysis {
        center_freq: default_freq_b,
        energy_db: -60.0,
        confidence: 0.0,
    };

    if min_bin >= max_bin || max_bin >= avg_power.len() {
        return (default_a, default_b);
    }

    // Compute deviation ratio (actual / smoothed)
    // Values > 1 indicate peaks that stick out above the local average
    let mut deviation = vec![0.0f32; avg_power.len()];
    for bin in min_bin..=max_bin {
        if smoothed[bin] > 1e-12 {
            deviation[bin] = avg_power[bin] / smoothed[bin];
        } else {
            deviation[bin] = 1.0;
        }
    }

    // Find top 2 peaks in the deviation spectrum
    let mut peak1_bin = min_bin;
    let mut peak1_deviation = deviation[min_bin];

    for bin in min_bin..=max_bin {
        if deviation[bin] > peak1_deviation {
            peak1_deviation = deviation[bin];
            peak1_bin = bin;
        }
    }

    // Find second peak (highest deviation outside exclusion zone)
    let mut peak2_bin = min_bin;
    let mut peak2_deviation = 0.0f32;

    for bin in min_bin..=max_bin {
        let distance = if bin > peak1_bin {
            bin - peak1_bin
        } else {
            peak1_bin - bin
        };

        if distance >= CORRECTION_MIN_SPACING_BINS && deviation[bin] > peak2_deviation {
            peak2_deviation = deviation[bin];
            peak2_bin = bin;
        }
    }

    // Build results - confidence is based on how much the peak deviates (ratio > 1)
    // A deviation of 2.0 means the peak is 2x the local average
    let build_result = |bin: usize, dev: f32| -> BandAnalysis {
        BandAnalysis {
            center_freq: bin as f32 * bin_freq,
            energy_db: 10.0 * avg_power[bin].max(1e-12).log10(),
            confidence: ((dev - 1.0) / 1.0).clamp(0.0, 1.0),
        }
    };

    let result1 = build_result(peak1_bin, peak1_deviation);

    // If no valid second peak found, use default
    let result2 = if peak2_deviation > 1.0 {
        build_result(peak2_bin, peak2_deviation)
    } else {
        // Place second peak at opposite end of range from first
        let range_mid = (CORRECTION_FREQ_MIN + CORRECTION_FREQ_MAX) / 2.0;
        let second_default = if result1.center_freq < range_mid {
            default_freq_b
        } else {
            default_freq_a
        };
        BandAnalysis {
            center_freq: second_default,
            energy_db: -60.0,
            confidence: 0.0,
        }
    };

    // Return sorted by frequency (lower first)
    if result1.center_freq < result2.center_freq {
        (result1, result2)
    } else {
        (result2, result1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_empty() {
        let result = analyze_audio(&[], 48000);
        assert_eq!(result.mud.center_freq, 300.0);
        assert_eq!(result.mud.confidence, 0.0);
    }

    #[test]
    fn test_mix_to_mono() {
        let left = vec![1.0, 0.5, 0.0];
        let right = vec![0.0, 0.5, 1.0];
        let mono = mix_to_mono(&left, &right);
        assert_eq!(mono, vec![0.5, 0.5, 0.5]);
    }
}
