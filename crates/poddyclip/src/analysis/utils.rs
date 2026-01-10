//! Shared audio analysis utilities
//!
//! Common functions used across analysis and processor modules.

/// Convert linear amplitude to decibels
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    20.0 * linear.max(1e-10).log10()
}

/// Convert decibels to linear amplitude
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

/// Mix stereo to mono for analysis (average of L+R)
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
    fn test_db_conversions() {
        // 0 dB = 1.0 linear
        assert!((db_to_linear(0.0) - 1.0).abs() < 0.001);
        // -6 dB ≈ 0.5 linear
        assert!((db_to_linear(-6.0) - 0.5).abs() < 0.02);
        // Round trip
        let original = 0.7;
        let db = linear_to_db(original);
        let back = db_to_linear(db);
        assert!((original - back).abs() < 0.001);
    }

    #[test]
    fn test_mix_to_mono() {
        let left = vec![1.0, 0.5, 0.0];
        let right = vec![0.0, 0.5, 1.0];
        let mono = mix_to_mono(&left, &right);
        assert_eq!(mono, vec![0.5, 0.5, 0.5]);
    }
}
