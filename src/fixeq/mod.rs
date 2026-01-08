//! FixEq - Dynamic EQ processor for podcast enhancement
//!
//! Applied after denoising to fix tonal issues:
//! - De-mud (150-500Hz) - reduces proximity effect / boominess
//! - De-esser (4-10kHz) - reduces sibilance
#![cfg_attr(all(feature = "cli", feature = "plugin"), allow(dead_code))]

pub mod analysis;

#[allow(unused_imports)]
pub use analysis::{
    analyze_audio, analyze_mud_frequency, mix_to_mono, FixEqAnalysis, MudAnalysis,
    SibilanceAnalysis,
};

use crate::filters::dynamic::DynamicBand;

/// FixEq processor containing all dynamic EQ bands.
/// Handles stereo/mono internally.
#[derive(Clone, Debug)]
pub struct FixEq {
    sample_rate: f32,

    // Left channel bands
    demud_left: Option<DynamicBand>,
    deesser_left: Option<DynamicBand>,

    // Right channel bands (None for mono)
    demud_right: Option<DynamicBand>,
    deesser_right: Option<DynamicBand>,

    // Computed strengths (from analysis + preset)
    demud_strength: f32,
    deesser_strength: f32,

    // Last analysis results (for display)
    last_analysis: Option<FixEqAnalysis>,

    is_stereo: bool,
}

