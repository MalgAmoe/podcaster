//! Radio Voice EQ - Biquad filter cascade
//!
//! Applies the fitted filter parameters as a cascade of biquad filters:
//! - 24 dB/oct HPF (2 cascaded 12 dB/oct)
//! - Low shelf for bass boost
//! - Mud peaking EQ
//! - Presence peaking EQ
//! - Air high shelf

use super::filters::{HighShelfSvf, LowShelfSvf, PeakingEqSvf, SvfHighPass};
use super::radio_fitter::RadioFilterParams;

// =============================================================================
// Radio EQ - Complete Filter Cascade
// =============================================================================

/// Radio Voice EQ filter cascade
///
/// Applies HPF, low shelf boost, mud cut, presence boost, and air shelf
/// for the hyped "smiley curve" radio sound.
#[derive(Clone, Debug)]
pub struct RadioEq {
    // 24 dB/oct HPF (2 cascaded 12 dB/oct stages)
    hpf_stage1: SvfHighPass,
    hpf_stage2: SvfHighPass,

    // Low shelf for bass boost
    low_shelf: LowShelfSvf,

    // Mud band peaking EQ
    mud_eq: PeakingEqSvf,

    // Presence band peaking EQ
    presence_eq: PeakingEqSvf,

    // Air high shelf
    air_shelf: HighShelfSvf,

    // Current parameters (for reporting)
    params: RadioFilterParams,
}

impl RadioEq {
    /// Create a new Radio EQ with default parameters
    pub fn new(sample_rate: f32) -> Self {
        Self {
            hpf_stage1: SvfHighPass::new(80.0, sample_rate),
            hpf_stage2: SvfHighPass::new(80.0, sample_rate),
            low_shelf: LowShelfSvf::new(120.0, 0.0, sample_rate),
            mud_eq: PeakingEqSvf::new(400.0, 1.0, 0.0, sample_rate),
            presence_eq: PeakingEqSvf::new(4000.0, 1.0, 0.0, sample_rate),
            air_shelf: HighShelfSvf::new(8000.0, 0.0, sample_rate),
            params: RadioFilterParams::default(),
        }
    }

    /// Configure filters from fitted parameters
    pub fn configure(&mut self, params: &RadioFilterParams) {
        self.params = params.clone();

        // HPF
        self.hpf_stage1.set_freq(params.hpf_freq);
        self.hpf_stage2.set_freq(params.hpf_freq);

        // Low shelf (bass boost)
        self.low_shelf.set_params(params.low_shelf_freq, params.low_shelf_gain);

        // Mud EQ
        self.mud_eq.set_params(params.mud_freq, params.mud_q, params.mud_gain);

        // Presence EQ
        self.presence_eq.set_params(params.presence_freq, params.presence_q, params.presence_gain);

        // Air shelf
        self.air_shelf.set_params(8000.0, params.air_gain);
    }

    /// Process a single sample through the filter cascade
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        // HPF (24 dB/oct) - remove rumble
        let mut sample = self.hpf_stage1.process(input);
        sample = self.hpf_stage2.process(sample);

        // Low shelf boost (bass)
        sample = self.low_shelf.process(sample);

        // Mud cut
        sample = self.mud_eq.process(sample);

        // Presence boost
        sample = self.presence_eq.process(sample);

        // Air shelf
        sample = self.air_shelf.process(sample);

        sample
    }

    /// Process a buffer of samples
    pub fn process(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process_sample(*sample);
        }
    }

    /// Reset all filter states
    pub fn reset(&mut self) {
        self.hpf_stage1.reset();
        self.hpf_stage2.reset();
        self.low_shelf.reset();
        self.mud_eq.reset();
        self.presence_eq.reset();
        self.air_shelf.reset();
    }

    // Getters for reporting
    pub fn get_hpf_freq(&self) -> f32 {
        self.params.hpf_freq
    }

    pub fn get_low_shelf_freq(&self) -> f32 {
        self.params.low_shelf_freq
    }

    pub fn get_low_shelf_gain(&self) -> f32 {
        self.params.low_shelf_gain
    }

    pub fn get_mud_freq(&self) -> f32 {
        self.params.mud_freq
    }

    pub fn get_mud_gain(&self) -> f32 {
        self.params.mud_gain
    }

    pub fn get_presence_freq(&self) -> f32 {
        self.params.presence_freq
    }

    pub fn get_presence_gain(&self) -> f32 {
        self.params.presence_gain
    }

    pub fn get_air_gain(&self) -> f32 {
        self.params.air_gain
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;
    use super::*;

    #[test]
    fn test_radio_eq_creation() {
        let eq = RadioEq::new(48000.0);
        assert_eq!(eq.params.hpf_freq, 60.0); // Default from RadioFilterParams
    }

    #[test]
    fn test_bypass_with_zero_gains() {
        let mut eq = RadioEq::new(48000.0);

        // Configure with zero gains
        let params = RadioFilterParams {
            hpf_freq: 20.0, // Very low, effectively bypass
            low_shelf_freq: 120.0,
            low_shelf_gain: 0.0,
            mud_freq: 350.0,
            mud_gain: 0.0,
            mud_q: 1.0,
            presence_freq: 3500.0,
            presence_gain: 0.0,
            presence_q: 1.0,
            air_gain: 0.0,
        };
        eq.configure(&params);

        // Process a 1kHz sine wave
        let freq = 1000.0;
        let mut samples: Vec<f32> = (0..4800)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0 * PI * freq * t).sin()
            })
            .collect();

        let original = samples.clone();
        eq.process(&mut samples);

        // With zero gains and very low HPF, output should be similar to input
        // (after initial filter settling)
        let tail_original = &original[2000..];
        let tail_processed = &samples[2000..];

        let max_diff: f32 = tail_original
            .iter()
            .zip(tail_processed.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);

        assert!(max_diff < 0.1, "Max difference should be small: {}", max_diff);
    }

    #[test]
    fn test_mud_cut() {
        let mut eq = RadioEq::new(48000.0);

        // Configure with significant mud cut
        let params = RadioFilterParams {
            hpf_freq: 20.0,
            low_shelf_freq: 120.0,
            low_shelf_gain: 0.0,
            mud_freq: 350.0,
            mud_gain: -6.0,
            mud_q: 1.0,
            presence_freq: 3500.0,
            presence_gain: 0.0,
            presence_q: 1.0,
            air_gain: 0.0,
        };
        eq.configure(&params);

        // Generate 350 Hz tone (in mud band)
        let mut samples: Vec<f32> = (0..4800)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0 * PI * 350.0 * t).sin()
            })
            .collect();

        let original_rms: f32 = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
        eq.process(&mut samples);
        let processed_rms: f32 = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();

        // 350 Hz should be reduced
        assert!(processed_rms < original_rms * 0.8, "350 Hz should be cut");
    }
}
