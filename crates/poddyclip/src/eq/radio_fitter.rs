//! Zone-based filter fitting for Radio Voice EQ
//!
//! Maps the error spectrum (target - actual) to biquad filter parameters
//! using a greedy zone-based algorithm.
//!
//! Features:
//! - f0-adaptive frequencies (scales mud/presence zones based on voice pitch)
//! - Sibilance awareness (reduces air boost if audio is already bright/sibilant)

/// Reference f0 for frequency scaling (typical male voice)
const REFERENCE_F0: f32 = 120.0;

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
    /// Mid band bell center frequency (800-2000 Hz) - nasal/honky cut
    pub mid_freq: f32,
    /// Mid band gain (typically negative for cut, 0 = off)
    pub mid_gain: f32,
    /// Mid band Q factor
    pub mid_q: f32,
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
            hpf_freq: 60.0,        // Lower HPF for hyped sound
            low_shelf_freq: 120.0, // Bass boost center
            low_shelf_gain: 0.0,   // Set by fitter
            mud_freq: 400.0,       // Mud scoop center
            mud_gain: 0.0,
            mud_q: 1.0,
            mid_freq: 1200.0, // Nasal/honky center
            mid_gain: 0.0,    // 0 = off (only applied if problem detected)
            mid_q: 1.0,
            presence_freq: 4000.0, // Presence boost center
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
        Self::fit_from_error_with_sibilance(error_db, f0, bin_freq, n_bins, 0.0)
    }

    /// Fit filter parameters with sibilance awareness
    ///
    /// # Arguments
    /// * `error_db` - Error spectrum (target - actual), positive = need boost
    /// * `f0` - Detected fundamental frequency (Hz)
    /// * `bin_freq` - Hz per FFT bin
    /// * `n_bins` - Number of FFT bins
    /// * `sibilance_level` - Sibilance confidence (0.0 = none, 1.0 = very sibilant)
    pub fn fit_from_error_with_sibilance(
        error_db: &[f32],
        f0: f32,
        bin_freq: f32,
        n_bins: usize,
        sibilance_level: f32,
    ) -> Self {
        let mut params = Self::default();

        // Clamp f0 to reasonable range
        let f0 = f0.clamp(75.0, 300.0);

        // Calculate frequency scaling factor based on f0
        // Higher voice (f0 > 120) → shift frequencies up
        // Lower voice (f0 < 120) → shift frequencies down
        let freq_scale = (f0 / REFERENCE_F0).sqrt(); // sqrt for gentler scaling

        // 1. HPF: Set cutoff for rumble protection, scales with f0
        // Higher voices can have higher HPF since fundamental is well above
        params.hpf_freq = (f0 * 0.5).clamp(40.0, 120.0);

        // 2. Low shelf: Cutoff above f0 so the fundamental gets boosted
        // Shelf boosts everything BELOW cutoff, so cutoff should be ~1.5x f0
        let low_shelf_cutoff = (f0 * 1.5).clamp(120.0, 300.0);
        let bass_low_hz = f0 * 0.5;
        let bass_high_hz = f0 * 1.5;

        let bass_low_bin = ((bass_low_hz / bin_freq).round() as usize).min(n_bins - 1);
        let bass_high_bin = ((bass_high_hz / bin_freq).round() as usize).min(n_bins - 1);

        if bass_high_bin > bass_low_bin {
            let bass_sum: f32 = error_db[bass_low_bin..=bass_high_bin].iter().sum();
            let bass_count = (bass_high_bin - bass_low_bin + 1) as f32;
            let avg_bass_error = bass_sum / bass_count;

            params.low_shelf_gain = avg_bass_error.clamp(-4.0, 0.0); // No bias, reasonable clamp
            params.low_shelf_freq = low_shelf_cutoff;
        }

        // 3. Mud band: Scale with f0
        // Male (120 Hz) → 300-600 Hz, Female (220 Hz) → ~400-800 Hz
        let mud_center_base = 400.0;
        let mud_center_scaled = mud_center_base * freq_scale;
        let mud_low_hz = (mud_center_scaled * 0.75).clamp(250.0, 500.0);
        let mud_high_hz = (mud_center_scaled * 1.5).clamp(450.0, 800.0);

        let mud_low_bin = ((mud_low_hz / bin_freq).round() as usize).min(n_bins - 1);
        let mud_high_bin = ((mud_high_hz / bin_freq).round() as usize).min(n_bins - 1);

        let (mud_peak_bin, mud_peak_error) =
            Self::find_peak_in_range(error_db, mud_low_bin, mud_high_bin, false);

        params.mud_freq = (mud_peak_bin as f32 * bin_freq).clamp(mud_low_hz, mud_high_hz);
        params.mud_gain = mud_peak_error.clamp(-8.0, 0.0); // No bias, reasonable clamp
        params.mud_q =
            Self::estimate_q_from_error(error_db, mud_peak_bin, bin_freq, mud_low_hz, mud_high_hz);

        // 4. Mid band: Scale with f0, find problems in nasal/honky region
        // Only applies cut if there's actually a problem (error < -1 dB)
        let mid_center_base = 1200.0;
        let mid_center_scaled = mid_center_base * freq_scale;
        let mid_low_hz = (mid_center_scaled * 0.67).clamp(700.0, 1200.0);
        let mid_high_hz = (mid_center_scaled * 1.5).clamp(1400.0, 2500.0);

        let mid_low_bin = ((mid_low_hz / bin_freq).round() as usize).min(n_bins - 1);
        let mid_high_bin = ((mid_high_hz / bin_freq).round() as usize).min(n_bins - 1);

        let (mid_peak_bin, mid_peak_error) =
            Self::find_peak_in_range(error_db, mid_low_bin, mid_high_bin, false);

        // Only apply if there's a significant problem (error < -1 dB means audio is too loud there)
        if mid_peak_error < -1.0 {
            params.mid_freq = (mid_peak_bin as f32 * bin_freq).clamp(mid_low_hz, mid_high_hz);
            params.mid_gain = mid_peak_error.clamp(-6.0, 0.0); // No bias, reasonable clamp
            params.mid_q = Self::estimate_q_from_error(
                error_db,
                mid_peak_bin,
                bin_freq,
                mid_low_hz,
                mid_high_hz,
            );
        }
        // else mid_gain stays 0.0 (off)

        // 5. Presence band: Scale with f0
        // Male (120 Hz) → 3-5 kHz, Female (220 Hz) → ~4-6 kHz
        let presence_center_base = 4000.0;
        let presence_center_scaled = presence_center_base * freq_scale;
        let presence_low_hz = (presence_center_scaled * 0.75).clamp(2500.0, 4500.0);
        let presence_high_hz = (presence_center_scaled * 1.25).clamp(4000.0, 7000.0);

        let presence_low_bin = ((presence_low_hz / bin_freq).round() as usize).min(n_bins - 1);
        let presence_high_bin = ((presence_high_hz / bin_freq).round() as usize).min(n_bins - 1);

        let (presence_peak_bin, presence_peak_error) =
            Self::find_peak_in_range(error_db, presence_low_bin, presence_high_bin, true);

        params.presence_freq =
            (presence_peak_bin as f32 * bin_freq).clamp(presence_low_hz, presence_high_hz);
        params.presence_gain = presence_peak_error.clamp(0.0, 6.0); // No bias, reasonable clamp
        params.presence_q = Self::estimate_q_from_error(
            error_db,
            presence_peak_bin,
            bin_freq,
            presence_low_hz,
            presence_high_hz,
        );

        // 6. Air shelf: Average error above 8 kHz
        // Apply sibilance awareness - reduce boost if audio is already sibilant
        let air_low_bin = ((8000.0 / bin_freq).round() as usize).min(n_bins - 1);
        let air_high_bin = ((16000.0 / bin_freq).round() as usize).min(n_bins - 1);

        if air_high_bin > air_low_bin {
            let air_sum: f32 = error_db[air_low_bin..=air_high_bin].iter().sum();
            let air_count = (air_high_bin - air_low_bin + 1) as f32;
            let avg_error = air_sum / air_count;

            // Base air gain - no bias, target already has +2 dB shelf
            let base_air_gain = avg_error.clamp(0.0, 4.0);

            // Reduce air boost based on sibilance level
            // sibilance=0 → full boost, sibilance=1 → reduced to ~40% of boost
            let sibilance_factor = 1.0 - (sibilance_level.clamp(0.0, 1.0) * 0.6);
            params.air_gain = base_air_gain * sibilance_factor;
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
        self.mid_gain *= amount;
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
    fn test_fit_flat_error() {
        // Flat error (0 everywhere) means audio matches target - no correction needed
        let error_db = vec![0.0; 2049];
        let params = RadioFilterParams::fit_from_error(&error_db, 120.0, 48000.0 / 4096.0, 2049);

        // With perfect match to target, corrections should be minimal
        assert!(
            params.low_shelf_gain.abs() < 1.0,
            "Bass should need minimal correction"
        );
        assert!(
            params.mud_gain.abs() < 1.0,
            "Mud should need minimal correction"
        );
        assert!(
            params.presence_gain.abs() < 1.0,
            "Presence should need minimal correction"
        );
        assert!(
            params.air_gain.abs() < 1.0,
            "Air should need minimal correction"
        );
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
            mid_freq: 1200.0,
            mid_gain: -4.0,
            mid_q: 1.0,
            presence_freq: 4000.0,
            presence_gain: 4.0,
            presence_q: 1.0,
            air_gain: 4.0,
        };

        params.scale_by_amount(0.5);

        assert!((params.low_shelf_gain - 2.0).abs() < 0.01);
        assert!((params.mud_gain - (-2.0)).abs() < 0.01);
        assert!((params.mid_gain - (-2.0)).abs() < 0.01);
        assert!((params.presence_gain - 2.0).abs() < 0.01);
        assert!((params.air_gain - 2.0).abs() < 0.01);
        // HPF should interpolate: 20 + (60-20)*0.5 = 40
        assert!((params.hpf_freq - 40.0).abs() < 0.01);
    }
}
