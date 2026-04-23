//! Reverb analysis for de-reverb processing
//!
//! Computes reverb characteristics from existing cepstral analysis:
//! - RT60 (reverberation time)
//! - DRR (direct-to-reverberant ratio)
//! - C50 (clarity metric)
//! - Per-band decay rates

use super::{CepstralAnalysis, SpectralAnalysis};

/// Number of bands for reverb analysis (coarser than denoiser)
pub const NUM_REVERB_BANDS: usize = 6;

/// Frequency ranges for reverb bands
/// High frequencies decay faster than low frequencies
pub const REVERB_BANDS: [(f32, f32); NUM_REVERB_BANDS] = [
    (0.0, 250.0),      // Sub-bass/bass - slowest decay
    (250.0, 500.0),    // Low-mid
    (500.0, 1000.0),   // Mid
    (1000.0, 2000.0),  // Upper-mid
    (2000.0, 4000.0),  // Presence
    (4000.0, 16000.0), // High - fastest decay
];

/// Reverb analysis results
#[derive(Clone, Debug)]
pub struct ReverbAnalysis {
    /// Estimated RT60 (ms) - time for reverb to decay 60dB
    pub rt60_avg_ms: f32,
    /// RT60 per band (ms) - high frequencies decay faster
    pub rt60_per_band: [f32; NUM_REVERB_BANDS],
    /// Direct-to-Reverberant Ratio (dB)
    /// Positive = dry, negative = reverby
    pub drr_db: f32,
    /// C50 clarity metric (dB)
    /// Ratio of early (0-50ms) to late energy
    pub c50_db: f32,
    /// Echo delay if detected (ms)
    pub echo_delay_ms: Option<f32>,
    /// Echo strength (0.0-1.0)
    pub echo_strength: f32,
    /// Sample rate
    pub sample_rate: u32,
}

impl Default for ReverbAnalysis {
    fn default() -> Self {
        Self {
            rt60_avg_ms: 200.0,
            rt60_per_band: [300.0, 250.0, 200.0, 180.0, 150.0, 100.0],
            drr_db: 6.0, // Default: fairly dry
            c50_db: 10.0,
            echo_delay_ms: None,
            echo_strength: 0.0,
            sample_rate: 48000,
        }
    }
}

impl ReverbAnalysis {
    /// Compute reverb analysis from spectral and cepstral analysis
    pub fn from_analyses(spectrum: &SpectralAnalysis, cepstral: &CepstralAnalysis) -> Self {
        let sample_rate = spectrum.sample_rate;

        // Get echo info from cepstral analysis
        let echo_delay_ms = cepstral.echo_delay_ms;
        let raw_echo_strength = cepstral.echo_strength;

        // The cepstral echo detection can produce false positives from:
        // - Pitch harmonics (f0 and multiples)
        // - Formant structure
        // - Periodic patterns in speech
        //
        // Heuristics to reduce false positives:
        // 1. Short delays (<50ms) are often pitch-related, not reverb
        // 2. Use spectral tilt as additional indicator (reverb flattens spectrum)
        // 3. CPP (voicing) - highly voiced content may have periodic patterns

        // Spectral tilt: reverby rooms flatten the spectrum
        // Natural speech tilt: typically -5 to +5 dB
        // Very bass-heavy (< -15dB) or treble-heavy (> +10dB) is NOT reverb
        // Reverb flattens toward 0dB, but extreme tilt is something else
        let tilt_db = spectrum.tilt_db();

        // Tilt-based reverb indicator:
        // Natural tilt (-10 to +5dB) = could be reverb if flat
        // Extreme tilt = not reverb, something else (proximity effect, EQ, etc.)
        let tilt_reverb_factor = if tilt_db.abs() > 15.0 {
            0.0 // Extreme tilt = not reverb
        } else {
            // Closer to 0 = flatter = more reverby
            // -5 to +5 is natural speech range
            let flatness = 1.0 - (tilt_db.abs() / 10.0);
            flatness.clamp(0.0, 0.5) // Max 50% confidence from tilt alone
        };

        // Echo delay-based confidence:
        // Very short delays (30-60ms) could be pitch or early reflections
        // Medium delays (60-150ms) more likely true reverb
        // Long delays (>150ms) could be discrete echo
        let delay_confidence = if let Some(delay) = echo_delay_ms {
            if delay < 50.0 {
                0.3 // Low confidence for short delays
            } else if delay < 150.0 {
                1.0 // High confidence for typical reverb delays
            } else {
                0.7 // Medium confidence for long delays
            }
        } else {
            0.0 // No echo detected
        };

        // CPP-based adjustment: highly voiced content may have false positives
        let cpp_factor = if cepstral.cpp > 10.0 {
            0.5 // Reduce echo strength for highly voiced content
        } else {
            1.0
        };

        // Combine factors to estimate effective reverb strength
        let echo_strength = if raw_echo_strength > 0.0 {
            (raw_echo_strength * delay_confidence * cpp_factor)
                .max(tilt_reverb_factor) // Use tilt if stronger indicator
                .clamp(0.0, 1.0)
        } else {
            tilt_reverb_factor // Fall back to tilt-based estimate
        };

        // Estimate RT60 from adjusted echo strength
        // echo_strength 0.0 -> RT60 ~100ms (very dry room)
        // echo_strength 1.0 -> RT60 ~600ms (reverby room)
        let rt60_avg_ms = 100.0 + echo_strength * 500.0;

        // Per-band RT60: high frequencies decay faster
        // Typical ratio: bass decays 1.5-2x slower than highs
        let rt60_per_band = [
            rt60_avg_ms * 1.5, // Sub-bass: slowest
            rt60_avg_ms * 1.3, // Low-mid
            rt60_avg_ms * 1.1, // Mid
            rt60_avg_ms * 1.0, // Upper-mid (reference)
            rt60_avg_ms * 0.8, // Presence
            rt60_avg_ms * 0.5, // High: fastest decay
        ];

        // Estimate DRR from echo strength
        // echo_strength 0.0 -> DRR +10dB (very dry)
        // echo_strength 1.0 -> DRR -6dB (reverby)
        let drr_db = 10.0 - 16.0 * echo_strength;

        // Estimate C50 from DRR
        // C50 and DRR are related: C50 ≈ DRR + 3dB for typical rooms
        let c50_db = drr_db + 3.0;

        Self {
            rt60_avg_ms,
            rt60_per_band,
            drr_db,
            c50_db,
            echo_delay_ms,
            echo_strength,
            sample_rate,
        }
    }

