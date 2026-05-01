//! AutoGain - Automatic input level normalization
//!
//! Analyzes audio RMS and applies gain to normalize to a target level.
//! This ensures the processing chain operates at consistent levels.

#![allow(dead_code)]

/// Default target RMS level in dBFS
pub const DEFAULT_TARGET_RMS_DB: f32 = -18.0;

/// Default target peak level in dBFS (ceiling to prevent clipping)
pub const DEFAULT_TARGET_PEAK_DB: f32 = -3.0;

const ACTIVITY_WINDOW_MS: f32 = 50.0;
const ACTIVITY_GATE_FLOOR_DB: f32 = -65.0;
const ACTIVITY_GATE_BELOW_P95_DB: f32 = 35.0;
const MAX_INPUT_BOOST_DB: f32 = 12.0;
const MAX_INPUT_CUT_DB: f32 = -18.0;

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

fn mono_sample(samples: &[Vec<f32>], index: usize) -> Option<f32> {
    match samples {
        [] => None,
        [mono] => mono.get(index).copied(),
        [left, right, ..] => left
            .get(index)
            .zip(right.get(index))
            .map(|(&l, &r)| (l + r) * 0.5),
    }
}

fn full_file_peak(samples: &[Vec<f32>]) -> f32 {
    samples
        .iter()
        .flat_map(|channel| channel.iter())
        .map(|s| s.abs())
        .fold(0.0_f32, f32::max)
}

fn percentile(sorted: &[f32], percentile: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }

    let idx = ((sorted.len() - 1) as f32 * percentile).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn active_rms(samples: &[Vec<f32>], sample_rate: u32) -> Option<f32> {
    let num_samples = samples.iter().map(Vec::len).min()?;
    if num_samples == 0 {
        return None;
    }

    let window_samples = ((sample_rate as f32 * ACTIVITY_WINDOW_MS) / 1000.0)
        .round()
        .max(1.0) as usize;
    let mut block_rms_db = Vec::new();

    let mut start = 0;
    while start < num_samples {
        let end = (start + window_samples).min(num_samples);
        let mut sum_sq = 0.0_f32;
        let mut count = 0usize;

        for i in start..end {
            if let Some(sample) = mono_sample(samples, i) {
                sum_sq += sample * sample;
                count += 1;
            }
        }

        if count > 0 {
            let rms = (sum_sq / count as f32).sqrt();
            block_rms_db.push(linear_to_db(rms));
        }

        start = end;
    }

    if block_rms_db.is_empty() {
        return None;
    }

    block_rms_db.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_db = percentile(&block_rms_db, 0.95);
    let activity_gate_db = ACTIVITY_GATE_FLOOR_DB.max(p95_db - ACTIVITY_GATE_BELOW_P95_DB);

    let activity_gate = db_to_linear(activity_gate_db);
    let mut active_sum_sq = 0.0_f32;
    let mut active_count = 0usize;

    let mut start = 0;
    while start < num_samples {
        let end = (start + window_samples).min(num_samples);
        let mut block_sum_sq = 0.0_f32;
        let mut block_count = 0usize;

        for i in start..end {
            if let Some(sample) = mono_sample(samples, i) {
                block_sum_sq += sample * sample;
                block_count += 1;
            }
        }

        if block_count > 0 {
            let block_rms = (block_sum_sq / block_count as f32).sqrt();
            if block_rms >= activity_gate {
                active_sum_sq += block_sum_sq;
                active_count += block_count;
            }
        }

        start = end;
    }

    if active_count == 0 {
        return None;
    }

    Some((active_sum_sq / active_count as f32).sqrt())
}

/// Analyze audio and calculate required gain to reach target RMS,
/// while ensuring peak doesn't exceed target_peak_db.
///
/// Returns gain in dB (positive = boost, negative = cut)
pub fn analyze_gain(
    samples: &[Vec<f32>],
    sample_rate: u32,
    target_rms_db: f32,
    target_peak_db: f32,
) -> f32 {
    let Some(rms) = active_rms(samples, sample_rate) else {
        return 0.0;
    };
    let peak = full_file_peak(samples);

    let rms_db = linear_to_db(rms);
    let peak_db = linear_to_db(peak); // linear_to_db works for any linear->dB conversion

    let gain_for_rms = target_rms_db - rms_db;
    let gain_for_peak = target_peak_db - peak_db;

    let gain = if gain_for_rms > 0.0 {
        // Boosting: limit gain so peak doesn't exceed ceiling
        gain_for_rms.min(gain_for_peak)
    } else {
        // Cutting: no clipping concern, just use RMS-based gain
        gain_for_rms
    };

    gain.clamp(MAX_INPUT_CUT_DB, MAX_INPUT_BOOST_DB)
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
        let gain = analyze_gain(&samples, 48000, -18.0, -1.0);
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

        let gain = analyze_gain(&vec![samples], 48000, -18.0, -1.0);
        // Gain should be limited to +5dB (from -6 to -1 peak)
        assert!((gain - 5.0).abs() < 0.2);
    }

    #[test]
    fn test_analyze_gain_ignores_long_silence() {
        let sample_rate = 48000;
        let speech: Vec<f32> = (0..sample_rate)
            .map(|i| {
                db_to_linear(-30.0)
                    * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin()
            })
            .collect();

        let mut padded = vec![0.0; sample_rate as usize * 4];
        padded.extend_from_slice(&speech);
        padded.extend(vec![0.0; sample_rate as usize * 4]);

        let speech_gain = analyze_gain(&vec![speech], sample_rate, -18.0, -1.0);
        let padded_gain = analyze_gain(&vec![padded], sample_rate, -18.0, -1.0);

        assert!(
            (speech_gain - padded_gain).abs() < 0.5,
            "active gain should not depend on silence duration: speech={speech_gain}, padded={padded_gain}"
        );
    }

    #[test]
    fn test_analyze_gain_does_not_boost_silence() {
        let samples = vec![vec![0.0; 48000]];
        let gain = analyze_gain(&samples, 48000, -18.0, -1.0);
        assert_eq!(gain, 0.0);
    }

    #[test]
    fn test_default_peak_ceiling_is_below_full_scale() {
        assert!(DEFAULT_TARGET_PEAK_DB < 0.0);
    }
}
