//! AutoGain - Automatic input level normalization
//!
//! Analyzes audio RMS and applies gain to normalize to a target level.
//! This ensures the processing chain operates at consistent levels.

#![allow(dead_code)]

/// Default target RMS level in dBFS
pub const DEFAULT_TARGET_RMS_DB: f32 = -18.0;

/// Default target peak level in dBFS (ceiling to prevent clipping)
pub const DEFAULT_TARGET_PEAK_DB: f32 = 10.0;

/// Calculate RMS of mono audio samples
pub fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Calculate RMS of stereo audio by summing to mono first
pub fn calculate_rms_stereo(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = left
        .iter()
        .zip(right.iter())
        .map(|(l, r)| {
            let mono = (l + r) * 0.5;
            mono * mono
        })
        .sum();
    (sum_sq / left.len() as f32).sqrt()
}

/// Calculate peak (max absolute value) of mono audio samples
pub fn calculate_peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max)
}

/// Calculate peak of stereo audio (max across both channels)
pub fn calculate_peak_stereo(left: &[f32], right: &[f32]) -> f32 {
    let peak_left = calculate_peak(left);
    let peak_right = calculate_peak(right);
    peak_left.max(peak_right)
}

/// Calculate both RMS and peak in a single pass (mono)
pub fn calculate_rms_and_peak(samples: &[f32]) -> (f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum_sq = 0.0_f32;
    let mut peak = 0.0_f32;
    for &s in samples {
        sum_sq += s * s;
        peak = peak.max(s.abs());
    }
    ((sum_sq / samples.len() as f32).sqrt(), peak)
}

/// Calculate both RMS and peak in a single pass (stereo)
pub fn calculate_rms_and_peak_stereo(left: &[f32], right: &[f32]) -> (f32, f32) {
    if left.is_empty() || right.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum_sq = 0.0_f32;
    let mut peak = 0.0_f32;
    for (&l, &r) in left.iter().zip(right.iter()) {
        let mono = (l + r) * 0.5;
        sum_sq += mono * mono;
        peak = peak.max(l.abs()).max(r.abs());
    }
    ((sum_sq / left.len() as f32).sqrt(), peak)
}

// Re-export from shared utilities
pub use crate::analysis::utils::{db_to_linear, linear_to_db};

/// Analyze audio and calculate required gain to reach target RMS,
/// while ensuring peak doesn't exceed target_peak_db.
///
/// Returns gain in dB (positive = boost, negative = cut)
pub fn analyze_gain(samples: &[Vec<f32>], target_rms_db: f32, target_peak_db: f32) -> f32 {
    let (rms, peak) = if samples.len() >= 2 {
        // Stereo: RMS from mono sum, peak from either channel
        calculate_rms_and_peak_stereo(&samples[0], &samples[1])
    } else if !samples.is_empty() {
        // Mono
        calculate_rms_and_peak(&samples[0])
    } else {
        return 0.0;
    };

    let rms_db = linear_to_db(rms);
    let peak_db = linear_to_db(peak); // linear_to_db works for any linear->dB conversion

    let gain_for_rms = target_rms_db - rms_db;
    let gain_for_peak = target_peak_db - peak_db;

    if gain_for_rms > 0.0 {
        // Boosting: limit gain so peak doesn't exceed ceiling
        gain_for_rms.min(gain_for_peak)
    } else {
        // Cutting: no clipping concern, just use RMS-based gain
        gain_for_rms
    }
}

/// Apply gain to all channels in-place
pub fn apply_gain(samples: &mut [Vec<f32>], gain_db: f32) {
    let gain_linear = db_to_linear(gain_db);
    for channel in samples.iter_mut() {
        for sample in channel.iter_mut() {
            *sample *= gain_linear;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rms_silence() {
        let samples = vec![0.0; 1000];
        let rms = calculate_rms(&samples);
        assert!(rms < 1e-9);
    }

    #[test]
    fn test_rms_sine() {
        // RMS of a sine wave is amplitude / sqrt(2)
        let amplitude = 0.5;
        let samples: Vec<f32> = (0..48000)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 48000.0).sin())
            .collect();
        let rms = calculate_rms(&samples);
        let expected = amplitude / 2.0_f32.sqrt();
        assert!((rms - expected).abs() < 0.01);
    }

    #[test]
    fn test_db_conversion() {
        // -20 dBFS = 0.1 linear
        let db = linear_to_db(0.1);
        assert!((db - (-20.0)).abs() < 0.01);

        // Round trip
        let linear = db_to_linear(-20.0);
        assert!((linear - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_analyze_gain() {
        // Audio at -30 dBFS RMS should need +12dB to reach -18 dBFS
        // With constant signal, RMS = peak, so peak would be -30 dBFS
        // Boosting by +12dB gives peak at -18 dBFS, well below -1 dBFS ceiling
        let rms_linear = db_to_linear(-30.0);
        let samples = vec![vec![rms_linear; 1000]]; // Constant value = RMS = peak
        let gain = analyze_gain(&samples, -18.0, -1.0);
        assert!((gain - 12.0).abs() < 0.1);
    }

    #[test]
    fn test_analyze_gain_peak_limited() {
        // Audio at -30 dBFS RMS but with a peak at -6 dBFS
        // Target RMS is -18 dBFS (+12dB boost)
        // But that would put peak at +6 dBFS (clipping!)
        // Peak ceiling is -1 dBFS, so max boost is +5dB
        let rms_linear = db_to_linear(-30.0);
        let peak_linear = db_to_linear(-6.0);

        // Create samples with low RMS but high peak
        let mut samples = vec![rms_linear; 1000];
        samples[500] = peak_linear; // Single peak

        let gain = analyze_gain(&vec![samples], -18.0, -1.0);
        // Gain should be limited to +5dB (from -6 to -1 peak)
        assert!((gain - 5.0).abs() < 0.2);
    }
}
