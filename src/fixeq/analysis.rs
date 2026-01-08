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

/// Sibilance frequency range (Hz) - narrower range focused on actual "ss" sounds
const SIBILANCE_FREQ_MIN: f32 = 5500.0;
const SIBILANCE_FREQ_MAX: f32 = 9000.0;

/// Correction frequency range (Hz) - mid-range for resonances/harshness
const CORRECTION_FREQ_MIN: f32 = 500.0;
const CORRECTION_FREQ_MAX: f32 = 5000.0;

/// Minimum spacing between correction peaks (in bins) to avoid picking adjacent frequencies
const CORRECTION_MIN_SPACING_BINS: usize = 20; // ~200Hz at 48kHz

/// Result of mud frequency analysis
#[derive(Clone, Debug)]
pub struct MudAnalysis {
    /// Detected center frequency for de-mud (Hz)
    pub center_freq: f32,
    /// Energy level at the detected frequency (dB)
    pub energy_db: f32,
    /// Confidence score (0.0 = uncertain, 1.0 = very confident)
    pub confidence: f32,
}

/// Result of sibilance frequency analysis
#[derive(Clone, Debug)]
pub struct SibilanceAnalysis {
    /// Detected center frequency for de-esser (Hz)
    pub center_freq: f32,
    /// Energy level at the detected frequency (dB)
    pub energy_db: f32,
    /// Confidence score (0.0 = uncertain, 1.0 = very confident)
    pub confidence: f32,
}

/// Result of single correction band analysis
#[derive(Clone, Debug)]
pub struct CorrectionBandAnalysis {
    /// Detected center frequency (Hz)
    pub center_freq: f32,
    /// Energy level at the detected frequency (dB)
    pub energy_db: f32,
    /// Confidence score (0.0 = uncertain, 1.0 = very confident)
    pub confidence: f32,
}

/// Result of correction analysis (two bands)
#[derive(Clone, Debug)]
pub struct CorrectionAnalysis {
    pub band_a: CorrectionBandAnalysis,
    pub band_b: CorrectionBandAnalysis,
}

/// Combined analysis for all FixEq bands
#[derive(Clone, Debug)]
pub struct FixEqAnalysis {
    pub mud: MudAnalysis,
    pub sibilance: SibilanceAnalysis,
    pub correction: CorrectionAnalysis,
}

