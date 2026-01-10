//! Enhance EQ - Three-band enhancement processor
//!
//! Three-stage processor:
//! 1. Low-mid cut: Bell filter at 250Hz to counteract compression/saturation buildup
//! 2. Presence boost: Bell filter in 2-5kHz for voice clarity
//! 3. Dynamic Air: Envelope-following high shelf that only boosts when signal present
//!
//! The dynamic air prevents boosting noise/hiss in quiet passages.

#![allow(dead_code)]

use super::filters::{HighShelfSvf, SvfBiquad};

/// Default low-mid cut frequency
const DEFAULT_LOWMID_FREQ: f32 = 250.0;

/// Default low-mid cut gain (-1.5dB cut)
const DEFAULT_LOWMID_GAIN: f32 = -3.0;

/// Default low-mid Q (narrower than presence)
const DEFAULT_LOWMID_Q: f32 = 1.5;

/// Default presence frequency (voice clarity range)
const DEFAULT_PRESENCE_FREQ: f32 = 3000.0;

/// Default presence gain (+1dB subtle boost)
const DEFAULT_PRESENCE_GAIN: f32 = 1.0;

/// Default presence Q (moderate width)
const DEFAULT_PRESENCE_Q: f32 = 1.2;

/// Default air shelf frequency (10-12kHz recommended for podcast)
const DEFAULT_SHELF_FREQ: f32 = 10000.0;

/// Default shelf gain (+3dB) - this is max gain for dynamic
const DEFAULT_SHELF_GAIN: f32 = 2.0;

/// Default LP cutoff (gentle rolloff above 16kHz)
const DEFAULT_LP_FREQ: f32 = 16000.0;

/// Q for LP filter (Butterworth)
const LP_Q: f32 = 0.707;

/// Envelope threshold for dynamic air (-40dB = above noise floor)
const DEFAULT_ENV_THRESHOLD_DB: f32 = -40.0;

/// Attack time for envelope follower (ms)
const DEFAULT_ENV_ATTACK_MS: f32 = 5.0;

/// Release time for envelope follower (ms)
const DEFAULT_ENV_RELEASE_MS: f32 = 100.0;

/// Enhance EQ processor - Low-mid cut + Presence bell + Dynamic high shelf + LP rolloff
#[derive(Clone, Debug)]
pub struct EnhanceEq {
    sample_rate: f32,

    // Low-mid cut (static bell filter)
    lowmid: SvfBiquad,
    lowmid_freq: f32,
    lowmid_gain_db: f32,

    // Presence boost (static bell filter)
    presence: SvfBiquad,
    presence_freq: f32,
    presence_gain_db: f32,

    // Dynamic air shelf
    shelf: HighShelfSvf,
    lowpass: SvfBiquad,

    // Envelope follower for dynamic operation
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
    threshold_db: f32,

    // Parameters
    shelf_freq: f32,
    shelf_gain_db: f32, // Max gain when signal present
    lp_freq: f32,
    enabled: bool,
}

impl EnhanceEq {
    /// Create a new Enhance EQ with default settings
    /// - Low-mid: 250Hz bell, -1.5dB
    /// - Presence: 3kHz bell, +1dB
    /// - Air: 10kHz dynamic shelf, +3dB max
    /// - LP: 16kHz rolloff
    pub fn new(sample_rate: f32) -> Self {
        // Calculate envelope coefficients
        let attack_coeff = 1.0 - (-1000.0 / (DEFAULT_ENV_ATTACK_MS * sample_rate)).exp();
        let release_coeff = 1.0 - (-1000.0 / (DEFAULT_ENV_RELEASE_MS * sample_rate)).exp();

        Self {
            sample_rate,
            // Low-mid cut bell filter
            lowmid: SvfBiquad::new(DEFAULT_LOWMID_FREQ, sample_rate, DEFAULT_LOWMID_Q),
            lowmid_freq: DEFAULT_LOWMID_FREQ,
            lowmid_gain_db: DEFAULT_LOWMID_GAIN,
            // Presence bell filter
            presence: SvfBiquad::new(DEFAULT_PRESENCE_FREQ, sample_rate, DEFAULT_PRESENCE_Q),
            presence_freq: DEFAULT_PRESENCE_FREQ,
            presence_gain_db: DEFAULT_PRESENCE_GAIN,
            // Dynamic air shelf (starts at 0 gain, modulated by envelope)
            shelf: HighShelfSvf::new(DEFAULT_SHELF_FREQ, 0.0, sample_rate),
            lowpass: SvfBiquad::new(DEFAULT_LP_FREQ, sample_rate, LP_Q),
            // Envelope follower
            envelope: 0.0,
            attack_coeff,
            release_coeff,
            threshold_db: DEFAULT_ENV_THRESHOLD_DB,
            // Parameters
            shelf_freq: DEFAULT_SHELF_FREQ,
            shelf_gain_db: DEFAULT_SHELF_GAIN,
            lp_freq: DEFAULT_LP_FREQ,
            enabled: true,
        }
    }

