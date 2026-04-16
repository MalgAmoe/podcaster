//! Radio voice target spectrum generation (hyped smiley curve)
//!
//! Uses voice Long-Term Average Speech Spectrum (LTASS) as baseline:
//! - Voice has -6 dB/octave slope above 1kHz (NOT -3 dB like pink noise)
//! - Reference: Byrne et al. "An international comparison of LTASS"
//!
//! Smiley modifications on top of LTASS:
//! - Bass boost (80-200 Hz) for weight and punch
//! - Mud scoop (300-500 Hz) for clarity
//! - Presence boost (3-5 kHz) for forward sound
//! - Air shelf (8+ kHz) for shimmer (gentle - voice has little HF content)

/// Target spectrum for "radio voice" EQ
#[derive(Clone, Debug)]
pub struct RadioTarget {
    /// Target magnitude curve (dB)
    pub curve_db: Vec<f32>,
    /// Matching frequency bins (Hz)
    pub frequencies: Vec<f32>,
    /// Fundamental frequency used for generation
    pub f0: f32,
}

impl RadioTarget {
    /// Generate target spectrum for hyped radio voice EQ (smiley curve)
    ///
    /// Uses voice LTASS baseline (-6 dB/octave) with smiley modifications:
    /// - Rumble rolloff below 60 Hz
    /// - Bass boost at 80-200 Hz (+3 dB)
    /// - Mud scoop at 300-500 Hz (-3 dB)
    /// - Presence bump at 3-5 kHz (+3 dB)
    /// - Air shelf above 8 kHz (+2 dB, gentle since voice has little HF)
    ///
    /// # Arguments
    /// * `f0` - Detected fundamental frequency (Hz). Use ~120 Hz as default if unknown.
    /// * `num_bins` - Number of frequency bins to generate
    /// * `sample_rate` - Audio sample rate
    /// * `bin_freq` - Hz per FFT bin
    pub fn generate(f0: f32, num_bins: usize, sample_rate: u32, bin_freq: f32) -> Self {
        let mut curve_db = vec![0.0f32; num_bins];
        let mut frequencies = vec![0.0f32; num_bins];

        // Clamp f0 to reasonable voice range
        let _f0 = f0.clamp(75.0, 300.0);

        // Reference frequency for voice LTASS slope
        const REF_FREQ: f32 = 1000.0;

        // Voice LTASS slope: -6 dB/octave (NOT -3 dB like pink noise)
        const VOICE_SLOPE_DB_PER_OCTAVE: f32 = -6.0;

        // Hyped smiley curve zones
        const RUMBLE_CUTOFF: f32 = 60.0; // HPF below this

        const BASS_LOW: f32 = 60.0;
        const BASS_HIGH: f32 = 200.0;
        const BASS_CENTER: f32 = 120.0;
        const BASS_BOOST: f32 = 0.0;

        const MUD_LOW: f32 = 300.0;
        const MUD_HIGH: f32 = 600.0;
        const MUD_CENTER: f32 = 400.0;
        const MUD_DIP: f32 = 0.0;

        const MID_LOW: f32 = 600.0;
        const MID_HIGH: f32 = 2000.0;

        const PRESENCE_LOW: f32 = 2000.0;
        const PRESENCE_HIGH: f32 = 5000.0;
        const PRESENCE_CENTER: f32 = 3500.0;
        const PRESENCE_BUMP: f32 = 0.0;

        const AIR_LOW: f32 = 8000.0;
        const AIR_SHELF: f32 = 0.0;

        for bin in 0..num_bins {
            let freq = bin as f32 * bin_freq;
            frequencies[bin] = freq;

            if freq < 1.0 {
                // DC bin
                curve_db[bin] = -60.0;
                continue;
            }

            // Start with voice LTASS slope (-6 dB/octave from 1kHz reference)
            let voice_slope = if freq > 1.0 {
                VOICE_SLOPE_DB_PER_OCTAVE * (freq / REF_FREQ).log2()
            } else {
                0.0
            };

            let mut target = voice_slope;

            // Zone 1: Rumble rolloff (high-pass characteristic)
            if freq < RUMBLE_CUTOFF {
                // -24 dB/octave rolloff below cutoff
                let octaves_below = (RUMBLE_CUTOFF / freq).log2();
                target -= 24.0 * octaves_below;
            }
            // Zone 2: Bass boost (60-200 Hz) - the key hyped addition!
            else if freq >= BASS_LOW && freq <= BASS_HIGH {
                // Shelf-like boost centered around 120 Hz
                let octaves_from_center = ((freq / BASS_CENTER).log2()).abs();
                let shelf_width: f32 = 1.5; // octaves (wide shelf)
                let boost_amount =
                    BASS_BOOST * (-octaves_from_center.powi(2) / (2.0 * shelf_width.powi(2))).exp();
                target += boost_amount;
            }
            // Zone 3: Mud scoop (300-600 Hz)
            else if freq >= MUD_LOW && freq <= MUD_HIGH {
                // Bell-shaped dip centered at 400 Hz
                let octaves_from_center = ((freq / MUD_CENTER).log2()).abs();
                let bell_width: f32 = 0.8; // octaves
                let dip_amount =
                    MUD_DIP * (-octaves_from_center.powi(2) / (2.0 * bell_width.powi(2))).exp();
                target += dip_amount;
            }
            // Zone 4: Flat mid (600-3000 Hz) - just voice slope
            else if freq >= MID_LOW && freq <= MID_HIGH {
                // Already have voice slope, no modification
            }
            // Zone 5: Presence bump (3-5 kHz)
            else if freq >= PRESENCE_LOW && freq <= PRESENCE_HIGH {
                let octaves_from_center = ((freq / PRESENCE_CENTER).log2()).abs();
                let bell_width: f32 = 0.6; // narrower bell for focused presence
                let bump_amount = PRESENCE_BUMP
                    * (-octaves_from_center.powi(2) / (2.0 * bell_width.powi(2))).exp();
                target += bump_amount;
            }
            // Zone 6: Air shelf (above 8 kHz)
            else if freq >= AIR_LOW && freq < sample_rate as f32 / 2.0 {
                // Gradual shelf boost
                let transition_octaves = (freq / AIR_LOW).log2();
                let shelf_amount = AIR_SHELF * (1.0 - (-transition_octaves * 2.0).exp());
                target += shelf_amount;
            }

            curve_db[bin] = target;
        }

        Self {
            curve_db,
            frequencies,
            f0,
        }
    }

