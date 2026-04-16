//! FixEq - Dynamic EQ processor for podcast enhancement
//!
//! Applied after denoising to fix tonal issues:
//! - De-mud (150-500Hz) - reduces proximity effect / boominess
//! - Correction bands (500-5000Hz) - tames resonances

#![allow(dead_code)]

use super::dynamic::DynamicBand;
use super::fixeq_analysis::{analyze_from_spectrum, FixEqAnalysis};

/// FixEq processor containing all dynamic EQ bands.
/// Handles stereo/mono internally.
#[derive(Clone, Debug)]
pub struct FixEq {
    sample_rate: f32,

    // Left channel bands
    demud_left: Option<DynamicBand>,
    correction_a_left: Option<DynamicBand>,
    correction_b_left: Option<DynamicBand>,

    // Right channel bands (None for mono)
    demud_right: Option<DynamicBand>,
    correction_a_right: Option<DynamicBand>,
    correction_b_right: Option<DynamicBand>,

    // Current frequencies (to avoid recreating bands unnecessarily)
    demud_freq: f32,
    correction_a_freq: f32,
    correction_b_freq: f32,

    // Computed strengths (from analysis + preset)
    demud_strength: f32,
    correction_a_strength: f32,
    correction_b_strength: f32,

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
            correction_a_left: None,
            correction_b_left: None,
            demud_right: None,
            correction_a_right: None,
            correction_b_right: None,
            demud_freq: 300.0,
            correction_a_freq: 1000.0,
            correction_b_freq: 3000.0,
            demud_strength: 0.0,
            correction_a_strength: 0.0,
            correction_b_strength: 0.0,
            last_analysis: None,
            is_stereo: false,
        }
    }

    /// Configure from pre-computed spectral analysis
    pub fn configure_from_spectrum(
        &mut self,
        spectrum: &crate::analysis::SpectralAnalysis,
        preset: usize,
        is_stereo: bool,
    ) -> &FixEqAnalysis {
        self.is_stereo = is_stereo;
        let analysis = analyze_from_spectrum(spectrum);
        self.apply_analysis(analysis, preset)
    }

    /// Apply analysis results and configure bands
    fn apply_analysis(&mut self, analysis: FixEqAnalysis, preset: usize) -> &FixEqAnalysis {
        // Calculate strengths from preset + analysis
        let preset_factor = preset as f32 / 5.0; // 1=0.2, 5=1.0

        // De-mud strength
        let mud_energy_factor = ((analysis.mud.energy_db + 40.0) / 30.0).clamp(0.0, 1.0);
        self.demud_strength =
            (preset_factor * analysis.mud.confidence * mud_energy_factor).clamp(0.0, 1.0);

        // Correction A strength
        let corr_a_energy_factor =
            ((analysis.correction_a.energy_db + 40.0) / 30.0).clamp(0.0, 1.0);
        self.correction_a_strength =
            (preset_factor * analysis.correction_a.confidence * corr_a_energy_factor)
                .clamp(0.0, 1.0);

        // Correction B strength
        let corr_b_energy_factor =
            ((analysis.correction_b.energy_db + 40.0) / 30.0).clamp(0.0, 1.0);
        self.correction_b_strength =
            (preset_factor * analysis.correction_b.confidence * corr_b_energy_factor)
                .clamp(0.0, 1.0);

        // Configure left channel bands
        self.demud_left = Some(DynamicBand::new_demud_at(
            analysis.mud.center_freq,
            self.sample_rate,
        ));
        self.correction_a_left = Some(DynamicBand::new_correction_at(
            analysis.correction_a.center_freq,
            self.sample_rate,
        ));
        self.correction_b_left = Some(DynamicBand::new_correction_at(
            analysis.correction_b.center_freq,
            self.sample_rate,
        ));

        // Configure right channel bands (if stereo)
        if self.is_stereo {
            self.demud_right = Some(DynamicBand::new_demud_at(
                analysis.mud.center_freq,
                self.sample_rate,
            ));
            self.correction_a_right = Some(DynamicBand::new_correction_at(
                analysis.correction_a.center_freq,
                self.sample_rate,
            ));
            self.correction_b_right = Some(DynamicBand::new_correction_at(
                analysis.correction_b.center_freq,
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
        let len = left.len().min(right.len());
        for i in 0..len {
            left[i] = self.process_sample_left(left[i]);
            right[i] = self.process_sample_right(right[i]);
        }
    }

    /// Process a single left channel sample
    pub fn process_sample_left(&mut self, input: f32) -> f32 {
        let mut output = input;

        if let Some(demud) = &mut self.demud_left {
            output = demud.process(output, self.demud_strength);
        }

        if let Some(correction_a) = &mut self.correction_a_left {
            output = correction_a.process(output, self.correction_a_strength);
        }

        if let Some(correction_b) = &mut self.correction_b_left {
            output = correction_b.process(output, self.correction_b_strength);
        }

        output
    }

    /// Process a single right channel sample
    pub fn process_sample_right(&mut self, input: f32) -> f32 {
        let mut output = input;

        if let Some(demud) = &mut self.demud_right {
            output = demud.process(output, self.demud_strength);
        }

        if let Some(correction_a) = &mut self.correction_a_right {
            output = correction_a.process(output, self.correction_a_strength);
        }

        if let Some(correction_b) = &mut self.correction_b_right {
            output = correction_b.process(output, self.correction_b_strength);
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

    /// Get de-mud gain reduction in dB (from left channel)
    pub fn get_demud_gain_db(&self) -> f32 {
        self.demud_left
            .as_ref()
            .map_or(0.0, |d| d.get_gain_reduction_db())
    }

    /// Get computed correction A strength (0-1)
    pub fn get_correction_a_strength(&self) -> f32 {
        self.correction_a_strength
    }

    /// Get computed correction B strength (0-1)
    pub fn get_correction_b_strength(&self) -> f32 {
        self.correction_b_strength
    }

    /// Get correction A gain reduction in dB (from left channel)
    pub fn get_correction_a_gain_db(&self) -> f32 {
        self.correction_a_left
            .as_ref()
            .map_or(0.0, |d| d.get_gain_reduction_db())
    }

    /// Get correction B gain reduction in dB (from left channel)
    pub fn get_correction_b_gain_db(&self) -> f32 {
        self.correction_b_left
            .as_ref()
            .map_or(0.0, |d| d.get_gain_reduction_db())
    }

    // =========================================================================
    // Manual control methods (for real-time use)
    // =========================================================================

    /// Enable/disable de-mud (uses current frequency setting)
    pub fn set_demud_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.demud_left.is_none() {
                self.demud_left =
                    Some(DynamicBand::new_demud_at(self.demud_freq, self.sample_rate));
            }
            if self.is_stereo && self.demud_right.is_none() {
                self.demud_right =
                    Some(DynamicBand::new_demud_at(self.demud_freq, self.sample_rate));
            }
        } else {
            self.demud_left = None;
            self.demud_right = None;
        }
    }

    /// Enable/disable correction A (uses current frequency setting)
    pub fn set_correction_a_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.correction_a_left.is_none() {
                self.correction_a_left = Some(DynamicBand::new_correction_at(
                    self.correction_a_freq,
                    self.sample_rate,
                ));
            }
            if self.is_stereo && self.correction_a_right.is_none() {
                self.correction_a_right = Some(DynamicBand::new_correction_at(
                    self.correction_a_freq,
                    self.sample_rate,
                ));
            }
        } else {
            self.correction_a_left = None;
            self.correction_a_right = None;
        }
    }

    /// Enable/disable correction B (uses current frequency setting)
    pub fn set_correction_b_enabled(&mut self, enabled: bool) {
        if enabled {
            if self.correction_b_left.is_none() {
                self.correction_b_left = Some(DynamicBand::new_correction_at(
                    self.correction_b_freq,
                    self.sample_rate,
                ));
            }
            if self.is_stereo && self.correction_b_right.is_none() {
                self.correction_b_right = Some(DynamicBand::new_correction_at(
                    self.correction_b_freq,
                    self.sample_rate,
                ));
            }
        } else {
            self.correction_b_left = None;
            self.correction_b_right = None;
        }
    }

    /// Set de-mud strength directly
    pub fn set_demud_strength(&mut self, strength: f32) {
        self.demud_strength = strength.clamp(0.0, 1.0);
    }

    /// Set correction A strength directly
    pub fn set_correction_a_strength(&mut self, strength: f32) {
        self.correction_a_strength = strength.clamp(0.0, 1.0);
    }

    /// Set correction B strength directly
    pub fn set_correction_b_strength(&mut self, strength: f32) {
        self.correction_b_strength = strength.clamp(0.0, 1.0);
    }

    /// Set de-mud frequency (only recreates band if frequency changed)
    pub fn set_demud_frequency(&mut self, freq: f32) {
        if (self.demud_freq - freq).abs() < 0.1 {
            return; // No change
        }
        self.demud_freq = freq;
        if self.demud_left.is_some() {
            self.demud_left = Some(DynamicBand::new_demud_at(freq, self.sample_rate));
        }
        if self.demud_right.is_some() {
            self.demud_right = Some(DynamicBand::new_demud_at(freq, self.sample_rate));
        }
    }

    /// Set correction A frequency (only recreates band if frequency changed)
    pub fn set_correction_a_frequency(&mut self, freq: f32) {
        if (self.correction_a_freq - freq).abs() < 0.1 {
            return; // No change
        }
        self.correction_a_freq = freq;
        if self.correction_a_left.is_some() {
            self.correction_a_left = Some(DynamicBand::new_correction_at(freq, self.sample_rate));
        }
        if self.correction_a_right.is_some() {
            self.correction_a_right = Some(DynamicBand::new_correction_at(freq, self.sample_rate));
        }
    }

    /// Set correction B frequency (only recreates band if frequency changed)
    pub fn set_correction_b_frequency(&mut self, freq: f32) {
        if (self.correction_b_freq - freq).abs() < 0.1 {
            return; // No change
        }
        self.correction_b_freq = freq;
        if self.correction_b_left.is_some() {
            self.correction_b_left = Some(DynamicBand::new_correction_at(freq, self.sample_rate));
        }
        if self.correction_b_right.is_some() {
            self.correction_b_right = Some(DynamicBand::new_correction_at(freq, self.sample_rate));
        }
    }

    /// Set stereo mode
    pub fn set_stereo(&mut self, is_stereo: bool) {
        if self.is_stereo != is_stereo {
            self.is_stereo = is_stereo;
            // Reset right channel if switching to mono
            if !is_stereo {
                self.demud_right = None;
                self.correction_a_right = None;
                self.correction_b_right = None;
            }
        }
    }

    /// Process a single sample (uses left channel processing)
    pub fn process(&mut self, input: f32) -> f32 {
        self.process_sample_left(input)
    }

    /// Reset all bands
    pub fn reset(&mut self) {
        if let Some(demud) = &mut self.demud_left {
            demud.reset();
        }
        if let Some(correction_a) = &mut self.correction_a_left {
            correction_a.reset();
        }
        if let Some(correction_b) = &mut self.correction_b_left {
            correction_b.reset();
        }
        if let Some(demud) = &mut self.demud_right {
            demud.reset();
        }
        if let Some(correction_a) = &mut self.correction_a_right {
            correction_a.reset();
        }
        if let Some(correction_b) = &mut self.correction_b_right {
            correction_b.reset();
        }
    }

    /// Update sample rate (resets configuration)
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (self.sample_rate - sample_rate).abs() > 0.1 {
            self.sample_rate = sample_rate;
            self.demud_left = None;
            self.correction_a_left = None;
            self.correction_b_left = None;
            self.demud_right = None;
            self.correction_a_right = None;
            self.correction_b_right = None;
            self.demud_strength = 0.0;
            self.correction_a_strength = 0.0;
            self.correction_b_strength = 0.0;
            self.last_analysis = None;
        }
    }
}

impl crate::traits::AudioProcessor for FixEq {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for FixEq {}

impl crate::traits::Processor for FixEq {
    fn new(sample_rate: f32) -> Self {
        Self::new(sample_rate)
    }
}