/// Analyze audio to find the dominant mud frequency.
///
/// Takes mixed L+R (mono) audio after denoising.
/// Returns the optimal center frequency for de-mud processing.
pub fn analyze_mud_frequency(audio: &[f32], sample_rate: u32) -> MudAnalysis {
    if audio.is_empty() {
        return MudAnalysis {
            center_freq: 300.0, // Default fallback
            energy_db: -60.0,
            confidence: 0.0,
        };
    }

    let n_bins = WINDOW_SIZE / 2 + 1;
    let bin_freq = sample_rate as f32 / WINDOW_SIZE as f32;

    // Calculate bin range for mud frequencies
    let min_bin = (MUD_FREQ_MIN / bin_freq).ceil() as usize;
    let max_bin = (MUD_FREQ_MAX / bin_freq).floor() as usize;
    let max_bin = max_bin.min(n_bins - 1);

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

        // Apply window
        let windowed: Vec<f32> = frame
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        // FFT
        let mut spectrum: Vec<Complex<f32>> = windowed
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();
        fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

        // Accumulate power
        for (j, c) in spectrum[..n_bins].iter().enumerate() {
            avg_power[j] += c.norm_sqr();
        }
        frame_count += 1;
        i += HOP_SIZE;
    }

    if frame_count == 0 {
        return MudAnalysis {
            center_freq: 300.0,
            energy_db: -60.0,
            confidence: 0.0,
        };
    }

    // Average the power spectrum
    for p in &mut avg_power {
        *p /= frame_count as f32;
    }

    // Find peak in mud range
    let mut peak_bin = min_bin;
    let mut peak_power = avg_power[min_bin];

    for bin in min_bin..=max_bin {
        if avg_power[bin] > peak_power {
            peak_power = avg_power[bin];
            peak_bin = bin;
        }
    }

    // Calculate energy in dB
    let energy_db = 10.0 * (peak_power.max(1e-12)).log10();

    // Calculate average energy outside mud range for comparison
    let mut outside_energy = 0.0f32;
    let mut outside_count = 0usize;

    // Below mud range (50-150Hz)
    let below_min = (50.0 / bin_freq).ceil() as usize;
    for bin in below_min..min_bin {
        outside_energy += avg_power[bin];
        outside_count += 1;
    }

    // Above mud range (500-2000Hz)
    let above_max = (2000.0 / bin_freq).floor() as usize;
    for bin in (max_bin + 1)..=above_max.min(n_bins - 1) {
        outside_energy += avg_power[bin];
        outside_count += 1;
    }

    let avg_outside = if outside_count > 0 {
        outside_energy / outside_count as f32
    } else {
        peak_power
    };

    // Confidence: how much does mud range stand out?
    let ratio = peak_power / avg_outside.max(1e-12);
    let confidence = (ratio.log10() / 1.0).clamp(0.0, 1.0); // 10x = full confidence

    // Convert bin to frequency
    let center_freq = peak_bin as f32 * bin_freq;

    MudAnalysis {
        center_freq,
        energy_db,
        confidence,
    }
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
    if audio.is_empty() {
        return FixEqAnalysis {
            mud: MudAnalysis {
                center_freq: 300.0,
                energy_db: -60.0,
                confidence: 0.0,
            },
            sibilance: SibilanceAnalysis {
                center_freq: 6500.0,
                energy_db: -60.0,
                confidence: 0.0,
            },
            correction: CorrectionAnalysis {
                band_a: CorrectionBandAnalysis {
                    center_freq: DEFAULT_CORRECTION_A_FREQ,
                    energy_db: -60.0,
                    confidence: 0.0,
                },
                band_b: CorrectionBandAnalysis {
                    center_freq: DEFAULT_CORRECTION_B_FREQ,
                    energy_db: -60.0,
                    confidence: 0.0,
                },
            },
        };
    }

    let n_bins = WINDOW_SIZE / 2 + 1;
    let bin_freq = sample_rate as f32 / WINDOW_SIZE as f32;

    // Calculate bin ranges
    let mud_min_bin = (MUD_FREQ_MIN / bin_freq).ceil() as usize;
    let mud_max_bin = (MUD_FREQ_MAX / bin_freq).floor() as usize;
    let mud_max_bin = mud_max_bin.min(n_bins - 1);

    let sib_min_bin = (SIBILANCE_FREQ_MIN / bin_freq).ceil() as usize;
    let sib_max_bin = (SIBILANCE_FREQ_MAX / bin_freq).floor() as usize;
    let sib_max_bin = sib_max_bin.min(n_bins - 1);

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
        return FixEqAnalysis {
            mud: MudAnalysis {
                center_freq: 300.0,
                energy_db: -60.0,
                confidence: 0.0,
            },
            sibilance: SibilanceAnalysis {
                center_freq: 6500.0,
                energy_db: -60.0,
                confidence: 0.0,
            },
            correction: CorrectionAnalysis {
                band_a: CorrectionBandAnalysis {
                    center_freq: DEFAULT_CORRECTION_A_FREQ,
                    energy_db: -60.0,
                    confidence: 0.0,
                },
                band_b: CorrectionBandAnalysis {
                    center_freq: DEFAULT_CORRECTION_B_FREQ,
                    energy_db: -60.0,
                    confidence: 0.0,
                },
            },
        };
    }

    for p in &mut avg_power {
        *p /= frame_count as f32;
    }

    // Analyze mud range
    let mud = analyze_range(&avg_power, mud_min_bin, mud_max_bin, bin_freq, 300.0);

    // Analyze sibilance range
    let sibilance = analyze_range(&avg_power, sib_min_bin, sib_max_bin, bin_freq, 6500.0);

    // Analyze correction range (find top 2 peaks)
    let (corr_a, corr_b) = find_top_two_peaks(
        &avg_power,
        corr_min_bin,
        corr_max_bin,
        bin_freq,
        DEFAULT_CORRECTION_A_FREQ,
        DEFAULT_CORRECTION_B_FREQ,
    );

    FixEqAnalysis {
        mud: MudAnalysis {
            center_freq: mud.0,
            energy_db: mud.1,
            confidence: mud.2,
        },
        sibilance: SibilanceAnalysis {
            center_freq: sibilance.0,
            energy_db: sibilance.1,
            confidence: sibilance.2,
        },
        correction: CorrectionAnalysis {
            band_a: corr_a,
            band_b: corr_b,
        },
    }
}

