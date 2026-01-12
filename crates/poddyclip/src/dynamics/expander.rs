//! Stereo Linked Expander
//!
//! Reduces gain for signals below threshold (opposite of compressor).
//! Useful for reducing noise/room tone during quiet passages.

use crate::analysis::utils::{db_to_linear, linear_to_db};

/// Mono expander with envelope follower
#[derive(Clone, Debug)]
pub struct Expander {
    sample_rate: f32,

    // Parameters
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    range_db: f32,

    // Coefficients
    attack_coeff: f32,
    release_coeff: f32,

    // State
    envelope_db: f32,
    gain_reduction_db: f32,
    max_gain_reduction_db: f32,
}

impl Expander {
    /// Create a new expander
    pub fn new(sample_rate: f32) -> Self {
        let mut exp = Self {
            sample_rate,
            threshold_db: -40.0,
            ratio: 2.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 6.0,
            range_db: 20.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            envelope_db: -100.0,
            gain_reduction_db: 0.0,
            max_gain_reduction_db: 0.0,
        };
        exp.update_coefficients();
        exp
    }

    fn update_coefficients(&mut self) {
        self.attack_coeff = (-2.2 / (self.attack_ms * self.sample_rate / 1000.0)).exp();
        self.release_coeff = (-2.2 / (self.release_ms * self.sample_rate / 1000.0)).exp();
    }

    /// Calculate expansion amount based on level
    fn calculate_expansion(&self, level_db: f32) -> f32 {
        let knee_start = self.threshold_db - self.knee_db / 2.0;
        let knee_end = self.threshold_db + self.knee_db / 2.0;

        if level_db >= knee_end {
            // Above threshold: no expansion
            0.0
        } else if level_db <= knee_start {
            // Below knee: full expansion
            let below = self.threshold_db - level_db;
            (below * (1.0 - 1.0 / self.ratio)).min(self.range_db)
        } else {
            // In knee: smooth transition (quadratic)
            let x = (knee_end - level_db) / self.knee_db;
            let below = self.threshold_db - level_db;
            x * x * below * (1.0 - 1.0 / self.ratio)
        }
    }

    /// Process a single sample
    pub fn process(&mut self, input: f32) -> f32 {
        let level_db = linear_to_db(input.abs());

        // Envelope follower (peak detection)
        if level_db > self.envelope_db {
            // Signal rising: fast attack
            self.envelope_db =
                self.attack_coeff * self.envelope_db + (1.0 - self.attack_coeff) * level_db;
        } else {
            // Signal falling: slower release
            self.envelope_db =
                self.release_coeff * self.envelope_db + (1.0 - self.release_coeff) * level_db;
        }

        // Calculate expansion based on envelope
        let target_gr = self.calculate_expansion(self.envelope_db);

        // Smooth gain reduction changes
        if target_gr > self.gain_reduction_db {
            // Entering expansion (gain decreasing)
            self.gain_reduction_db =
                self.attack_coeff * self.gain_reduction_db + (1.0 - self.attack_coeff) * target_gr;
        } else {
            // Exiting expansion (gain recovering)
            self.gain_reduction_db = self.release_coeff * self.gain_reduction_db
                + (1.0 - self.release_coeff) * target_gr;
        }

        // Track max for display
        if self.gain_reduction_db > self.max_gain_reduction_db {
            self.max_gain_reduction_db = self.gain_reduction_db;
        }

        // Apply gain
        let gain = db_to_linear(-self.gain_reduction_db);
        input * gain
    }

    /// Set threshold in dB
    pub fn set_threshold(&mut self, db: f32) {
        self.threshold_db = db;
    }

    /// Set expansion ratio (e.g., 2.0 for 2:1)
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
    }

    /// Set attack time in ms
    pub fn set_attack(&mut self, ms: f32) {
        self.attack_ms = ms;
        self.update_coefficients();
    }

    /// Set release time in ms
    pub fn set_release(&mut self, ms: f32) {
        self.release_ms = ms;
        self.update_coefficients();
    }

    /// Set maximum expansion range in dB
    pub fn set_range(&mut self, db: f32) {
        self.range_db = db;
    }

    /// Get maximum gain reduction seen (for metering)
    pub fn get_max_gain_reduction_db(&self) -> f32 {
        self.max_gain_reduction_db
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.envelope_db = -100.0;
        self.gain_reduction_db = 0.0;
        self.max_gain_reduction_db = 0.0;
    }

    /// Reset max gain reduction meter
    pub fn reset_meter(&mut self) {
        self.max_gain_reduction_db = 0.0;
    }
}

