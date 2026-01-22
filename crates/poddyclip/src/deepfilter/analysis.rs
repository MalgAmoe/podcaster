//! Audio analysis for DeepFilterNet auto-tuning

use std::f32::consts::PI;

/// Analysis results used for auto-tuning DeepFilterNet parameters
#[derive(Debug, Clone)]
pub struct DfAnalysis {
    /// Peak level in dB
    pub peak_db: f32,
    /// Estimated signal-to-noise ratio in dB
    pub estimated_snr: f32,
    /// Whether DC offset was detected (>1% of full scale)
    pub has_dc_offset: bool,
    /// Ratio of energy below 80Hz to total energy (rumble detection)
    pub low_freq_energy_ratio: f32,
}

impl DfAnalysis {
    /// Returns a severity description based on SNR
    pub fn noise_severity(&self) -> &'static str {
        match self.estimated_snr {
            snr if snr < 10.0 => "very noisy",
            snr if snr < 20.0 => "noisy",
            snr if snr < 30.0 => "moderate",
            snr if snr < 40.0 => "clean",
            _ => "very clean",
        }
    }
}

/// Analyze audio for DeepFilterNet auto-tuning
///
/// This function analyzes the input audio to determine optimal processing
/// parameters for the DeepFilterNet model.
///
/// # Arguments
///
/// * `samples` - Mono audio samples (f32, normalized to [-1, 1])
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
///
/// Analysis results containing SNR estimate, peak level, and other metrics
pub fn analyze_for_deepfilter(samples: &[f32], sample_rate: u32) -> DfAnalysis {
    let eps = 1e-9;

    // Peak level
    let peak = samples.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
    let peak_db = 20.0 * (peak.max(eps)).log10();

    // RMS level
    let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
    let rms_db = 20.0 * (rms.max(eps)).log10();

    // DC offset detection
    let dc_offset = samples.iter().sum::<f32>() / samples.len() as f32;
    let has_dc_offset = dc_offset.abs() > 0.01;

    // Noise floor estimation (RMS of quietest 10% of frames)
    let noise_floor_db = estimate_noise_floor(samples, sample_rate);

    // SNR estimate
    let estimated_snr = rms_db - noise_floor_db;

    // Low frequency energy ratio (rumble detection)
    let low_freq_energy_ratio = estimate_low_freq_ratio(samples, sample_rate);

    DfAnalysis {
        peak_db,
        estimated_snr,
        has_dc_offset,
        low_freq_energy_ratio,
    }
}

/// Estimate noise floor using minimum statistics on short frames
fn estimate_noise_floor(samples: &[f32], sample_rate: u32) -> f32 {
    // Split into 50ms frames, find RMS of each
    let frame_size = ((sample_rate as usize * 50) / 1000).max(1);

    let mut frame_rms: Vec<f32> = samples
        .chunks(frame_size)
        .map(|chunk| {
            let rms = (chunk.iter().map(|x| x * x).sum::<f32>() / chunk.len() as f32).sqrt();
            rms
        })
        .filter(|&rms| rms > 1e-10) // ignore silence
        .collect();

    frame_rms.retain(|x| x.is_finite());
    frame_rms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    // Take 10th percentile as noise floor
    let idx = frame_rms.len() / 10;
    let noise_rms = frame_rms.get(idx).copied().unwrap_or(1e-10);
    20.0 * noise_rms.log10()
}

/// Estimate the ratio of low-frequency energy (below 80Hz) to total energy
fn estimate_low_freq_ratio(samples: &[f32], sample_rate: u32) -> f32 {
    // Simple energy below 80Hz vs total using a single-pole high-pass filter
    let cutoff = 80.0;
    let rc = 1.0 / (2.0 * PI * cutoff);
    let dt = 1.0 / sample_rate as f32;
    let alpha = rc / (rc + dt);

    let mut hp_prev = 0.0f32;
    let mut prev_sample = 0.0f32;
    let mut low_energy = 0.0f32;
    let mut total_energy = 0.0f32;

    for &sample in samples {
        // High-pass filter
        let hp = alpha * (hp_prev + sample - prev_sample);
        let lp = sample - hp; // Low-pass = original - high-pass

        low_energy += lp * lp;
        total_energy += sample * sample;

        hp_prev = hp;
        prev_sample = sample;
    }

    if total_energy > 0.0 {
        low_energy / total_energy
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_silence() {
        let samples = vec![0.0f32; 48000];
        let analysis = analyze_for_deepfilter(&samples, 48000);
        assert!(analysis.peak_db < -80.0);
        assert!(!analysis.has_dc_offset);
    }

    #[test]
    fn test_analyze_sine() {
        // Generate 1 second of 440Hz sine at -6dB
        let sample_rate = 48000u32;
        let amplitude = 0.5f32; // -6dB
        let samples: Vec<f32> = (0..sample_rate)
            .map(|i| amplitude * (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin())
            .collect();

        let analysis = analyze_for_deepfilter(&samples, sample_rate);
        assert!((analysis.peak_db - (-6.0)).abs() < 1.0);
        assert!(!analysis.has_dc_offset);
    }

    #[test]
    fn test_dc_offset_detection() {
        let samples: Vec<f32> = (0..48000).map(|_| 0.05).collect(); // 5% DC offset
        let analysis = analyze_for_deepfilter(&samples, 48000);
        assert!(analysis.has_dc_offset);
    }
}
