//! Dynamic EQ band implementation
//!
//! Single band dynamic processor with:
//! - Sidechain detection (wide Q)
//! - Envelope follower (attack/release)
//! - Gain computer (threshold/ratio)
//! - Bell filter (narrow Q processing)
//!
//! Also includes DynamicBandCut for flat band-cut processing (de-esser).

use super::filters::{BandExtractFilter, SvfBiquad};

/// A single dynamic EQ band with sidechain detection and bell filter processing
#[derive(Clone, Debug)]
pub struct DynamicBand {
    // Detection chain
    sidechain: SvfBiquad,
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,

    // Processing chain
    filter: SvfBiquad,
    current_gain_db: f32,
    max_gain_reduction_db: f32,
    gain_smooth_coeff: f32,

    // Parameters
    threshold_db: f32,
    ratio: f32,
    max_cut_db: f32,
}

impl DynamicBand {
    /// Create a new dynamic band
    pub fn new(
        fc: f32,
        q_detect: f32,
        q_process: f32,
        sample_rate: f32,
        attack_ms: f32,
        release_ms: f32,
    ) -> Self {
        let sidechain = SvfBiquad::new(fc, sample_rate, q_detect);
        let filter = SvfBiquad::new(fc, sample_rate, q_process);

        let attack_coeff = 1.0 - (-1000.0 / (attack_ms * sample_rate)).exp();
        let release_coeff = 1.0 - (-1000.0 / (release_ms * sample_rate)).exp();
        let gain_smooth_coeff = 1.0 - (-1000.0 / (10.0 * sample_rate)).exp();

        Self {
            sidechain,
            envelope: 0.0,
            attack_coeff,
            release_coeff,
            filter,
            current_gain_db: 0.0,
            max_gain_reduction_db: 0.0,
            gain_smooth_coeff,
            threshold_db: -20.0,
            ratio: 2.0,
            max_cut_db: -6.0,
        }
    }

    /// Create a de-mud preset with custom center frequency
    pub fn new_demud_at(center_freq: f32, sample_rate: f32) -> Self {
        Self::new(center_freq, 1.2, 1.0, sample_rate, 50.0, 300.0)
            .with_threshold(-20.0)
            .with_ratio(2.0)
            .with_max_cut(-6.0)
    }

    /// Create a mid-range correction preset with custom center frequency
    /// Used for taming resonances and harshness in 500-5000 Hz range
    pub fn new_correction_at(center_freq: f32, sample_rate: f32) -> Self {
        Self::new(center_freq, 1.5, 2.0, sample_rate, 20.0, 150.0) // Medium Q, medium attack
            .with_threshold(-22.0)
            .with_ratio(2.5)
            .with_max_cut(-6.0)
    }

    pub fn with_threshold(mut self, threshold_db: f32) -> Self {
        self.threshold_db = threshold_db;
        self
    }

    pub fn with_ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio;
        self
    }

    pub fn with_max_cut(mut self, max_cut_db: f32) -> Self {
        self.max_cut_db = max_cut_db;
        self
    }

    /// Process a single sample
    pub fn process(&mut self, input: f32, macro_val: f32) -> f32 {
        if macro_val <= 0.0 {
            return input;
        }

        let (adj_threshold, adj_ratio, adj_max_cut) = self.compute_adjusted_params(macro_val);

        // Sidechain detection
        let (_, sc_bp, _) = self.sidechain.process(input);
        let abs_sc = sc_bp.abs();

        // Envelope follower
        let coeff = if abs_sc > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope += coeff * (abs_sc - self.envelope);

        // Gain computer
        let env_db = 20.0 * (self.envelope.max(1e-6)).log10();
        let overshoot = env_db - adj_threshold;

        let target_gain_db = if overshoot > 0.0 {
            let reduction = (overshoot * (1.0 - 1.0 / adj_ratio)) * -1.0;
            reduction.max(adj_max_cut)
        } else {
            0.0
        };

        // Smooth gain changes
        self.current_gain_db += self.gain_smooth_coeff * (target_gain_db - self.current_gain_db);
        self.max_gain_reduction_db = self.max_gain_reduction_db.min(self.current_gain_db);

        // Apply bell filter
        let (_, bp, _) = self.filter.process(input);
        let gain_lin = 10.0f32.powf(self.current_gain_db / 20.0);

        input + (gain_lin - 1.0) * bp
    }

    fn compute_adjusted_params(&self, macro_val: f32) -> (f32, f32, f32) {
        let adj_threshold = self.threshold_db - (macro_val * 6.0);
        let adj_ratio = 1.0 + (self.ratio - 1.0) * macro_val;
        let adj_max_cut = self.max_cut_db * macro_val;
        (adj_threshold, adj_ratio, adj_max_cut)
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.current_gain_db = 0.0;
        self.max_gain_reduction_db = 0.0;
        self.sidechain.reset();
        self.filter.reset();
    }

    pub fn get_gain_reduction_db(&self) -> f32 {
        self.current_gain_db
    }

    pub fn get_max_gain_reduction_db(&self) -> f32 {
        self.max_gain_reduction_db
    }
}