/// Helper to analyze a frequency range from power spectrum
fn analyze_range(
    avg_power: &[f32],
    min_bin: usize,
    max_bin: usize,
    bin_freq: f32,
    default_freq: f32,
) -> (f32, f32, f32) {
    if min_bin >= max_bin || max_bin >= avg_power.len() {
        return (default_freq, -60.0, 0.0);
    }

    // Find peak in range
    let mut peak_bin = min_bin;
    let mut peak_power = avg_power[min_bin];

    for bin in min_bin..=max_bin {
        if avg_power[bin] > peak_power {
            peak_power = avg_power[bin];
            peak_bin = bin;
        }
    }

    let energy_db = 10.0 * (peak_power.max(1e-12)).log10();

    // Calculate average energy outside this range for confidence
    let mut outside_energy = 0.0f32;
    let mut outside_count = 0usize;

    for (bin, &power) in avg_power.iter().enumerate() {
        if bin < min_bin || bin > max_bin {
            outside_energy += power;
            outside_count += 1;
        }
    }

    let avg_outside = if outside_count > 0 {
        outside_energy / outside_count as f32
    } else {
        peak_power
    };

    let ratio = peak_power / avg_outside.max(1e-12);
    let confidence = (ratio.log10() / 1.0).clamp(0.0, 1.0);

    let center_freq = peak_bin as f32 * bin_freq;

    (center_freq, energy_db, confidence)
}

/// Smoothing window size for spectral deviation method (in bins)
const SMOOTHING_WINDOW: usize = 15;

/// Find top 2 resonances using spectral deviation method.
/// Compares actual spectrum to smoothed spectrum to find peaks that "stick out".
/// Returns bands sorted by frequency (lower first).
fn find_top_two_peaks(
    avg_power: &[f32],
    min_bin: usize,
    max_bin: usize,
    bin_freq: f32,
    default_freq_a: f32,
    default_freq_b: f32,
) -> (CorrectionBandAnalysis, CorrectionBandAnalysis) {
    let default_a = CorrectionBandAnalysis {
        center_freq: default_freq_a,
        energy_db: -60.0,
        confidence: 0.0,
    };
    let default_b = CorrectionBandAnalysis {
        center_freq: default_freq_b,
        energy_db: -60.0,
        confidence: 0.0,
    };

    if min_bin >= max_bin || max_bin >= avg_power.len() {
        return (default_a, default_b);
    }

    // Step 1: Compute smoothed spectrum using moving average
    let mut smoothed = vec![0.0f32; avg_power.len()];
    let half_window = SMOOTHING_WINDOW / 2;

    for bin in 0..avg_power.len() {
        let start = bin.saturating_sub(half_window);
        let end = (bin + half_window + 1).min(avg_power.len());
        let sum: f32 = avg_power[start..end].iter().sum();
        let count = (end - start) as f32;
        smoothed[bin] = sum / count;
    }

    // Step 2: Compute deviation ratio (actual / smoothed)
    // Values > 1 indicate peaks that stick out above the local average
    let mut deviation = vec![0.0f32; avg_power.len()];
    for bin in min_bin..=max_bin {
        if smoothed[bin] > 1e-12 {
            deviation[bin] = avg_power[bin] / smoothed[bin];
        } else {
            deviation[bin] = 1.0;
        }
    }

    // Step 3: Find top 2 peaks in the deviation spectrum
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
    let build_result = |bin: usize, dev: f32| -> CorrectionBandAnalysis {
        let power = avg_power[bin];
        // Confidence: deviation of 1.5 = 50%, 2.0 = 100%
        let confidence = ((dev - 1.0) / 1.0).clamp(0.0, 1.0);
        CorrectionBandAnalysis {
            center_freq: bin as f32 * bin_freq,
            energy_db: 10.0 * power.max(1e-12).log10(),
            confidence,
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
        CorrectionBandAnalysis {
            center_freq: second_default,
            energy_db: -60.0,
            confidence: 0.0,
        }
    };

    // Return sorted by frequency (lower first = band_a)
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
        let result = analyze_mud_frequency(&[], 48000);
        assert_eq!(result.center_freq, 300.0);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_mix_to_mono() {
        let left = vec![1.0, 0.5, 0.0];
        let right = vec![0.0, 0.5, 1.0];
        let mono = mix_to_mono(&left, &right);
        assert_eq!(mono, vec![0.5, 0.5, 0.5]);
    }
}
