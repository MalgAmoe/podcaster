//! Zone-based filter fitting for Radio Voice EQ
//!
//! Maps the error spectrum (target - actual) to biquad filter parameters
//! using a greedy zone-based algorithm.

/// Fitted filter parameters for Radio Voice EQ (hyped smiley curve)
#[derive(Clone, Debug)]
pub struct RadioFilterParams {
    /// High-pass filter cutoff frequency (Hz) - rumble protection
    pub hpf_freq: f32,
    /// Low shelf center frequency (Hz) - bass boost
    pub low_shelf_freq: f32,
    /// Low shelf gain (dB) - typically positive for boost
    pub low_shelf_gain: f32,
    /// Mud band bell center frequency (300-500 Hz)
    pub mud_freq: f32,
    /// Mud band gain (typically negative for cut)
    pub mud_gain: f32,
    /// Mud band Q factor
    pub mud_q: f32,
    /// Presence band bell center frequency (3-5 kHz)
    pub presence_freq: f32,
    /// Presence band gain (typically positive for boost)
    pub presence_gain: f32,
    /// Presence band Q factor
    pub presence_q: f32,
    /// Air high shelf gain (dB)
    pub air_gain: f32,
}

impl Default for RadioFilterParams {
    fn default() -> Self {
        Self {
            hpf_freq: 60.0,           // Lower HPF for hyped sound
            low_shelf_freq: 120.0,    // Bass boost center
            low_shelf_gain: 0.0,      // Set by fitter
            mud_freq: 400.0,          // Mud scoop center
            mud_gain: 0.0,
            mud_q: 1.0,
            presence_freq: 4000.0,    // Presence boost center
            presence_gain: 0.0,
            presence_q: 1.0,
            air_gain: 0.0,
        }
    }
}

impl RadioFilterParams {
    /// Fit filter parameters from error spectrum using zone-based greedy algorithm
    ///
    /// # Arguments
    /// * `error_db` - Error spectrum (target - actual), positive = need boost
    /// * `f0` - Detected fundamental frequency (Hz)
    /// * `bin_freq` - Hz per FFT bin
    /// * `n_bins` - Number of FFT bins
    pub fn fit_from_error(error_db: &[f32], f0: f32, bin_freq: f32, n_bins: usize) -> Self {
        let mut params = Self::default();

        // Clamp f0 to reasonable range
        let f0 = f0.clamp(75.0, 300.0);

        // 1. HPF: Set cutoff for rumble protection (lower than before for hyped sound)
        params.hpf_freq = (f0 * 0.5).max(40.0).min(80.0);

        // 2. Low shelf: Average error in bass region (80-200 Hz)
        let bass_low_bin = ((80.0 / bin_freq).round() as usize).min(n_bins - 1);
        let bass_high_bin = ((200.0 / bin_freq).round() as usize).min(n_bins - 1);

        if bass_high_bin > bass_low_bin {
            let bass_sum: f32 = error_db[bass_low_bin..=bass_high_bin].iter().sum();
            let bass_count = (bass_high_bin - bass_low_bin + 1) as f32;
            let avg_bass_error = bass_sum / bass_count;

            // For hyped sound, we want bass boost (+4 to +6 dB)
            // Add a fixed boost on top of the error-based adjustment
            params.low_shelf_gain = (avg_bass_error + 4.5).clamp(0.0, 6.0);
            params.low_shelf_freq = 120.0;
        }

        // 3. Mud band: Find max negative error (need cut) in 300-600 Hz
        let mud_low_bin = ((300.0 / bin_freq).round() as usize).min(n_bins - 1);
        let mud_high_bin = ((600.0 / bin_freq).round() as usize).min(n_bins - 1);

        let (mud_peak_bin, mud_peak_error) =
            Self::find_peak_in_range(error_db, mud_low_bin, mud_high_bin, false);

        // For hyped sound, always apply some mud cut
        params.mud_freq = (mud_peak_bin as f32 * bin_freq).clamp(300.0, 600.0);
        params.mud_gain = (mud_peak_error - 2.0).clamp(-6.0, -2.0); // Always cut at least -2 dB
        params.mud_q = Self::estimate_q_from_error(error_db, mud_peak_bin, bin_freq, 300.0, 600.0);

        // 4. Presence band: Find max positive error (need boost) in 3-5 kHz
        let presence_low_bin = ((3000.0 / bin_freq).round() as usize).min(n_bins - 1);
        let presence_high_bin = ((5000.0 / bin_freq).round() as usize).min(n_bins - 1);

        let (presence_peak_bin, presence_peak_error) =
            Self::find_peak_in_range(error_db, presence_low_bin, presence_high_bin, true);

        // For hyped sound, always boost presence
        params.presence_freq = (presence_peak_bin as f32 * bin_freq).clamp(3000.0, 5000.0);
        params.presence_gain = (presence_peak_error + 3.0).clamp(2.0, 5.0); // Always boost at least +2 dB
        params.presence_q = Self::estimate_q_from_error(error_db, presence_peak_bin, bin_freq, 3000.0, 5000.0);

        // 5. Air shelf: Average error above 8 kHz (with hyped boost)
        let air_low_bin = ((8000.0 / bin_freq).round() as usize).min(n_bins - 1);
        let air_high_bin = ((16000.0 / bin_freq).round() as usize).min(n_bins - 1);

        if air_high_bin > air_low_bin {
            let air_sum: f32 = error_db[air_low_bin..=air_high_bin].iter().sum();
            let air_count = (air_high_bin - air_low_bin + 1) as f32;
            let avg_error = air_sum / air_count;

            // For hyped sound, always boost air
            params.air_gain = (avg_error + 3.0).clamp(2.0, 5.0); // Always boost at least +2 dB
        }

        params
    }

