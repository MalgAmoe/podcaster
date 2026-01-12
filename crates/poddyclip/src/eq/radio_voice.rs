//! Radio Voice Processor - Automatic Broadcast EQ
//!
//! Orchestrates cepstral analysis, target generation, filter fitting,
//! and EQ application for achieving broadcast-quality voice.
//!
//! # Usage (CLI only - offline processing)
//!
//! ```rust,ignore
//! use poddyclip::eq::RadioVoiceProcessor;
//!
//! let mut radio = RadioVoiceProcessor::new(48000);
//! radio.set_amount(1.0); // Full correction
//!
//! // Analyze audio (mono mix for analysis)
//! radio.analyze(&mono_samples);
//!
//! // Process audio
//! radio.process(&mut samples);
//! ```

use crate::analysis::{CepstralAnalysis, SpectralAnalysis};
use crate::traits::{AudioProcessor, MonoProcessor, Processor, StereoProcessor};

use super::deesser_analysis::analyze_sibilance;
use super::radio_eq::RadioEq;
use super::radio_fitter::RadioFilterParams;
use super::radio_target::RadioTarget;

/// Radio Voice Processor - Automatic Broadcast EQ
///
/// Single-knob control that automatically analyzes audio and applies
/// corrective EQ to achieve a broadcast-quality voice sound.
#[derive(Clone)]
pub struct RadioVoiceProcessor {
    sample_rate: u32,
    amount: f32,

    // Processing
    eq: RadioEq,

    // Analysis results (for reporting)
    detected_f0: f32,
    sibilance_level: f32,
    cpp: f32,
    echo_delay_ms: Option<f32>,
    echo_strength: f32,
    params: RadioFilterParams,
    configured: bool,
}

