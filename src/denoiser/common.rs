//! Shared code between batch (CLI) and realtime (plugin) denoisers

use std::f32::consts::PI;

// =============================================================================
// Shared Constants
// =============================================================================

pub const WINDOW_SIZE: usize = 2048;
pub const HOP_SIZE: usize = 1024;
pub const EPSILON: f32 = 1e-10;
pub const WARMUP_FRAMES: usize = 20;
pub const TRANSITION_BINS: usize = 10;

// Noise estimation defaults
pub const DEFAULT_LAMBDA: f32 = 0.95;
pub const DEFAULT_SPIKE_THRESHOLD: f32 = 10.0;
pub const DEFAULT_SFM_SPEECH: f32 = 0.1;
pub const DEFAULT_SFM_NOISE: f32 = 0.4;

// =============================================================================
// Window Functions
// =============================================================================

pub fn root_hann_window(n: usize) -> Vec<f32> {
    (0..n).map(|i| (PI * i as f32 / n as f32).sin()).collect()
}

// =============================================================================
// Spectral Flatness Measure
// =============================================================================

pub fn compute_sfm(power_spectrum: &[f32]) -> f32 {
    let n = power_spectrum.len() as f32;

    // Geometric mean via log
    let log_sum: f32 = power_spectrum.iter().map(|&p| (p + EPSILON).ln()).sum();
    let geo_mean = (log_sum / n).exp();

    // Arithmetic mean
    let arith_mean: f32 = power_spectrum.iter().sum::<f32>() / n;

    geo_mean / (arith_mean + EPSILON)
}

// =============================================================================
// Frequency/Bin Conversion
// =============================================================================

pub fn hz_to_bin(hz: f32, fft_size: usize, sample_rate: u32) -> usize {
    (hz * fft_size as f32 / sample_rate as f32) as usize
}
