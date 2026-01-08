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

/// Combined analysis for all FixEq bands
#[derive(Clone, Debug)]
pub struct FixEqAnalysis {
    pub mud: MudAnalysis,
    pub sibilance: SibilanceAnalysis,
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
        };
    }

    for p in &mut avg_power {
        *p /= frame_count as f32;
    }

    // Analyze mud range
    let mud = analyze_range(&avg_power, mud_min_bin, mud_max_bin, bin_freq, 300.0);

    // Analyze sibilance range
    let sibilance = analyze_range(&avg_power, sib_min_bin, sib_max_bin, bin_freq, 6500.0);

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
