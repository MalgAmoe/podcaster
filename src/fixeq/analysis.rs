//! Audio analysis for FixEq
//!
//! Analyzes denoised audio to find optimal frequencies for dynamic EQ bands.
#![cfg_attr(all(feature = "cli", feature = "plugin"), allow(dead_code))]

use crate::analysis::SpectralAnalysis;

/// Mud frequency range (Hz)
const MUD_FREQ_MIN: f32 = 150.0;
const MUD_FREQ_MAX: f32 = 500.0;

/// Correction frequency range (Hz) - mid-range for resonances/harshness
const CORRECTION_FREQ_MIN: f32 = 500.0;
const CORRECTION_FREQ_MAX: f32 = 5000.0;

/// Minimum spacing between correction peaks (Hz)
const CORRECTION_MIN_SPACING_HZ: f32 = 200.0;

/// Result of band frequency analysis
#[derive(Clone, Debug)]
pub struct BandAnalysis {
    /// Detected center frequency (Hz)
    pub center_freq: f32,
    /// Energy level at the detected frequency (dB)
    pub energy_db: f32,
    /// Confidence score (0.0 = uncertain, 1.0 = very confident)
    pub confidence: f32,
}

/// Combined analysis for all FixEq bands
#[derive(Clone, Debug)]
pub struct FixEqAnalysis {
    pub mud: BandAnalysis,
    pub correction_a: BandAnalysis,
    pub correction_b: BandAnalysis,
}

/// Re-export mix_to_mono from analysis module
pub use crate::analysis::mix_to_mono;

/// Default correction frequencies (Hz)
pub const DEFAULT_CORRECTION_A_FREQ: f32 = 1000.0;
pub const DEFAULT_CORRECTION_B_FREQ: f32 = 3000.0;

/// Analyze audio using shared spectral analysis
pub fn analyze_from_spectrum(spectrum: &SpectralAnalysis) -> FixEqAnalysis {
    // Find mud peak (150-500Hz)
    let (mud_freq, mud_energy, mud_conf) =
        spectrum.find_peak_deviation(MUD_FREQ_MIN, MUD_FREQ_MAX, 300.0);

    let mud = BandAnalysis {
        center_freq: mud_freq,
        energy_db: mud_energy,
        confidence: mud_conf,
    };

    // Find two correction peaks (500-5000Hz)
    let ((freq_a, energy_a, conf_a), (freq_b, energy_b, conf_b)) = spectrum.find_two_peaks_deviation(
        CORRECTION_FREQ_MIN,
        CORRECTION_FREQ_MAX,
        CORRECTION_MIN_SPACING_HZ,
        DEFAULT_CORRECTION_A_FREQ,
        DEFAULT_CORRECTION_B_FREQ,
    );

    let correction_a = BandAnalysis {
        center_freq: freq_a,
        energy_db: energy_a,
        confidence: conf_a,
    };

    let correction_b = BandAnalysis {
        center_freq: freq_b,
        energy_db: energy_b,
        confidence: conf_b,
    };

    FixEqAnalysis {
        mud,
        correction_a,
        correction_b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_empty() {
        let spectrum = SpectralAnalysis::new(&[], 48000);
        let result = analyze_from_spectrum(&spectrum);
        // Empty audio should still return valid structure
        assert!(result.mud.center_freq > 0.0);
        assert!(result.correction_a.center_freq > 0.0);
        assert!(result.correction_b.center_freq > 0.0);
    }
}
