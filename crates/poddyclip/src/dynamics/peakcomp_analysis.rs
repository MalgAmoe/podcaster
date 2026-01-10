//! Peak profile analysis for VCA Peak Compressor
//!
//! Analyzes audio to determine peak characteristics and automatic threshold.

use crate::analysis::utils::linear_to_db;

/// Number of histogram bins for peak analysis
const HISTOGRAM_BINS: usize = 100;
const HISTOGRAM_MIN_DB: f32 = -60.0;
const HISTOGRAM_MAX_DB: f32 = 0.0;

/// Peak profile analysis result
#[derive(Clone, Debug)]
pub struct PeakProfile {
    /// RMS level in dB
    pub rms_db: f32,
    /// 95th percentile peak level in dB
    pub peak_95_db: f32,
    /// True maximum peak in dB
    pub peak_max_db: f32,
    /// Crest factor (peak_95 - rms) in dB
    pub crest_factor_db: f32,
    /// Auto-detected threshold where body ends and peaks begin
    pub histogram_threshold_db: f32,
    /// Suggested crest reduction to reach 8-12dB target
    pub suggested_reduction_db: f32,
}

impl Default for PeakProfile {
    fn default() -> Self {
        Self {
            rms_db: -60.0,
            peak_95_db: -60.0,
            peak_max_db: -60.0,
            crest_factor_db: 0.0,
            histogram_threshold_db: -12.0,
            suggested_reduction_db: 0.0,
        }
    }
}

/// Calculate RMS of samples
fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Calculate the Nth percentile of absolute sample values
fn calculate_percentile_peak(samples: &[f32], percentile: f32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    // Collect absolute values
    let mut abs_values: Vec<f32> = samples.iter().map(|s| s.abs()).collect();
    abs_values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Find percentile index
    let idx = ((abs_values.len() - 1) as f32 * percentile) as usize;
    abs_values[idx]
}

/// Calculate maximum absolute value
fn calculate_peak_max(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max)
}

/// Build histogram of sample magnitudes and find body/peak threshold
fn find_histogram_threshold(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return -12.0; // Default
    }

    let db_range = HISTOGRAM_MAX_DB - HISTOGRAM_MIN_DB;
    let db_per_bin = db_range / HISTOGRAM_BINS as f32;
    let mut bins = vec![0u32; HISTOGRAM_BINS];
    let mut total_count = 0u32;

    // Build histogram
    for &sample in samples {
        let abs_db = linear_to_db(sample.abs());
        if abs_db >= HISTOGRAM_MIN_DB && abs_db <= HISTOGRAM_MAX_DB {
            let bin_idx = ((abs_db - HISTOGRAM_MIN_DB) / db_range * (HISTOGRAM_BINS - 1) as f32) as usize;
            let bin_idx = bin_idx.min(HISTOGRAM_BINS - 1);
            bins[bin_idx] += 1;
            total_count += 1;
        }
    }

    if total_count == 0 {
        return -12.0;
    }

    // Find where cumulative > 90% AND density drops below 0.5%
    let mut cumulative = 0u32;
    let density_threshold = total_count as f32 * 0.005; // 0.5%

    for (i, &count) in bins.iter().enumerate() {
        cumulative += count;
        let cumulative_ratio = cumulative as f32 / total_count as f32;

        // When we've accumulated 90% of samples and density drops
        if cumulative_ratio > 0.90 && (count as f32) < density_threshold {
            return HISTOGRAM_MIN_DB + i as f32 * db_per_bin;
        }
    }

    // Fallback: use 95th percentile level
    let peak_95 = calculate_percentile_peak(samples, 0.95);
    linear_to_db(peak_95)
}

/// Calculate safe crest reduction to reach 8-12dB target
fn calculate_safe_reduction(crest_factor_db: f32) -> f32 {
    let target_crest = 10.0; // Middle of 8-12dB range

    if crest_factor_db <= 12.0 {
        return 0.0; // Already in range, minimal processing
    }

    // Reduce to bring crest into 8-12dB range
    (crest_factor_db - target_crest).max(0.0)
}

/// Analyze audio samples and return peak profile
pub fn analyze_peak_profile(samples: &[f32]) -> PeakProfile {
    if samples.is_empty() {
        return PeakProfile::default();
    }

    let rms = calculate_rms(samples);
    let peak_95 = calculate_percentile_peak(samples, 0.95);
    let peak_max = calculate_peak_max(samples);

    let rms_db = linear_to_db(rms);
    let peak_95_db = linear_to_db(peak_95);
    let peak_max_db = linear_to_db(peak_max);
    let crest_factor_db = peak_95_db - rms_db;

    let histogram_threshold_db = find_histogram_threshold(samples);
    let suggested_reduction_db = calculate_safe_reduction(crest_factor_db);

    PeakProfile {
        rms_db,
        peak_95_db,
        peak_max_db,
        crest_factor_db,
        histogram_threshold_db,
        suggested_reduction_db,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_empty() {
        let profile = analyze_peak_profile(&[]);
        assert_eq!(profile.rms_db, -60.0);
        assert_eq!(profile.crest_factor_db, 0.0);
    }

    #[test]
    fn test_percentile_peak() {
        let samples: Vec<f32> = (0..100).map(|i| i as f32 / 100.0).collect();
        let p95 = calculate_percentile_peak(&samples, 0.95);
        assert!((p95 - 0.95).abs() < 0.02);
    }

    #[test]
    fn test_safe_reduction() {
        // Crest of 18dB should suggest ~8dB reduction
        let reduction = calculate_safe_reduction(18.0);
        assert!((reduction - 8.0).abs() < 0.1);

        // Crest of 10dB should suggest 0dB reduction
        let reduction = calculate_safe_reduction(10.0);
        assert_eq!(reduction, 0.0);
    }

}
