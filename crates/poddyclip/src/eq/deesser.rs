//! De-Esser - Dynamic sibilance reduction
//!
//! Applied after FixEq to reduce harsh "ess" sounds in speech.
//! Uses flat band-cut filter because sibilance is broadband noise-like.

#![allow(dead_code)]

use super::deesser_analysis::{analyze_sibilance, SibilanceAnalysis, DEFAULT_SIBILANCE_FREQ};
use super::dynamic::DynamicBandCut;
use crate::analysis::utils::mix_to_mono;

const D_ATTACK: f32 = 0.2;
const D_RELEASE: f32 = 130.0;
const D_THRESHOLD: f32 = -3.0;
const D_RATIO: f32 = 9.4;
const D_MAX_CUT: f32 = -18.0;

/// Default sibilance band (Hz)
const DEFAULT_START_FREQ: f32 = DEFAULT_SIBILANCE_FREQ - 1500.0; // 5000 Hz
const DEFAULT_STOP_FREQ: f32 = DEFAULT_SIBILANCE_FREQ + 1500.0;  // 8000 Hz

/// De-Esser processor for a single channel
/// Uses flat band-cut filter (not resonant) for broadband sibilance reduction
#[derive(Clone, Debug)]
pub struct DeEsser {
    band: DynamicBandCut,
    start_freq: f32,
    stop_freq: f32,
    strength: f32,
    sample_rate: f32,
}

impl DeEsser {
    /// Create a new de-esser with given band frequencies
    pub fn new(start_freq: f32, stop_freq: f32, sample_rate: f32) -> Self {
        let band = DynamicBandCut::new(start_freq, stop_freq, sample_rate, D_ATTACK, D_RELEASE)
            .with_threshold(D_THRESHOLD)
            .with_ratio(D_RATIO)
            .with_max_cut(D_MAX_CUT);

        Self {
            band,
            start_freq,
            stop_freq,
            strength: 1.0,
            sample_rate,
        }
    }

    /// Create with default sibilance band
    pub fn new_default(sample_rate: f32) -> Self {
        Self::new(DEFAULT_START_FREQ, DEFAULT_STOP_FREQ, sample_rate)
    }

    /// Set band frequencies
    pub fn set_band(&mut self, start_freq: f32, stop_freq: f32) {
        let changed = (self.start_freq - start_freq).abs() > 0.1
            || (self.stop_freq - stop_freq).abs() > 0.1;

        if changed {
            self.start_freq = start_freq;
            self.stop_freq = stop_freq;
            self.band.set_band(start_freq, stop_freq);
        }
    }

    /// Set center frequency (backward-compatible method)
    /// Calculates start/stop based on current bandwidth
    pub fn set_frequency(&mut self, center_freq: f32) {
        let current_bandwidth = self.stop_freq - self.start_freq;
        let half_bw = current_bandwidth / 2.0;
        self.set_band(center_freq - half_bw, center_freq + half_bw);
    }

    /// Set Q factor (backward-compatible method)
    /// Q affects bandwidth: bandwidth = center_freq / Q
    /// Lower Q = wider band, higher Q = narrower band
    pub fn set_q(&mut self, q: f32) {
        let q = q.clamp(0.5, 3.0);
        let center_freq = (self.start_freq + self.stop_freq) / 2.0;
        let bandwidth = center_freq / q;
        let half_bw = bandwidth / 2.0;
        self.set_band(center_freq - half_bw, center_freq + half_bw);
    }

    /// Set processing strength (0.0 = bypass, 1.0 = full)
    pub fn set_strength(&mut self, strength: f32) {
        self.strength = strength.clamp(0.0, 1.0);
    }

    /// Get current start frequency
    pub fn get_start_freq(&self) -> f32 {
        self.start_freq
    }

    /// Get current stop frequency
    pub fn get_stop_freq(&self) -> f32 {
        self.stop_freq
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

impl crate::traits::AudioProcessor for DeEsser {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for DeEsser {}

impl crate::traits::Processor for DeEsser {
    fn new(sample_rate: f32) -> Self {
        Self::new_default(sample_rate)
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

        // Sibilance analysis includes f0 detection and harmonic checking
        // Returns start_freq and stop_freq for flat band-cut
        let analysis = analyze_sibilance(&mono, self.sample_rate as u32);

        // Configure both channels with detected band
        self.left.set_band(analysis.start_freq, analysis.stop_freq);

        if self.is_stereo {
            self.right.set_band(analysis.start_freq, analysis.stop_freq);
        }

        // Scale strength by confidence and energy
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
        // Confidence already includes harmonic adjustment from analysis
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

impl crate::traits::StereoProcessor for StereoDeEsser {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        StereoDeEsser::process_stereo(self, left, right)
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn latency_samples(&self) -> usize {
        0 // DeEsser has no lookahead, zero latency
    }
}