    /// Compute decay coefficient per bin for given FFT parameters
    ///
    /// decay = 10^(-3 * hop_time / RT60)
    /// This gives the factor by which reverb decays per hop
    pub fn compute_decay_per_bin(&self, n_bins: usize, hop_size: usize, bin_freq: f32) -> Vec<f32> {
        let hop_time_ms = (hop_size as f32 / self.sample_rate as f32) * 1000.0;

        (0..n_bins)
            .map(|bin| {
                let freq = bin as f32 * bin_freq;
                let rt60 = self.rt60_at_freq(freq);
                // decay = 10^(-3 * hop_time / RT60)
                // -3 because we want 60dB decay, and 20*log10(decay^n) = -60
                // So decay^n = 10^(-3), and n = RT60/hop_time
                10.0f32.powf(-3.0 * hop_time_ms / rt60)
            })
            .collect()
    }

    /// Get RT60 at a specific frequency (interpolate between bands)
    pub fn rt60_at_freq(&self, freq: f32) -> f32 {
        // Find which band this frequency falls into
        for (i, &(low, high)) in REVERB_BANDS.iter().enumerate() {
            if freq >= low && freq < high {
                return self.rt60_per_band[i];
            }
        }
        // Above highest band, use fastest decay
        self.rt60_per_band[NUM_REVERB_BANDS - 1]
    }

    /// Check if recording is reverby (DRR < 0 dB)
    pub fn is_reverby(&self) -> bool {
        self.drr_db < 0.0
    }

    /// Get reverb severity description
    pub fn severity(&self) -> &'static str {
        if self.drr_db > 6.0 {
            "Dry"
        } else if self.drr_db > 0.0 {
            "Light reverb"
        } else if self.drr_db > -6.0 {
            "Moderate reverb"
        } else {
            "Heavy reverb"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let analysis = ReverbAnalysis::default();
        assert_eq!(analysis.rt60_avg_ms, 200.0);
        assert_eq!(analysis.drr_db, 6.0);
        assert!(!analysis.is_reverby());
    }

    #[test]
    fn test_decay_computation() {
        let mut analysis = ReverbAnalysis::default();
        analysis.rt60_avg_ms = 500.0;
        analysis.rt60_per_band = [500.0; NUM_REVERB_BANDS];

        // With 1024 hop at 48kHz = 21.33ms hop time
        // decay = 10^(-3 * 21.33 / 500) = 10^(-0.128) ≈ 0.745
        let decay = analysis.compute_decay_per_bin(1025, 1024, 48000.0 / 2048.0);
        assert!((decay[0] - 0.745).abs() < 0.01);
    }

    #[test]
    fn test_severity() {
        let mut analysis = ReverbAnalysis::default();

        analysis.drr_db = 10.0;
        assert_eq!(analysis.severity(), "Dry");

        analysis.drr_db = 3.0;
        assert_eq!(analysis.severity(), "Light reverb");

        analysis.drr_db = -3.0;
        assert_eq!(analysis.severity(), "Moderate reverb");

        analysis.drr_db = -10.0;
        assert_eq!(analysis.severity(), "Heavy reverb");
    }
}