    /// Set low-mid frequency (Hz)
    pub fn set_lowmid_freq(&mut self, freq: f32) {
        if (self.lowmid_freq - freq).abs() > 0.1 {
            self.lowmid_freq = freq;
            self.lowmid.update(freq, self.sample_rate, DEFAULT_LOWMID_Q);
        }
    }

    /// Set low-mid gain (dB) - typically negative for cut
    pub fn set_lowmid_gain(&mut self, gain_db: f32) {
        self.lowmid_gain_db = gain_db;
    }

    /// Set presence frequency (Hz)
    pub fn set_presence_freq(&mut self, freq: f32) {
        if (self.presence_freq - freq).abs() > 0.1 {
            self.presence_freq = freq;
            self.presence.update(freq, self.sample_rate, DEFAULT_PRESENCE_Q);
        }
    }

    /// Set presence gain (dB)
    pub fn set_presence_gain(&mut self, gain_db: f32) {
        self.presence_gain_db = gain_db;
    }

    /// Set shelf frequency (Hz)
    pub fn set_shelf_freq(&mut self, freq: f32) {
        if (self.shelf_freq - freq).abs() > 0.1 {
            self.shelf_freq = freq;
            // Don't update shelf params here - they're set dynamically in process()
        }
    }

    /// Set shelf max gain (dB) - the ceiling for dynamic operation
    pub fn set_shelf_gain(&mut self, gain_db: f32) {
        self.shelf_gain_db = gain_db;
    }

    /// Set LP cutoff frequency (Hz)
    pub fn set_lp_freq(&mut self, freq: f32) {
        if (self.lp_freq - freq).abs() > 0.1 {
            self.lp_freq = freq;
            self.lowpass.update(freq, self.sample_rate, LP_Q);
        }
    }

    /// Enable/disable the Enhance EQ
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Get current low-mid frequency
    pub fn get_lowmid_freq(&self) -> f32 {
        self.lowmid_freq
    }

    /// Get current low-mid gain
    pub fn get_lowmid_gain(&self) -> f32 {
        self.lowmid_gain_db
    }

    /// Get current presence frequency
    pub fn get_presence_freq(&self) -> f32 {
        self.presence_freq
    }

    /// Get current presence gain
    pub fn get_presence_gain(&self) -> f32 {
        self.presence_gain_db
    }

    /// Get current shelf frequency
    pub fn get_shelf_freq(&self) -> f32 {
        self.shelf_freq
    }

    /// Get current shelf max gain
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

        // 1. Low-mid cut (bell filter)
        let (_, bp_lowmid, _) = self.lowmid.process(input);
        let lowmid_gain = 10.0f32.powf(self.lowmid_gain_db / 20.0);
        let with_lowmid = input + (lowmid_gain - 1.0) * bp_lowmid;

        // 2. Presence boost (bell filter)
        let (_, bp_presence, _) = self.presence.process(with_lowmid);
        let presence_gain = 10.0f32.powf(self.presence_gain_db / 20.0);
        let with_presence = with_lowmid + (presence_gain - 1.0) * bp_presence;

        // 3. Envelope follower for dynamic air
        let abs_in = with_presence.abs();
        let coeff = if abs_in > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };
        self.envelope += coeff * (abs_in - self.envelope);

        // Calculate dynamic gain based on envelope
        let env_db = 20.0 * self.envelope.max(1e-10).log10();
        // Ramp from 0 to max gain over 20dB above threshold
        let gain_factor = ((env_db - self.threshold_db) / 20.0).clamp(0.0, 1.0);
        let dynamic_gain = self.shelf_gain_db * gain_factor;

        // 4. Apply dynamic air shelf
        self.shelf.set_params(self.shelf_freq, dynamic_gain);
        let with_air = self.shelf.process(with_presence);

        // 5. LP rolloff
        let (lp, _, _) = self.lowpass.process(with_air);
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
        self.lowmid.reset();
        self.presence.reset();
        self.shelf.reset();
        self.lowpass.reset();
        self.envelope = 0.0;
    }
}