impl crate::traits::AudioProcessor for Expander {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

impl crate::traits::MonoProcessor for Expander {}

impl crate::traits::Processor for Expander {
    fn new(sample_rate: f32) -> Self {
        Self::new(sample_rate)
    }
}

/// Stereo expander with linked detection
#[derive(Clone, Debug)]
pub struct StereoExpander {
    sample_rate: f32,

    // Parameters
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    knee_db: f32,
    range_db: f32,

    // Coefficients
    attack_coeff: f32,
    release_coeff: f32,

    // State (shared for linked operation)
    envelope_db: f32,
    gain_reduction_db: f32,
    max_gain_reduction_db: f32,
}

impl StereoExpander {
    /// Create a new stereo expander
    pub fn new(sample_rate: f32) -> Self {
        let mut exp = Self {
            sample_rate,
            threshold_db: -40.0,
            ratio: 2.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 6.0,
            range_db: 20.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            envelope_db: -100.0,
            gain_reduction_db: 0.0,
            max_gain_reduction_db: 0.0,
        };
        exp.update_coefficients();
        exp
    }

    fn update_coefficients(&mut self) {
        self.attack_coeff = (-2.2 / (self.attack_ms * self.sample_rate / 1000.0)).exp();
        self.release_coeff = (-2.2 / (self.release_ms * self.sample_rate / 1000.0)).exp();
    }

    /// Calculate expansion amount based on level
    fn calculate_expansion(&self, level_db: f32) -> f32 {
        let knee_start = self.threshold_db - self.knee_db / 2.0;
        let knee_end = self.threshold_db + self.knee_db / 2.0;

        if level_db >= knee_end {
            0.0
        } else if level_db <= knee_start {
            let below = self.threshold_db - level_db;
            (below * (1.0 - 1.0 / self.ratio)).min(self.range_db)
        } else {
            let x = (knee_end - level_db) / self.knee_db;
            let below = self.threshold_db - level_db;
            x * x * below * (1.0 - 1.0 / self.ratio)
        }
    }

    /// Process a stereo sample pair with linked detection
    pub fn process_sample(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Linked detection: max of both channels
        let peak = left.abs().max(right.abs());
        let level_db = linear_to_db(peak);

        // Envelope follower
        if level_db > self.envelope_db {
            self.envelope_db =
                self.attack_coeff * self.envelope_db + (1.0 - self.attack_coeff) * level_db;
        } else {
            self.envelope_db =
                self.release_coeff * self.envelope_db + (1.0 - self.release_coeff) * level_db;
        }

        // Calculate expansion
        let target_gr = self.calculate_expansion(self.envelope_db);

        // Smooth gain reduction
        if target_gr > self.gain_reduction_db {
            self.gain_reduction_db =
                self.attack_coeff * self.gain_reduction_db + (1.0 - self.attack_coeff) * target_gr;
        } else {
            self.gain_reduction_db = self.release_coeff * self.gain_reduction_db
                + (1.0 - self.release_coeff) * target_gr;
        }

        // Track max
        if self.gain_reduction_db > self.max_gain_reduction_db {
            self.max_gain_reduction_db = self.gain_reduction_db;
        }

        // Apply same gain to both channels
        let gain = db_to_linear(-self.gain_reduction_db);
        (left * gain, right * gain)
    }

    /// Set threshold in dB
    pub fn set_threshold(&mut self, db: f32) {
        self.threshold_db = db;
    }

    /// Set expansion ratio
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
    }

    /// Set attack time in ms
    pub fn set_attack(&mut self, ms: f32) {
        self.attack_ms = ms;
        self.update_coefficients();
    }

    /// Set release time in ms
    pub fn set_release(&mut self, ms: f32) {
        self.release_ms = ms;
        self.update_coefficients();
    }

    /// Set maximum expansion range in dB
    pub fn set_range(&mut self, db: f32) {
        self.range_db = db;
    }

    /// Get maximum gain reduction seen
    pub fn get_max_gain_reduction_db(&self) -> f32 {
        self.max_gain_reduction_db
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.envelope_db = -100.0;
        self.gain_reduction_db = 0.0;
        self.max_gain_reduction_db = 0.0;
    }

    /// Reset max gain reduction meter
    pub fn reset_meter(&mut self) {
        self.max_gain_reduction_db = 0.0;
    }

    /// Process stereo audio in-place
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (out_l, out_r) = self.process_sample(*l, *r);
            *l = out_l;
            *r = out_r;
        }
    }

    /// Process mono audio in-place
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let (out, _) = self.process_sample(*sample, *sample);
            *sample = out;
        }
    }
}

impl crate::traits::StereoProcessor for StereoExpander {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        StereoExpander::process_stereo(self, left, right)
    }

    fn reset(&mut self) {
        StereoExpander::reset(self)
    }

    fn latency_samples(&self) -> usize {
        0
    }
}
