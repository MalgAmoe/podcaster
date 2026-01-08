//! Dynamic EQ band implementation
//!
//! Single band dynamic processor with:
//! - Sidechain detection (wide Q)
//! - Envelope follower (attack/release)
//! - Gain computer (threshold/ratio)
//! - Bell filter (narrow Q processing)
#![cfg_attr(all(feature = "cli", feature = "plugin"), allow(dead_code))]

use super::common::SvfBiquad;

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

    /// Create a de-mud preset (300Hz default, reduces proximity effect)
    pub fn new_demud(sample_rate: f32) -> Self {
        Self::new_demud_at(300.0, sample_rate)
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
        self.sidechain.reset();
        self.filter.reset();
    }

    pub fn get_gain_reduction_db(&self) -> f32 {
        self.current_gain_db
    }
}
