//! Sibilance analysis for De-Esser
//!
//! Analyzes audio to find sibilance characteristics for dynamic processing.
//! Uses cepstral analysis to distinguish true sibilance from harmonic overtones.

use crate::analysis::{CepstralAnalysis, SpectralAnalysis};
use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

const WINDOW_SIZE: usize = 4096;
const HOP_SIZE: usize = 2048;

/// Sibilance frequency range (Hz)
const SIBILANCE_FREQ_MIN: f32 = 4000.0;
const SIBILANCE_FREQ_MAX: f32 = 12000.0; // Extended from 10kHz to catch higher sibilance

/// Default sibilance frequency (Hz) - middle of typical range
pub const DEFAULT_SIBILANCE_FREQ: f32 = 6500.0;

/// Result of sibilance analysis
#[derive(Clone, Debug)]
pub struct SibilanceAnalysis {
    /// Detected center frequency (Hz)
    pub center_freq: f32,
    /// Bandwidth at -6dB points (Hz)
    pub bandwidth_hz: f32,
    /// Energy level at the detected frequency (dB)
    pub energy_db: f32,
    /// Confidence score (0.0 = uncertain, 1.0 = very confident)
    /// Based on ratio of sibilance energy to rest of spectrum
    pub confidence: f32,
}

impl Default for SibilanceAnalysis {
    fn default() -> Self {
        Self {
            center_freq: DEFAULT_SIBILANCE_FREQ,
            bandwidth_hz: 3000.0,
            energy_db: -60.0,
            confidence: 0.0,
        }
    }
}


/// Analyze audio for sibilance characteristics
/// Computes f0 via cepstral analysis to distinguish sibilance from harmonic overtones
pub fn analyze_sibilance(audio: &[f32], sample_rate: u32) -> SibilanceAnalysis {
    if audio.is_empty() {
        return SibilanceAnalysis::default();
    }

    let n_bins = WINDOW_SIZE / 2 + 1;
    let bin_freq = sample_rate as f32 / WINDOW_SIZE as f32;

    // Calculate bin ranges for sibilance
    let sib_min_bin = (SIBILANCE_FREQ_MIN / bin_freq).ceil() as usize;
    let sib_max_bin = (SIBILANCE_FREQ_MAX / bin_freq).floor() as usize;
    let sib_max_bin = sib_max_bin.min(n_bins - 1);

    if sib_min_bin >= sib_max_bin {
        return SibilanceAnalysis::default();
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
        return SibilanceAnalysis::default();
    }

    for p in &mut avg_power {
        *p /= frame_count as f32;
    }

    // Find peak energy in sibilance range
    let mut peak_bin = sib_min_bin;
    let mut peak_power = avg_power[sib_min_bin];

    for bin in sib_min_bin..=sib_max_bin {
        if avg_power[bin] > peak_power {
            peak_power = avg_power[bin];
            peak_bin = bin;
        }
    }

    // Calculate average power in sibilance band
    let sib_count = (sib_max_bin - sib_min_bin + 1) as f32;
    let avg_sib_power: f32 = avg_power[sib_min_bin..=sib_max_bin].iter().sum::<f32>() / sib_count;

    // Dual-mode detection:
    // If peak is >3x average, use peak (sharp sibilance)
    // Otherwise, use spectral centroid (broadband sibilance)
    let peak_ratio = peak_power / avg_sib_power.max(1e-12);
    let is_peaked = peak_ratio > 3.0;

    let center_freq = if is_peaked {
        // Sharp sibilance: use peak frequency
        peak_bin as f32 * bin_freq
    } else {
        // Broadband sibilance: use spectral centroid (center of mass)
        let mut weighted_sum = 0.0f32;
        let mut power_sum = 0.0f32;
        for bin in sib_min_bin..=sib_max_bin {
            let freq = bin as f32 * bin_freq;
            weighted_sum += freq * avg_power[bin];
            power_sum += avg_power[bin];
        }
        if power_sum > 0.0 {
            weighted_sum / power_sum
        } else {
            DEFAULT_SIBILANCE_FREQ
        }
    };

    let energy_db = 10.0 * peak_power.max(1e-12).log10();

    // Compute cepstral analysis for envelope and f0
    let spectrum = SpectralAnalysis::new(audio, sample_rate);
    let cepstral = CepstralAnalysis::from_spectrum(&spectrum);
    let f0 = cepstral.f0;

    // Compute residue: original spectrum - cepstral envelope
    // Positive residue = energy above smooth envelope
    // Key insight from research:
    // - Sibilance: noise-like, residue spread across MANY bins (broadband)
    // - Harmonics: residue concentrated at FEW bins (peaked at harmonic frequencies)
    let total_bins = sib_max_bin - sib_min_bin + 1;
    let mut residue_values = Vec::with_capacity(total_bins);
    let mut positive_residue_count = 0usize;
    let mut residue_sum = 0.0f32;

    // Track extent of positive residue for bandwidth
    let mut first_positive_bin: Option<usize> = None;
    let mut last_positive_bin: Option<usize> = None;

    for (idx, bin) in (sib_min_bin..=sib_max_bin).enumerate() {
        if bin < cepstral.envelope_db.len() {
            let original_db = 10.0 * avg_power[bin].max(1e-12).log10();
            let envelope_db = cepstral.envelope_db[bin];
            let residue = original_db - envelope_db;
            residue_values.push(residue);
            if residue > 0.0 {
                residue_sum += residue;
                positive_residue_count += 1;
                if first_positive_bin.is_none() {
                    first_positive_bin = Some(idx);
                }
                last_positive_bin = Some(idx);
            }
        }
    }

    // Bandwidth from residue extent: where is sibilance energy above envelope?
    let bandwidth_hz = if let (Some(first), Some(last)) = (first_positive_bin, last_positive_bin) {
        let first_freq = (sib_min_bin + first) as f32 * bin_freq;
        let last_freq = (sib_min_bin + last) as f32 * bin_freq;
        (last_freq - first_freq).max(500.0)
    } else {
        // Fallback: use portion of sibilance range
        (SIBILANCE_FREQ_MAX - SIBILANCE_FREQ_MIN) * 0.5
    };

    // Spread factor: what fraction of bins have positive residue?
    // Sibilance (noise-like) → spread across many bins → high spread factor
    // Harmonics → concentrated at few bins → low spread factor
    let spread_factor = if !residue_values.is_empty() {
        positive_residue_count as f32 / residue_values.len() as f32
    } else {
        0.0
    };

    // Average positive residue in sibilance band (dB above envelope)
    let avg_residue = if positive_residue_count > 0 {
        residue_sum / positive_residue_count as f32
    } else {
        0.0
    };

    // Combined residue confidence:
    // - High avg_residue = energy above envelope (could be sibilance or harmonics)
    // - High spread_factor = energy spread broadly (sibilance indicator)
    // Require BOTH: significant residue AND broad spread
    let residue_magnitude = (avg_residue / 6.0).clamp(0.0, 1.0); // 6dB above = 100%
    let spread_confidence = (spread_factor / 0.3).clamp(0.0, 1.0); // 30% of bins = 100%
    let residue_confidence = residue_magnitude * spread_confidence;

    // Also use band energy ratio as secondary check
    let band_confidence = compute_sibilance_confidence(
        &avg_power,
        sib_min_bin,
        sib_max_bin,
        center_freq,
        f0,
    );

    // Combine: use max of residue and band confidence
    let confidence = residue_confidence.max(band_confidence);

    SibilanceAnalysis {
        center_freq,
        bandwidth_hz,
        energy_db,
        confidence,
    }
}

