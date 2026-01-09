//! LUFS Measurement (ITU-R BS.1770-4)
//!
//! Measures integrated loudness using K-weighting and gating.

#![allow(dead_code)]

use std::f32::consts::PI;

/// Default target LUFS for streaming platforms
pub const DEFAULT_TARGET_LUFS: f32 = -16.0;
/// Biquad filter coefficients
#[derive(Clone, Copy)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// Biquad filter state
#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    fn process(&mut self, input: f32, coeffs: &BiquadCoeffs) -> f32 {
        let output = coeffs.b0 * input + coeffs.b1 * self.x1 + coeffs.b2 * self.x2
            - coeffs.a1 * self.y1
            - coeffs.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }
}

/// K-weighting filter for LUFS measurement
/// Consists of two stages:
/// 1. High shelf filter (+4dB @ 1681Hz) - models acoustic effects of the head
/// 2. High-pass filter (38Hz, Q=0.5) - removes DC and very low frequencies
struct KWeightingFilter {
    shelf_coeffs: BiquadCoeffs,
    highpass_coeffs: BiquadCoeffs,
    shelf_state: BiquadState,
    highpass_state: BiquadState,
}

impl KWeightingFilter {
    fn new(sample_rate: f32) -> Self {
        // Stage 1: High shelf filter
        // ITU-R BS.1770-4 specifies these coefficients for 48kHz
        // We'll calculate them for the actual sample rate
        let shelf_coeffs = Self::calc_high_shelf(sample_rate, 1681.0, 4.0);

        // Stage 2: High-pass filter at 38Hz
        let highpass_coeffs = Self::calc_highpass(sample_rate, 38.0, 0.5);

        Self {
            shelf_coeffs,
            highpass_coeffs,
            shelf_state: BiquadState::default(),
            highpass_state: BiquadState::default(),
        }
    }

    fn calc_high_shelf(sample_rate: f32, freq: f32, gain_db: f32) -> BiquadCoeffs {
        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();

        // Peaking shelf with Q = 1/sqrt(2) for smooth response
        let alpha = sin_w0 / 2.0 * (2.0_f32).sqrt();

        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    fn calc_highpass(sample_rate: f32, freq: f32, q: f32) -> BiquadCoeffs {
        let w0 = 2.0 * PI * freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let shelf_out = self.shelf_state.process(sample, &self.shelf_coeffs);
        self.highpass_state
            .process(shelf_out, &self.highpass_coeffs)
    }

    fn reset(&mut self) {
        self.shelf_state = BiquadState::default();
        self.highpass_state = BiquadState::default();
    }
}

/// Calculate mean square of a block of samples
fn mean_square(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    sum / samples.len() as f32
}

/// Convert mean square to LUFS
fn ms_to_lufs(ms: f32) -> f32 {
    if ms < 1e-10 {
        return -70.0; // Below gate threshold
    }
    -0.691 + 10.0 * ms.log10()
}

/// Measure integrated LUFS (ITU-R BS.1770-4)
///
/// # Arguments
/// * `samples` - Audio channels (mono or stereo)
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// Integrated LUFS value
pub fn measure_integrated_lufs(samples: &[Vec<f32>], sample_rate: u32) -> f32 {
    let sample_rate_f = sample_rate as f32;

    // Block size: 400ms with 75% overlap (100ms hop)
    let block_samples = (0.4 * sample_rate_f) as usize;
    let hop_samples = (0.1 * sample_rate_f) as usize;

    // Apply K-weighting to all channels
    let weighted: Vec<Vec<f32>> = samples
        .iter()
        .map(|channel| {
            let mut filter = KWeightingFilter::new(sample_rate_f);
            channel.iter().map(|&s| filter.process(s)).collect()
        })
        .collect();

    // Calculate mean square for each 400ms block
    let num_samples = weighted.first().map(|c| c.len()).unwrap_or(0);
    if num_samples < block_samples {
        // Audio too short, just measure the whole thing
        let total_ms: f32 =
            weighted.iter().map(|ch| mean_square(ch)).sum::<f32>() / weighted.len().max(1) as f32;
        return ms_to_lufs(total_ms);
    }

    let mut block_ms: Vec<f32> = Vec::new();

    let mut pos = 0;
    while pos + block_samples <= num_samples {
        // Sum mean square across all channels (equal weight for stereo)
        let ms: f32 = weighted
            .iter()
            .map(|ch| mean_square(&ch[pos..pos + block_samples]))
            .sum::<f32>()
            / weighted.len() as f32;

        block_ms.push(ms);
        pos += hop_samples;
    }

    if block_ms.is_empty() {
        return -70.0;
    }

    // Step 1: Absolute gate at -70 LUFS
    let absolute_gate_ms = 10.0_f32.powf((-70.0 + 0.691) / 10.0);
    let gated_blocks: Vec<f32> = block_ms
        .iter()
        .copied()
        .filter(|&ms| ms > absolute_gate_ms)
        .collect();

    if gated_blocks.is_empty() {
        return -70.0;
    }

    // Step 2: Calculate ungated average
    let ungated_avg = gated_blocks.iter().sum::<f32>() / gated_blocks.len() as f32;
    let ungated_lufs = ms_to_lufs(ungated_avg);

    // Step 3: Relative gate at -10 LU below ungated average
    let relative_gate_lufs = ungated_lufs - 10.0;
    let relative_gate_ms = 10.0_f32.powf((relative_gate_lufs + 0.691) / 10.0);

    let final_blocks: Vec<f32> = gated_blocks
        .into_iter()
        .filter(|&ms| ms > relative_gate_ms)
        .collect();

    if final_blocks.is_empty() {
        return -70.0;
    }

    // Step 4: Calculate final integrated loudness
    let final_avg = final_blocks.iter().sum::<f32>() / final_blocks.len() as f32;
    ms_to_lufs(final_avg)
}

/// Calculate gain in dB to reach target LUFS
pub fn calculate_gain_for_target(current_lufs: f32, target_lufs: f32) -> f32 {
    target_lufs - current_lufs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence() {
        let samples = vec![vec![0.0; 48000]];
        let lufs = measure_integrated_lufs(&samples, 48000);
        assert!(lufs <= -70.0, "Silence should be at or below -70 LUFS");
    }

    #[test]
    fn test_sine_wave() {
        // Generate 1 second of 1kHz sine at -20 dBFS (0.1 amplitude)
        let amplitude = 0.1_f32;
        let samples: Vec<f32> = (0..48000)
            .map(|i| amplitude * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
            .collect();

        let lufs = measure_integrated_lufs(&vec![samples], 48000);

        // Should be around -23 LUFS (sine wave RMS = amplitude/sqrt(2), with K-weighting)
        // Allow some tolerance due to K-weighting and gating
        assert!(
            lufs > -30.0 && lufs < -15.0,
            "Sine wave LUFS {} should be in reasonable range",
            lufs
        );
    }

    #[test]
    fn test_gain_calculation() {
        let gain = calculate_gain_for_target(-20.0, -14.0);
        assert!((gain - 6.0).abs() < 0.001);
    }
}