// =============================================================================
// Dynamic Band Cut (Flat Band-Cut for De-Esser)
// =============================================================================

/// Dynamic band-cut processor with flat frequency response
/// Unlike DynamicBand (peaked resonant filter), this cuts a flat frequency band.
///
/// Uses BandExtractFilter (cascaded HP → LP) to extract the band,
/// then dynamically subtracts it from the input based on envelope detection.
#[derive(Clone, Debug)]
pub struct DynamicBandCut {
    // Detection chain
    sidechain: BandExtractFilter,
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,

    // Processing chain
    filter: BandExtractFilter,
    current_gain_db: f32,
    max_gain_reduction_db: f32,
    gain_smooth_coeff: f32,

    // Parameters
    threshold_db: f32,
    ratio: f32,
    max_cut_db: f32,
}

impl DynamicBandCut {
    /// Create a new dynamic band-cut processor
    pub fn new(
        start_freq: f32,
        stop_freq: f32,
        sample_rate: f32,
        attack_ms: f32,
        release_ms: f32,
    ) -> Self {
        let sidechain = BandExtractFilter::new(start_freq, stop_freq, sample_rate);
        let filter = BandExtractFilter::new(start_freq, stop_freq, sample_rate);

        let attack_coeff = 1.0 - (-1000.0 / (attack_ms * sample_rate)).exp();
        let release_coeff = 1.0 - (-1000.0 / (release_ms * sample_rate)).exp();
        let gain_smooth_coeff = 1.0 - (-1000.0 / (10.0 * sample_rate)).exp();

        Self {
            sidechain,
            envelope: 0.0,
            attack_coeff,
            release_coeff,
            filter,
            current_gain_db: 0.0,
            max_gain_reduction_db: 0.0,
            gain_smooth_coeff,
            threshold_db: -20.0,
            ratio: 2.0,
            max_cut_db: -6.0,
        }
    }

    pub fn with_threshold(mut self, threshold_db: f32) -> Self {
        self.threshold_db = threshold_db;
        self
    }

    pub fn with_ratio(mut self, ratio: f32) -> Self {
        self.ratio = ratio;
        self
    }

    pub fn with_max_cut(mut self, max_cut_db: f32) -> Self {
        self.max_cut_db = max_cut_db;
        self
    }

    /// Update band frequencies
    pub fn set_band(&mut self, start_freq: f32, stop_freq: f32) {
        self.sidechain.set_band(start_freq, stop_freq);
        self.filter.set_band(start_freq, stop_freq);
    }

    /// Process a single sample with flat band-cut
    pub fn process(&mut self, input: f32, strength: f32) -> f32 {
        if strength <= 0.0 {
            return input;
        }

        let (adj_threshold, adj_ratio, adj_max_cut) = self.compute_adjusted_params(strength);

        // Sidechain detection - extract band energy
        let band_energy = self.sidechain.process(input);
        let abs_band = band_energy.abs();

        // Envelope follower
        let coeff = if abs_band > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope += coeff * (abs_band - self.envelope);

        // Gain computer
        let env_db = 20.0 * (self.envelope.max(1e-6)).log10();
        let overshoot = env_db - adj_threshold;

        let target_gain_db = if overshoot > 0.0 {
            let reduction = (overshoot * (1.0 - 1.0 / adj_ratio)) * -1.0;
            reduction.max(adj_max_cut)
        } else {
            0.0
        };

        // Smooth gain changes
        self.current_gain_db += self.gain_smooth_coeff * (target_gain_db - self.current_gain_db);
        self.max_gain_reduction_db = self.max_gain_reduction_db.min(self.current_gain_db);

        // Apply band-cut: subtract scaled band from input
        // When gain_db = 0, gain_lin = 1.0, no subtraction
        // When gain_db = -6, gain_lin = 0.5, subtract 50% of band
        let band_output = self.filter.process(input);
        let gain_lin = 10.0f32.powf(self.current_gain_db / 20.0);

        input - (1.0 - gain_lin) * band_output
    }

    fn compute_adjusted_params(&self, strength: f32) -> (f32, f32, f32) {
        let adj_threshold = self.threshold_db - (strength * 6.0);
        let adj_ratio = 1.0 + (self.ratio - 1.0) * strength;
        let adj_max_cut = self.max_cut_db * strength;
        (adj_threshold, adj_ratio, adj_max_cut)
    }

    pub fn reset(&mut self) {
        self.envelope = 0.0;
        self.current_gain_db = 0.0;
        self.max_gain_reduction_db = 0.0;
        self.sidechain.reset();
        self.filter.reset();
    }

    pub fn get_gain_reduction_db(&self) -> f32 {
        self.current_gain_db
    }

    pub fn get_max_gain_reduction_db(&self) -> f32 {
        self.max_gain_reduction_db
    }
}