/// Compute sibilance confidence using total band energy ratio
/// Also reduces confidence if sibilance frequency aligns with harmonics of f0
fn compute_sibilance_confidence(
    power: &[f32],
    min_bin: usize,
    max_bin: usize,
    center_freq: f32,
    f0: Option<f32>,
) -> f32 {
    // Total energy in sibilance band (not just peak - sibilance is broadband)
    let sib_energy: f32 = power[min_bin..=max_bin].iter().sum();

    // Total energy outside sibilance band
    let outside_energy: f32 = power
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < min_bin || *i > max_bin)
        .map(|(_, &p)| p)
        .sum();

    // Ratio: what fraction of total spectrum is in sibilance band?
    let total = sib_energy + outside_energy;
    let sib_ratio = if total > 0.0 {
        sib_energy / total
    } else {
        0.0
    };

    // Map: 20% of spectrum in sibilance band = confidence 1.0
    // This handles broadband sibilance better than peak-only
    let base_confidence = (sib_ratio / 0.2).clamp(0.0, 1.0);

    // If f0 is detected, check if sibilance frequency aligns with harmonics
    // True sibilance is noise-like and won't align with harmonics
    let harmonic_factor = if let Some(f0) = f0 {
        let harmonic_number = (center_freq / f0).round();
        let nearest_harmonic = harmonic_number * f0;
        let distance = (center_freq - nearest_harmonic).abs();

        // If within 1/4 of f0, likely a harmonic overtone, not sibilance
        // Less aggressive: 0.5 minimum instead of 0.2
        let tolerance = f0 * 0.25;
        (distance / tolerance).clamp(0.5, 1.0)
    } else {
        1.0 // No f0 detected, don't adjust
    };

    base_confidence * harmonic_factor
}

/// Calculate adaptive Q based on detected bandwidth
/// Filter bandwidth is 4x detected bandwidth for wide coverage
pub fn calculate_deesser_q(bandwidth_hz: f32, center_freq: f32) -> f32 {
    // Filter bandwidth = detected bandwidth * 4.0
    let filter_bandwidth = bandwidth_hz * 4.0;
    // Q = center_freq / bandwidth
    let q = center_freq / filter_bandwidth;
    q.clamp(0.7, 2.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::utils::mix_to_mono;

    #[test]
    fn test_analyze_empty() {
        let result = analyze_sibilance(&[], 48000);
        assert_eq!(result.center_freq, DEFAULT_SIBILANCE_FREQ);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn test_mix_to_mono() {
        let left = vec![1.0, 0.5, 0.0];
        let right = vec![0.0, 0.5, 1.0];
        let mono = mix_to_mono(&left, &right);
        assert_eq!(mono, vec![0.5, 0.5, 0.5]);
    }

    #[test]
    fn test_deesser_q_calculation() {
        // Typical sibilance: 6500Hz center, 3000Hz detected bandwidth
        // Filter bandwidth = 3000 * 4.0 = 12000Hz
        // Q = 6500 / 12000 ≈ 0.54 → clamped to 0.7
        let q = calculate_deesser_q(3000.0, 6500.0);
        assert_eq!(q, 0.7);

        // Very wide bandwidth should clamp to minimum Q
        let q_wide = calculate_deesser_q(10000.0, 6500.0);
        assert_eq!(q_wide, 0.7);

        // Narrow bandwidth: 500Hz detected → 2000Hz filter → Q = 6500/2000 = 3.25 → clamped to 2.5
        let q_narrow = calculate_deesser_q(500.0, 6500.0);
        assert_eq!(q_narrow, 2.5);
    }
}