impl RadioVoiceProcessor {
    /// Create a new Radio Voice Processor
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            amount: 1.0,
            eq: RadioEq::new(sample_rate as f32),
            detected_f0: 120.0, // Default f0
            sibilance_level: 0.0,
            cpp: 0.0,
            echo_delay_ms: None,
            echo_strength: 0.0,
            params: RadioFilterParams::default(),
            configured: false,
        }
    }

    /// Set the amount of correction (0.0 = bypass, 1.0 = full)
    pub fn set_amount(&mut self, amount: f32) {
        self.amount = amount.clamp(0.0, 1.0);
    }

    /// Analyze audio and configure filters
    ///
    /// Call this with a mono mix of the audio before processing.
    /// The analysis determines f0, sibilance level, and computes the optimal EQ settings.
    pub fn analyze(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }

        // Step 1: Spectral analysis (reuses existing implementation)
        let spectrum = SpectralAnalysis::new(samples, self.sample_rate);

        // Step 2: Cepstral analysis for f0, CPP, and echo detection
        // Note: We use octave-band averaged spectrum (not cepstral envelope) for EQ matching
        // because cepstral envelope removes spectral tilt, giving wrong slope measurements.
        let cepstral = CepstralAnalysis::from_spectrum(&spectrum);

        // Store cepstral analysis results
        self.detected_f0 = cepstral.f0.unwrap_or(120.0);
        self.cpp = cepstral.cpp;
        self.echo_delay_ms = cepstral.echo_delay_ms;
        self.echo_strength = cepstral.echo_strength;

        // Step 3: Sibilance analysis (reuses de-esser analysis)
        let sibilance = analyze_sibilance(samples, self.sample_rate);
        self.sibilance_level = sibilance.confidence;

        // Step 4: Generate target spectrum
        let target = RadioTarget::generate(
            self.detected_f0,
            spectrum.n_bins,
            self.sample_rate,
            spectrum.bin_freq,
        );

        // Step 5: Get octave-band averaged spectrum (like LTASS measurement)
        // This preserves the natural spectral slope unlike cepstral envelope
        let actual_envelope_db = spectrum.octave_band_envelope_db();

        // Step 6: Normalize actual envelope to target's scale using total power
        // Compare total energy across speech range (0.8*f0 to 8kHz)
        let min_freq = self.detected_f0 * 0.8;
        let max_freq = 8000.0;

        let min_bin = ((min_freq / spectrum.bin_freq).round() as usize).min(spectrum.n_bins - 1);
        let max_bin = ((max_freq / spectrum.bin_freq).round() as usize).min(spectrum.n_bins - 1);

        // Sum power in linear domain (power adds linearly, not in dB)
        let actual_power: f32 = actual_envelope_db[min_bin..=max_bin]
            .iter()
            .map(|&db| 10.0_f32.powf(db / 10.0))
            .sum();

        let target_power: f32 = target.curve_db[min_bin..=max_bin]
            .iter()
            .map(|&db| 10.0_f32.powf(db / 10.0))
            .sum();

        // Offset in dB to match total power
        let normalization_offset = if actual_power > 0.0 && target_power > 0.0 {
            10.0 * (target_power / actual_power).log10()
        } else {
            0.0
        };

        // Normalize actual envelope
        let normalized_envelope: Vec<f32> = actual_envelope_db
            .iter()
            .map(|&db| db + normalization_offset)
            .collect();

        // Step 6: Compute error spectrum (target - normalized actual)
        let error_db = target.compute_error(&normalized_envelope);

        // Step 7: Fit filter parameters from error (with sibilance awareness)
        self.params = RadioFilterParams::fit_from_error_with_sibilance(
            &error_db,
            self.detected_f0,
            spectrum.bin_freq,
            spectrum.n_bins,
            self.sibilance_level,
        );

        // Step 8: Scale by amount
        self.params.scale_by_amount(self.amount);

        // Step 9: Configure EQ filters
        self.eq.configure(&self.params);
        self.configured = true;
    }

    /// Process a buffer of samples through the EQ
    pub fn process(&mut self, buffer: &mut [f32]) {
        if !self.configured || self.amount < 0.01 {
            return;
        }
        self.eq.process(buffer);
    }

    /// Process stereo buffers
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        if !self.configured || self.amount < 0.01 {
            return;
        }

        // Process left channel
        self.eq.process(left);

        // Reset and process right channel with same settings
        // (EQ is configured with the same params, just need to reset state)
        self.eq.reset();
        self.eq.process(right);
    }

    /// Reset filter states
    pub fn reset(&mut self) {
        self.eq.reset();
    }

    // Getters for reporting
    pub fn get_detected_f0(&self) -> f32 {
        self.detected_f0
    }

    pub fn get_sibilance_level(&self) -> f32 {
        self.sibilance_level
    }

    /// Cepstral Peak Prominence (dB) - measure of harmonicity
    /// Higher = more harmonic/voiced (5-15 dB typical for speech)
    /// Lower = noise or unvoiced (0-3 dB)
    pub fn get_cpp(&self) -> f32 {
        self.cpp
    }

    /// Detected echo/reverb delay in milliseconds, None if no echo detected
    pub fn get_echo_delay_ms(&self) -> Option<f32> {
        self.echo_delay_ms
    }

    /// Echo strength (0.0 = none, 1.0 = strong echo)
    pub fn get_echo_strength(&self) -> f32 {
        self.echo_strength
    }

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

    pub fn get_mid_freq(&self) -> f32 {
        self.params.mid_freq
    }

    pub fn get_mid_gain(&self) -> f32 {
        self.params.mid_gain
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

    pub fn get_amount(&self) -> f32 {
        self.amount
    }
}

// =============================================================================
// Stereo Wrapper
// =============================================================================

/// Stereo Radio Voice Processor
///
/// Wraps two mono processors for stereo operation with linked analysis.
pub struct StereoRadioVoice {
    left: RadioVoiceProcessor,
    right: RadioVoiceProcessor,
    amount: f32,
}