    /// Find peak error (max or min) in a frequency range
    ///
    /// Returns (bin_index, error_value)
    fn find_peak_in_range(
        error_db: &[f32],
        low_bin: usize,
        high_bin: usize,
        find_positive: bool,
    ) -> (usize, f32) {
        let mut peak_bin = low_bin;
        let mut peak_val = error_db.get(low_bin).copied().unwrap_or(0.0);

        for bin in low_bin..=high_bin.min(error_db.len() - 1) {
            let val = error_db[bin];
            if find_positive {
                if val > peak_val {
                    peak_val = val;
                    peak_bin = bin;
                }
            } else {
                if val < peak_val {
                    peak_val = val;
                    peak_bin = bin;
                }
            }
        }

        (peak_bin, peak_val)
    }

    /// Estimate Q factor based on how focused the error is around the peak
    fn estimate_q_from_error(
        error_db: &[f32],
        peak_bin: usize,
        bin_freq: f32,
        zone_low_hz: f32,
        zone_high_hz: f32,
    ) -> f32 {
        let peak_val = error_db.get(peak_bin).copied().unwrap_or(0.0).abs();
        if peak_val < 0.5 {
            return 1.0; // Default Q for small errors
        }

        // Find -3dB points (where error is half of peak)
        let half_peak = peak_val * 0.5;

        let zone_low_bin = ((zone_low_hz / bin_freq).round() as usize).min(error_db.len() - 1);
        let zone_high_bin = ((zone_high_hz / bin_freq).round() as usize).min(error_db.len() - 1);

        // Search left for -3dB point
        let mut left_bin = peak_bin;
        for bin in (zone_low_bin..peak_bin).rev() {
            if error_db[bin].abs() < half_peak {
                left_bin = bin;
                break;
            }
        }

        // Search right for -3dB point
        let mut right_bin = peak_bin;
        for bin in (peak_bin + 1)..=zone_high_bin {
            if error_db[bin].abs() < half_peak {
                right_bin = bin;
                break;
            }
        }

        // Calculate bandwidth in octaves
        let left_freq = (left_bin as f32 * bin_freq).max(1.0);
        let right_freq = (right_bin as f32 * bin_freq).max(left_freq + 1.0);
        let _center_freq = (peak_bin as f32 * bin_freq).max(1.0);
        let bandwidth_octaves = (right_freq / left_freq).log2();

        // Q = 1 / (2 * sinh(ln(2)/2 * BW))
        // Simplified: Q approximately center_freq / bandwidth_hz
        // Or from octaves: Q = sqrt(2^BW) / (2^BW - 1) for standard definition
        let q = if bandwidth_octaves > 0.1 {
            let bw_factor = 2.0_f32.powf(bandwidth_octaves);
            bw_factor.sqrt() / (bw_factor - 1.0)
        } else {
            3.0 // Narrow Q for very focused problems
        };

        q.clamp(0.5, 4.0)
    }

    /// Scale all gains by amount (0.0-1.0) for single-knob control
    pub fn scale_by_amount(&mut self, amount: f32) {
        let amount = amount.clamp(0.0, 1.0);
        self.low_shelf_gain *= amount;
        self.mud_gain *= amount;
        self.presence_gain *= amount;
        self.air_gain *= amount;
        // HPF interpolates from 20 Hz (0%) to target (100%)
        self.hpf_freq = 20.0 + (self.hpf_freq - 20.0) * amount;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_params() {
        let params = RadioFilterParams::default();
        assert_eq!(params.hpf_freq, 60.0);
        assert_eq!(params.low_shelf_freq, 120.0);
        assert_eq!(params.low_shelf_gain, 0.0);
        assert_eq!(params.mud_gain, 0.0);
        assert_eq!(params.presence_gain, 0.0);
        assert_eq!(params.air_gain, 0.0);
    }

    #[test]
    fn test_fit_hyped_curve() {
        // Flat error should still result in hyped gains (built-in boost)
        let error_db = vec![0.0; 2049];
        let params = RadioFilterParams::fit_from_error(&error_db, 120.0, 48000.0 / 4096.0, 2049);

        // Hyped sound always has bass boost, mud cut, presence boost, air boost
        assert!(params.low_shelf_gain > 2.0, "Should have bass boost");
        assert!(params.mud_gain < -1.0, "Should have mud cut");
        assert!(params.presence_gain > 2.0, "Should have presence boost");
        assert!(params.air_gain > 2.0, "Should have air boost");
    }

    #[test]
    fn test_scale_by_amount() {
        let mut params = RadioFilterParams {
            hpf_freq: 60.0,
            low_shelf_freq: 120.0,
            low_shelf_gain: 4.0,
            mud_freq: 400.0,
            mud_gain: -4.0,
            mud_q: 1.0,
            presence_freq: 4000.0,
            presence_gain: 4.0,
            presence_q: 1.0,
            air_gain: 4.0,
        };

        params.scale_by_amount(0.5);

        assert!((params.low_shelf_gain - 2.0).abs() < 0.01);
        assert!((params.mud_gain - (-2.0)).abs() < 0.01);
        assert!((params.presence_gain - 2.0).abs() < 0.01);
        assert!((params.air_gain - 2.0).abs() < 0.01);
        // HPF should interpolate: 20 + (60-20)*0.5 = 40
        assert!((params.hpf_freq - 40.0).abs() < 0.01);
    }
}