impl FixEq {
    /// Create a new FixEq processor
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            demud_left: None,
            deesser_left: None,
            demud_right: None,
            deesser_right: None,
            demud_strength: 0.0,
            deesser_strength: 0.0,
            last_analysis: None,
            is_stereo: false,
        }
    }

    /// Analyze audio and configure all bands based on preset.
    /// Handles stereo (2 channels) or mono (1 channel) automatically.
    pub fn configure(&mut self, samples: &[Vec<f32>], preset: usize) -> &FixEqAnalysis {
        self.is_stereo = samples.len() >= 2;

        // Mix to mono for analysis
        let mono = if self.is_stereo {
            mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };

        let analysis = analyze_audio(&mono, self.sample_rate as u32);

        // Calculate strengths from preset + analysis
        let preset_factor = preset as f32 / 5.0; // 1=0.2, 5=1.0

        // De-mud strength
        let mud_energy_factor = ((analysis.mud.energy_db + 40.0) / 30.0).clamp(0.0, 1.0);
        self.demud_strength =
            (preset_factor * analysis.mud.confidence * mud_energy_factor).clamp(0.0, 1.0);

        // De-esser strength - sibilance is always present in speech, use energy-based approach
        // with minimum confidence floor (don't require sibilance to "stand out")
        let sib_energy_factor = ((analysis.sibilance.energy_db + 40.0) / 30.0).clamp(0.0, 1.0);
        let sib_confidence = analysis.sibilance.confidence.max(0.5); // minimum 50% for speech
        self.deesser_strength =
            (preset_factor * sib_confidence * sib_energy_factor).clamp(0.0, 1.0);

        // Configure left channel bands
        self.demud_left = Some(DynamicBand::new_demud_at(
            analysis.mud.center_freq,
            self.sample_rate,
        ));
        self.deesser_left = Some(DynamicBand::new_deesser_at(
            analysis.sibilance.center_freq,
            self.sample_rate,
        ));

        // Configure right channel bands (if stereo)
        if self.is_stereo {
            self.demud_right = Some(DynamicBand::new_demud_at(
                analysis.mud.center_freq,
                self.sample_rate,
            ));
            self.deesser_right = Some(DynamicBand::new_deesser_at(
                analysis.sibilance.center_freq,
                self.sample_rate,
            ));
        }

        self.last_analysis = Some(analysis);
        self.last_analysis.as_ref().unwrap()
    }

    /// Process mono audio in-place
    pub fn process_mono(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.process_sample_left(*sample);
        }
    }

    /// Process stereo audio in-place
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            *l = self.process_sample_left(*l);
            *r = self.process_sample_right(*r);
        }
    }

    /// Process a single left channel sample
    fn process_sample_left(&mut self, input: f32) -> f32 {
        let mut output = input;

        if let Some(demud) = &mut self.demud_left {
            output = demud.process(output, self.demud_strength);
        }

        if let Some(deesser) = &mut self.deesser_left {
            output = deesser.process(output, self.deesser_strength);
        }

        output
    }

    /// Process a single right channel sample
    fn process_sample_right(&mut self, input: f32) -> f32 {
        let mut output = input;

        if let Some(demud) = &mut self.demud_right {
            output = demud.process(output, self.demud_strength);
        }

        if let Some(deesser) = &mut self.deesser_right {
            output = deesser.process(output, self.deesser_strength);
        }

        output
    }

    /// Get the last analysis results
    pub fn get_analysis(&self) -> Option<&FixEqAnalysis> {
        self.last_analysis.as_ref()
    }

    /// Get computed de-mud strength (0-1)
    pub fn get_demud_strength(&self) -> f32 {
        self.demud_strength
    }

    /// Get computed de-esser strength (0-1)
    pub fn get_deesser_strength(&self) -> f32 {
        self.deesser_strength
    }

    /// Get de-mud gain reduction in dB (from left channel)
    pub fn get_demud_gain_db(&self) -> f32 {
        self.demud_left
            .as_ref()
            .map_or(0.0, |d| d.get_gain_reduction_db())
    }

    /// Get de-esser gain reduction in dB (from left channel)
    pub fn get_deesser_gain_db(&self) -> f32 {
        self.deesser_left
            .as_ref()
            .map_or(0.0, |d| d.get_gain_reduction_db())
    }

    // =========================================================================
    // Manual control methods (for plugin realtime use)
    // =========================================================================

    /// Enable/disable de-mud with default frequency (for plugin)
    pub fn set_demud_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.demud_left.is_none() {
                self.demud_left = Some(DynamicBand::new_demud(self.sample_rate));
            }
            if self.is_stereo && self.demud_right.is_none() {
                self.demud_right = Some(DynamicBand::new_demud(self.sample_rate));
            }
        } else {
            self.demud_left = None;
            self.demud_right = None;
        }
    }

    /// Enable/disable de-esser with default frequency (for plugin)
    pub fn set_deesser_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.deesser_left.is_none() {
                self.deesser_left = Some(DynamicBand::new_deesser(self.sample_rate));
            }
            if self.is_stereo && self.deesser_right.is_none() {
                self.deesser_right = Some(DynamicBand::new_deesser(self.sample_rate));
            }
        } else {
            self.deesser_left = None;
            self.deesser_right = None;
        }
    }

    /// Set de-mud strength directly (for plugin)
    pub fn set_demud_strength(&mut self, strength: f32) {
        self.demud_strength = strength.clamp(0.0, 1.0);
    }

    /// Set de-esser strength directly (for plugin)
    pub fn set_deesser_strength(&mut self, strength: f32) {
        self.deesser_strength = strength.clamp(0.0, 1.0);
    }

    /// Set stereo mode (for plugin)
    pub fn set_stereo(&mut self, is_stereo: bool) {
        if self.is_stereo != is_stereo {
            self.is_stereo = is_stereo;
            // Reset right channel if switching to mono
            if !is_stereo {
                self.demud_right = None;
                self.deesser_right = None;
            }
        }
    }

    /// Process a single sample (for plugin - uses left channel processing)
    pub fn process(&mut self, input: f32) -> f32 {
        self.process_sample_left(input)
    }

    /// Reset all bands
    pub fn reset(&mut self) {
        if let Some(demud) = &mut self.demud_left {
            demud.reset();
        }
        if let Some(deesser) = &mut self.deesser_left {
            deesser.reset();
        }
        if let Some(demud) = &mut self.demud_right {
            demud.reset();
        }
        if let Some(deesser) = &mut self.deesser_right {
            deesser.reset();
        }
    }

    /// Update sample rate (resets configuration)
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (self.sample_rate - sample_rate).abs() > 0.1 {
            self.sample_rate = sample_rate;
            self.demud_left = None;
            self.deesser_left = None;
            self.demud_right = None;
            self.deesser_right = None;
            self.demud_strength = 0.0;
            self.deesser_strength = 0.0;
            self.last_analysis = None;
        }
    }
}