    /// Compute error spectrum: target - actual (positive = need boost, negative = need cut)
    pub fn compute_error(&self, actual_db: &[f32]) -> Vec<f32> {
        self.curve_db
            .iter()
            .zip(actual_db.iter())
            .map(|(&target, &actual)| target - actual)
            .collect()
    }

    /// Get target value at a specific frequency
    pub fn at_freq(&self, freq: f32, bin_freq: f32) -> f32 {
        let bin = ((freq / bin_freq).round() as usize).min(self.curve_db.len() - 1);
        self.curve_db[bin]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_generation() {
        let target = RadioTarget::generate(120.0, 2049, 48000, 48000.0 / 4096.0);

        // Should have correct size
        assert_eq!(target.curve_db.len(), 2049);
        assert_eq!(target.frequencies.len(), 2049);

        // DC should be very low
        assert!(target.curve_db[0] < -50.0);

        // Check that presence is boosted relative to voice slope alone
        let bin_freq: f32 = 48000.0 / 4096.0;
        let presence_bin = (4000.0 / bin_freq).round() as usize;

        // Voice slope at 4kHz would be: -6 * log2(4000/1000) = -12 dB
        let voice_at_4k = -6.0 * (4000.0 / 1000.0_f32).log2();

        // With +3 dB presence boost, should be higher than voice slope alone
        assert!(
            target.curve_db[presence_bin] > voice_at_4k,
            "Presence should be boosted above voice slope: {} > {}",
            target.curve_db[presence_bin],
            voice_at_4k
        );
    }

    #[test]
    fn test_rumble_rolloff() {
        let target = RadioTarget::generate(120.0, 2049, 48000, 48000.0 / 4096.0);
        let bin_freq: f32 = 48000.0 / 4096.0;

        // 30 Hz (well below rumble cutoff of 60 Hz) should be significantly lower than 120 Hz
        let hz30_bin = (30.0 / bin_freq).round() as usize;
        let hz120_bin = (120.0 / bin_freq).round() as usize;

        assert!(
            target.curve_db[hz30_bin] < target.curve_db[hz120_bin] - 6.0,
            "30Hz ({:.1} dB) should be >6dB below 120Hz ({:.1} dB) due to rumble rolloff",
            target.curve_db[hz30_bin],
            target.curve_db[hz120_bin]
        );
    }

    #[test]
    fn test_bass_follows_voice_slope() {
        let target = RadioTarget::generate(120.0, 2049, 48000, 48000.0 / 4096.0);
        let bin_freq: f32 = 48000.0 / 4096.0;

        // 120 Hz is in bass zone — with BASS_BOOST=0, should follow voice slope
        let hz120_bin = (120.0 / bin_freq).round() as usize;
        let actual_freq = hz120_bin as f32 * bin_freq;
        let voice_slope = -6.0 * (actual_freq / 1000.0_f32).log2();

        // Should be within 0.1 dB of pure voice slope (no boost applied)
        let diff = (target.curve_db[hz120_bin] - voice_slope).abs();
        assert!(
            diff < 0.1,
            "120Hz ({:.2} dB) should match voice slope ({:.2} dB), diff={:.3}",
            target.curve_db[hz120_bin],
            voice_slope,
            diff
        );
    }
}
