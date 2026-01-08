//! Air EQ - High frequency enhancement with top-end rolloff
//!
//! Adds presence/air with a gentle 1st order high shelf boost,
//! then rolls off the very top with an SVF LP filter to keep things smooth.

#![allow(dead_code)]

use crate::filters::{HighShelfSvf, SvfBiquad};

/// Default air shelf frequency (10-12kHz recommended for podcast)
const DEFAULT_SHELF_FREQ: f32 = 10000.0;

/// Default shelf gain (+2dB gentle boost)
const DEFAULT_SHELF_GAIN: f32 = 2.0;

/// Default LP cutoff (gentle rolloff above 16kHz)
const DEFAULT_LP_FREQ: f32 = 16000.0;

/// Q for LP filter (Butterworth)
const LP_Q: f32 = 0.707;

/// Air EQ processor - 1st order high shelf boost + SVF LP rolloff
#[derive(Clone, Debug)]
pub struct AirEq {
    sample_rate: f32,
    shelf: HighShelfSvf,
    lowpass: SvfBiquad,
    // Parameters
    shelf_freq: f32,
    shelf_gain_db: f32,
    lp_freq: f32,
    enabled: bool,
}

impl AirEq {
    /// Create a new Air EQ with default settings
    /// - Shelf: 10kHz, +2dB
    /// - LP: 16kHz
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            shelf: HighShelfSvf::new(DEFAULT_SHELF_FREQ, DEFAULT_SHELF_GAIN, sample_rate),
            lowpass: SvfBiquad::new(DEFAULT_LP_FREQ, sample_rate, LP_Q),
            shelf_freq: DEFAULT_SHELF_FREQ,
            shelf_gain_db: DEFAULT_SHELF_GAIN,
            lp_freq: DEFAULT_LP_FREQ,
            enabled: true,
        }
    }

    /// Set shelf frequency (Hz)
    pub fn set_shelf_freq(&mut self, freq: f32) {
        if (self.shelf_freq - freq).abs() > 0.1 {
            self.shelf_freq = freq;
            self.shelf.set_params(freq, self.shelf_gain_db);
        }
    }

    /// Set shelf gain (dB)
    pub fn set_shelf_gain(&mut self, gain_db: f32) {
        if (self.shelf_gain_db - gain_db).abs() > 0.01 {
            self.shelf_gain_db = gain_db;
            self.shelf.set_params(self.shelf_freq, gain_db);
        }
    }

    /// Set LP cutoff frequency (Hz)
    pub fn set_lp_freq(&mut self, freq: f32) {
        if (self.lp_freq - freq).abs() > 0.1 {
            self.lp_freq = freq;
            self.lowpass.update(freq, self.sample_rate, LP_Q);
        }
    }

    /// Enable/disable the Air EQ
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get current shelf frequency
    pub fn get_shelf_freq(&self) -> f32 {
        self.shelf_freq
    }

    /// Get current shelf gain
    pub fn get_shelf_gain(&self) -> f32 {
        self.shelf_gain_db
    }

    /// Get current LP frequency
    pub fn get_lp_freq(&self) -> f32 {
        self.lp_freq
    }

    /// Process a single sample
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if !self.enabled {
            return input;
        }

        // High shelf boost, then LP rolloff
        let shelved = self.shelf.process(input);
        let (lp, _, _) = self.lowpass.process(shelved);
        lp
    }

    /// Process mono buffer in-place
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        if !self.enabled {
            return;
        }
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Reset filter states
    pub fn reset(&mut self) {
        self.shelf.reset();
        self.lowpass.reset();
    }
}

/// Stereo Air EQ processor
#[derive(Clone, Debug)]
pub struct StereoAirEq {
    pub left: AirEq,
    pub right: AirEq,
}

impl StereoAirEq {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            left: AirEq::new(sample_rate),
            right: AirEq::new(sample_rate),
        }
    }

    pub fn set_shelf_freq(&mut self, freq: f32) {
        self.left.set_shelf_freq(freq);
        self.right.set_shelf_freq(freq);
    }

    pub fn set_shelf_gain(&mut self, gain_db: f32) {
        self.left.set_shelf_gain(gain_db);
        self.right.set_shelf_gain(gain_db);
    }

    pub fn set_lp_freq(&mut self, freq: f32) {
        self.left.set_lp_freq(freq);
        self.right.set_lp_freq(freq);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.left.set_enabled(enabled);
        self.right.set_enabled(enabled);
    }

    /// Process stereo buffers in-place
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l = self.left.process(*l);
            *r = self.right.process(*r);
        }
    }

    /// Process mono buffer in-place
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        self.left.process_mono(buffer);
    }

    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}
