//! AutoGain - Automatic input level normalization
//!
//! Analyzes audio RMS and applies gain to normalize to a target level.
//! This ensures the processing chain operates at consistent levels.

/// Default target RMS level in dBFS
pub const DEFAULT_TARGET_RMS_DB: f32 = -18.0;

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

/// Convert linear RMS to dBFS
pub fn rms_to_db(rms: f32) -> f32 {
    20.0 * rms.max(1e-10).log10()
}

/// Convert dB to linear gain
pub fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

/// Analyze audio and calculate required gain to reach target RMS
///
/// Returns gain in dB (positive = boost, negative = cut)
pub fn analyze_gain(samples: &[Vec<f32>], target_rms_db: f32) -> f32 {
    let rms = if samples.len() >= 2 {
        // Stereo: sum to mono first
        calculate_rms_stereo(&samples[0], &samples[1])
    } else if !samples.is_empty() {
        // Mono
        calculate_rms(&samples[0])
    } else {
        return 0.0;
    };

    let rms_db = rms_to_db(rms);
    target_rms_db - rms_db
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
        let db = rms_to_db(0.1);
        assert!((db - (-20.0)).abs() < 0.01);

        // Round trip
        let linear = db_to_linear(-20.0);
        assert!((linear - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_analyze_gain() {
        // Audio at -30 dBFS should need +12dB to reach -18 dBFS
        let rms_linear = db_to_linear(-30.0);
        let samples = vec![vec![rms_linear; 1000]]; // Constant value = RMS
        let gain = analyze_gain(&samples, -18.0);
        assert!((gain - 12.0).abs() < 0.1);
    }
}