impl crate::traits::AudioProcessor for EnhanceEq {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for EnhanceEq {}

impl crate::traits::Processor for EnhanceEq {
    fn new(sample_rate: f32) -> Self {
        Self::new(sample_rate)
    }
}

/// Stereo Enhance EQ processor
#[derive(Clone, Debug)]
pub struct StereoEnhanceEq {
    pub left: EnhanceEq,
    pub right: EnhanceEq,
}

impl StereoEnhanceEq {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            left: EnhanceEq::new(sample_rate),
            right: EnhanceEq::new(sample_rate),
        }
    }

    pub fn set_lowmid_freq(&mut self, freq: f32) {
        self.left.set_lowmid_freq(freq);
        self.right.set_lowmid_freq(freq);
    }

    pub fn set_lowmid_gain(&mut self, gain_db: f32) {
        self.left.set_lowmid_gain(gain_db);
        self.right.set_lowmid_gain(gain_db);
    }

    pub fn set_presence_freq(&mut self, freq: f32) {
        self.left.set_presence_freq(freq);
        self.right.set_presence_freq(freq);
    }

    pub fn set_presence_gain(&mut self, gain_db: f32) {
        self.left.set_presence_gain(gain_db);
        self.right.set_presence_gain(gain_db);
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

    /// Configure from spectral analysis
    /// Analyzes tilt and band energies to auto-configure all three bands
    pub fn configure_from_spectrum(&mut self, spectrum: &crate::analysis::SpectralAnalysis) {
        // Get spectral tilt (positive = bright, negative = dark)
        let tilt = spectrum.tilt_db();

        // Get band energies
        let lowmid_energy = spectrum.band_energy_db(200.0, 400.0);
        let presence_energy = spectrum.band_energy_db(2000.0, 5000.0);
        let air_energy = spectrum.band_energy_db(8000.0, 12000.0);
        let mid_energy = spectrum.band_energy_db(500.0, 2000.0);

        // === Low-mid band (250Hz) ===
        // Dark recordings (negative tilt) usually have too much low-mid
        // Energy-based: higher energy = more cut needed
        let lowmid_excess = (lowmid_energy - mid_energy).max(0.0); // How much louder than mids
        let tilt_factor = (-tilt / 10.0).clamp(0.0, 1.0); // Dark = more cut

        // Gain: -1 to -4dB based on excess and tilt
        let lowmid_gain: f32 = -1.0 - (lowmid_excess / 5.0 + tilt_factor * 2.0).min(3.0);

        // Q: larger cuts get narrower Q (1.2 to 2.0)
        let lowmid_q = 1.2 + (lowmid_gain.abs() / 4.0) * 0.8;

        // === Presence band (3kHz) ===
        // Bright recordings may already have enough presence
        // Dark recordings need more boost
        let presence_relative = presence_energy - mid_energy;

        let (presence_gain, presence_q) = if presence_relative > 3.0 {
            // Already a peak here - slight cut
            let cut = (presence_relative / 5.0).min(2.0);
            (-cut, 1.5 + cut * 0.25) // Narrower Q for cut
        } else if presence_relative < -3.0 {
            // Deficient - boost more
            let boost = ((-presence_relative) / 5.0).min(3.0);
            (boost, 1.0) // Wider Q for boost
        } else {
            // Neutral - subtle boost
            (1.0, 1.2)
        };

        // === Air band (dynamic shelf) ===
        // Compare air to mid energy
        let air_relative = air_energy - mid_energy;

        let air_gain = if air_relative < -15.0 {
            4.0 // Very deficient highs
        } else if air_relative < -8.0 {
            3.0 // Deficient
        } else if air_relative < -3.0 {
            2.0 // Slightly deficient
        } else {
            1.0 // Already bright enough
        };

        // Apply settings
        self.set_lowmid_gain(lowmid_gain);
        self.left.lowmid.update(250.0, self.left.sample_rate, lowmid_q);
        self.right.lowmid.update(250.0, self.right.sample_rate, lowmid_q);

        self.set_presence_gain(presence_gain);
        self.left.presence.update(3000.0, self.left.sample_rate, presence_q);
        self.right.presence.update(3000.0, self.right.sample_rate, presence_q);

        self.set_shelf_gain(air_gain);
    }

    /// Get current lowmid gain (for display)
    pub fn get_lowmid_gain(&self) -> f32 {
        self.left.get_lowmid_gain()
    }

    /// Get current presence gain (for display)
    pub fn get_presence_gain(&self) -> f32 {
        self.left.get_presence_gain()
    }

    /// Get current air/shelf gain (for display)
    pub fn get_shelf_gain(&self) -> f32 {
        self.left.get_shelf_gain()
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

impl crate::traits::StereoProcessor for StereoEnhanceEq {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        StereoEnhanceEq::process_stereo(self, left, right)
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn latency_samples(&self) -> usize {
        0 // EnhanceEq has no lookahead, zero latency
    }
}
