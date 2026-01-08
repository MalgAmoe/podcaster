//! FixEq - Dynamic EQ processor for podcast enhancement
//!
//! Applied after denoising to fix tonal issues:
//! - De-mud (300Hz) - reduces proximity effect / boominess
//! - Future: de-harsh, de-boom, presence boost, etc.
#![cfg_attr(all(feature = "cli", feature = "plugin"), allow(dead_code))]

use crate::filters::dynamic::DynamicBand;

/// FixEq processor containing all dynamic EQ bands.
/// Applied after denoising to clean up tonal issues.
#[derive(Clone, Debug)]
pub struct FixEq {
    sample_rate: f32,

    // Dynamic EQ bands
    demud: Option<DynamicBand>,
    // Future bands:
    // deharsh: Option<DynamicBand>,
    // deboom: Option<DynamicBand>,
    // presence: Option<DynamicBand>,
}

impl FixEq {
    /// Create a new FixEq processor
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            demud: None,
        }
    }

    /// Process a single sample through all enabled bands
    pub fn process(&mut self, input: f32, demud_strength: f32) -> f32 {
        let mut output = input;

        // De-mud (300Hz)
        if let Some(demud) = &mut self.demud {
            output = demud.process(output, demud_strength);
        }

        // Future bands would be chained here

        output
    }

    /// Set de-mud enabled state
    pub fn set_demud_enabled(&mut self, enabled: bool) {
        match (enabled, self.demud.is_some()) {
            (true, false) => self.demud = Some(DynamicBand::new_demud(self.sample_rate)),
            (false, true) => self.demud = None,
            _ => {}
        }
    }

    /// Get de-mud gain reduction in dB (0.0 if disabled)
    pub fn get_demud_gain_db(&self) -> f32 {
        self.demud.as_ref().map_or(0.0, |d| d.get_gain_reduction_db())
    }

    /// Reset all bands
    pub fn reset(&mut self) {
        if let Some(demud) = &mut self.demud {
            demud.reset();
        }
    }

    /// Update sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if (self.sample_rate - sample_rate).abs() > 0.1 {
            self.sample_rate = sample_rate;
            if self.demud.is_some() {
                self.demud = Some(DynamicBand::new_demud(sample_rate));
            }
        }
    }
}
