//! De-Esser - Dynamic sibilance reduction
//!
//! Applied after FixEq to reduce harsh "ess" sounds in speech.
//! Uses wider bandwidth than FixEq because sibilance is broadband noise-like.

#![allow(dead_code)]

pub mod analysis;

pub use analysis::{analyze_sibilance, mix_to_mono, SibilanceAnalysis, DEFAULT_SIBILANCE_FREQ};

use crate::filters::dynamic::DynamicBand;

const D_ATTACK: f32 = 0.2;
const D_RELEASE: f32 = 0.130;
const D_THRESHOLD: f32 = -3.0;
const D_RATIO: f32 = 9.4;
const D_MAX_CUT: f32 = -18.0;

/// De-Esser processor for a single channel
#[derive(Clone, Debug)]
pub struct DeEsser {
    band: DynamicBand,
    center_freq: f32,
    q: f32,
    strength: f32,
    sample_rate: f32,
}

impl DeEsser {
    /// Create a new de-esser with given parameters
    pub fn new(center_freq: f32, q: f32, sample_rate: f32) -> Self {
        // For de-esser: same Q for detection and processing
        // (unlike FixEq which uses wider detection Q)
        let band = DynamicBand::new(center_freq, q, q, sample_rate, D_ATTACK, D_RELEASE)
            .with_threshold(D_THRESHOLD)
            .with_ratio(D_RATIO)
            .with_max_cut(D_MAX_CUT);

        Self {
            band,
            center_freq,
            q,
            strength: 1.0,
            sample_rate,
        }
    }

    /// Create with default sibilance frequency
    pub fn new_default(sample_rate: f32) -> Self {
        Self::new(DEFAULT_SIBILANCE_FREQ, 1.5, sample_rate)
    }

    /// Set center frequency (recreates internal band)
    pub fn set_frequency(&mut self, freq: f32) {
        if (self.center_freq - freq).abs() > 0.1 {
            self.center_freq = freq;
            self.band = DynamicBand::new(freq, self.q, self.q, self.sample_rate,D_ATTACK, D_RELEASE)
            .with_threshold(D_THRESHOLD)
            .with_ratio(D_RATIO)
            .with_max_cut(D_MAX_CUT);
        }
    }

    /// Set Q factor (recreates internal band)
    pub fn set_q(&mut self, q: f32) {
        let q = q.clamp(0.7, 2.5);
        if (self.q - q).abs() > 0.01 {
            self.q = q;
            self.band = DynamicBand::new(self.center_freq, q, q, self.sample_rate,D_ATTACK, D_RELEASE)
            .with_threshold(D_THRESHOLD)
            .with_ratio(D_RATIO)
            .with_max_cut(D_MAX_CUT);
        }
    }

    /// Set processing strength (0.0 = bypass, 1.0 = full)
    pub fn set_strength(&mut self, strength: f32) {
        self.strength = strength.clamp(0.0, 1.0);
    }

    /// Process a single sample
    pub fn process(&mut self, input: f32) -> f32 {
        self.band.process(input, self.strength)
    }

    /// Get current gain reduction in dB
    pub fn get_gain_reduction_db(&self) -> f32 {
        self.band.get_gain_reduction_db()
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.band.reset();
    }
}

/// Stereo de-esser with analysis-based configuration
#[derive(Clone, Debug)]
pub struct StereoDeEsser {
    left: DeEsser,
    right: DeEsser,
    is_stereo: bool,
    sample_rate: f32,
    last_analysis: Option<SibilanceAnalysis>,
}

impl StereoDeEsser {
    /// Create a new stereo de-esser
    pub fn new(sample_rate: f32) -> Self {
        Self {
            left: DeEsser::new_default(sample_rate),
            right: DeEsser::new_default(sample_rate),
            is_stereo: false,
            sample_rate,
            last_analysis: None,
        }
    }

    /// Analyze audio and configure de-esser
    /// Returns the analysis result for display
    pub fn configure(&mut self, samples: &[Vec<f32>]) -> &SibilanceAnalysis {
        self.is_stereo = samples.len() >= 2;

        // Mix to mono for analysis
        let mono = if self.is_stereo {
            mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };

        let analysis = analyze_sibilance(&mono, self.sample_rate as u32);

        // Calculate adaptive Q from detected bandwidth
        let q = analysis::calculate_deesser_q(analysis.bandwidth_hz, analysis.center_freq);

        // Configure both channels
        self.left.set_frequency(analysis.center_freq);
        self.left.set_q(q);

        if self.is_stereo {
            self.right.set_frequency(analysis.center_freq);
            self.right.set_q(q);
        }

        // Scale strength by confidence AND energy (like FixEq)
        // Energy factor: map energy_db from [-60, -20] to [0, 1]
        let energy_factor = ((analysis.energy_db + 60.0) / 40.0).clamp(0.0, 1.0);
        let strength = (analysis.confidence * energy_factor).clamp(0.0, 1.0);
        self.left.set_strength(strength);
        self.right.set_strength(strength);

        self.last_analysis = Some(analysis);
        self.last_analysis.as_ref().unwrap()
    }

    /// Get computed strength (for display)
    pub fn get_strength(&self) -> f32 {
        // Return the strength that was set during configure
        // We can compute it from the last analysis
        if let Some(analysis) = &self.last_analysis {
            let energy_factor = ((analysis.energy_db + 60.0) / 40.0).clamp(0.0, 1.0);
            (analysis.confidence * energy_factor).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// Process stereo audio in-place
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l = self.left.process(*l);
            *r = self.right.process(*r);
        }
    }

    /// Process mono audio in-place
    pub fn process_mono(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.left.process(*sample);
        }
    }

    /// Get last analysis result
    pub fn get_analysis(&self) -> Option<&SibilanceAnalysis> {
        self.last_analysis.as_ref()
    }

    /// Get gain reduction from left channel (for metering)
    pub fn get_gain_reduction_db(&self) -> f32 {
        self.left.get_gain_reduction_db()
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}