impl StereoRadioVoice {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            left: RadioVoiceProcessor::new(sample_rate),
            right: RadioVoiceProcessor::new(sample_rate),
            amount: 1.0,
        }
    }

    pub fn set_amount(&mut self, amount: f32) {
        self.amount = amount.clamp(0.0, 1.0);
        self.left.set_amount(self.amount);
        self.right.set_amount(self.amount);
    }

    /// Analyze mono mix and configure both channels
    pub fn analyze(&mut self, mono_samples: &[f32]) {
        self.left.analyze(mono_samples);
        // Copy configuration to right channel
        self.right.detected_f0 = self.left.detected_f0;
        self.right.sibilance_level = self.left.sibilance_level;
        self.right.params = self.left.params.clone();
        self.right.eq.configure(&self.right.params);
        self.right.configured = true;
    }

    // Getters for reporting (from left channel)
    pub fn get_detected_f0(&self) -> f32 {
        self.left.get_detected_f0()
    }

    pub fn get_sibilance_level(&self) -> f32 {
        self.left.get_sibilance_level()
    }

    pub fn get_hpf_freq(&self) -> f32 {
        self.left.get_hpf_freq()
    }

    pub fn get_low_shelf_freq(&self) -> f32 {
        self.left.get_low_shelf_freq()
    }

    pub fn get_low_shelf_gain(&self) -> f32 {
        self.left.get_low_shelf_gain()
    }

    pub fn get_mud_freq(&self) -> f32 {
        self.left.get_mud_freq()
    }

    pub fn get_mud_gain(&self) -> f32 {
        self.left.get_mud_gain()
    }

    pub fn get_mid_freq(&self) -> f32 {
        self.left.get_mid_freq()
    }

    pub fn get_mid_gain(&self) -> f32 {
        self.left.get_mid_gain()
    }

    pub fn get_presence_freq(&self) -> f32 {
        self.left.get_presence_freq()
    }

    pub fn get_presence_gain(&self) -> f32 {
        self.left.get_presence_gain()
    }

    pub fn get_air_gain(&self) -> f32 {
        self.left.get_air_gain()
    }
}

impl AudioProcessor for StereoRadioVoice {
    fn process_buffer(&mut self, _buffer: &mut [f32]) {
        // Use process_stereo instead
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}

impl StereoProcessor for StereoRadioVoice {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.left.process(left);
        self.right.process(right);
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }
}

impl Processor for StereoRadioVoice {
    fn new(sample_rate: f32) -> Self {
        Self::new(sample_rate as u32)
    }
}

impl MonoProcessor for StereoRadioVoice {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radio_voice_creation() {
        let radio = RadioVoiceProcessor::new(48000);
        assert!((radio.get_amount() - 1.0).abs() < 0.01);
        assert!((radio.get_detected_f0() - 120.0).abs() < 1.0);
    }

    #[test]
    fn test_analyze_and_process() {
        let mut radio = RadioVoiceProcessor::new(48000);

        // Generate some test audio (speech-like mix of frequencies)
        let samples: Vec<f32> = (0..48000)
            .map(|i| {
                let t = i as f32 / 48000.0;
                let f0 = 150.0; // Fundamental
                let h1 = (2.0 * std::f32::consts::PI * f0 * t).sin();
                let h2 = 0.5 * (2.0 * std::f32::consts::PI * f0 * 2.0 * t).sin();
                let h3 = 0.3 * (2.0 * std::f32::consts::PI * f0 * 3.0 * t).sin();
                (h1 + h2 + h3) * 0.3
            })
            .collect();

        // Analyze
        radio.analyze(&samples);

        // Should detect f0 approximately correct
        let detected = radio.get_detected_f0();
        // Allow wide tolerance since cepstral f0 detection on synthetic audio may vary
        assert!(detected > 50.0 && detected < 400.0, "f0 should be in voice range: {}", detected);

        // Process
        let mut output = samples.clone();
        radio.process(&mut output);

        // Output should differ from input
        let diff: f32 = samples
            .iter()
            .zip(output.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / samples.len() as f32;

        assert!(diff > 0.001, "Output should differ from input");
    }

    #[test]
    fn test_amount_zero_bypass() {
        let mut radio = RadioVoiceProcessor::new(48000);
        radio.set_amount(0.0);

        let samples: Vec<f32> = (0..4800)
            .map(|i| {
                let t = i as f32 / 48000.0;
                (2.0 * std::f32::consts::PI * 440.0 * t).sin()
            })
            .collect();

        radio.analyze(&samples);

        let mut output = samples.clone();
        radio.process(&mut output);

        // With amount=0, output should equal input
        for (a, b) in samples.iter().zip(output.iter()) {
            assert!((a - b).abs() < 0.0001, "Should be bypassed with amount=0");
        }
    }
}
