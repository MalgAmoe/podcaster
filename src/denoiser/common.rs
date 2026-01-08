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
// Band Configuration (24 Bark critical bands)
// =============================================================================

pub const NUM_BANDS: usize = 24;

// Frequency ranges for psychoacoustic Bark bands
pub const BANDS: [(f32, f32); NUM_BANDS] = [
    // Low (0-770Hz): Conservative
    (0.0, 100.0),
    (100.0, 200.0),
    (200.0, 300.0),
    (300.0, 400.0),
    (400.0, 510.0),
    (510.0, 630.0),
    (630.0, 770.0),
    // Mid (770-2320Hz): Aggressive on speech
    (770.0, 920.0),
    (920.0, 1080.0),
    (1080.0, 1270.0),
    (1270.0, 1480.0),
    (1480.0, 1720.0),
    (1720.0, 2000.0),
    (2000.0, 2320.0),
    // High-mid (2320-5300Hz): Intelligibility
    (2320.0, 2700.0),
    (2700.0, 3150.0),
    (3150.0, 3700.0),
    (3700.0, 4400.0),
    (4400.0, 5300.0),
    // High (5300-15500Hz): Air/hiss
    (5300.0, 6400.0),
    (6400.0, 7700.0),
    (7700.0, 9500.0),
    (9500.0, 12000.0),
    (12000.0, 15500.0),
];

// Default delta values for "Moderate" preset (per-band SNR sensitivity)
pub const DEFAULT_DELTA: [f32; NUM_BANDS] = [
    // Low (7 bands)
    0.8, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0,
    // Mid (7 bands)
    2.2, 2.5, 2.8, 2.8, 2.8, 2.8, 2.5,
    // High-mid (5 bands)
    2.3, 2.0, 1.8, 1.8, 1.8,
    // High (5 bands)
    1.5, 1.3, 1.2, 1.0, 0.9,
];

// Default gamma values for "Moderate" preset (per-band temporal smoothing)
pub const DEFAULT_GAMMA: [f32; NUM_BANDS] = [
    // Low (7 bands)
    0.50, 0.52, 0.54, 0.56, 0.58, 0.60, 0.62,
    // Mid (7 bands)
    0.65, 0.68, 0.72, 0.75, 0.78, 0.80, 0.82,
    // High-mid (5 bands)
    0.84, 0.86, 0.87, 0.88, 0.89,
    // High (5 bands)
    0.90, 0.91, 0.92, 0.93, 0.94,
];

// =============================================================================
// Preset Configuration (shared between CLI and plugin)
// =============================================================================

#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub alpha_base: f32,
    pub alpha_min: f32,
    pub alpha_max: f32,
    pub beta: f32,
    pub delta: [f32; NUM_BANDS],
    pub gamma: [f32; NUM_BANDS],
}

pub const PRESETS: [Preset; 5] = [
    // 1: Gentle - minimal processing, preserve everything
    Preset {
        name: "Gentle",
        alpha_base: 1.5,
        alpha_min: 0.5,
        alpha_max: 2.5,
        beta: 0.15,
        delta: [
            0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, // Low (7)
            1.2, 1.4, 1.6, 1.6, 1.6, 1.6, 1.4, // Mid (7)
            1.3, 1.2, 1.1, 1.1, 1.1, // High-mid (5)
            1.0, 0.9, 0.8, 0.7, 0.6, // High (5)
        ],
        gamma: [
            0.40, 0.42, 0.44, 0.46, 0.48, 0.50, 0.52, // Low (7)
            0.55, 0.58, 0.62, 0.65, 0.68, 0.70, 0.72, // Mid (7)
            0.74, 0.76, 0.77, 0.78, 0.79, // High-mid (5)
            0.80, 0.81, 0.82, 0.83, 0.84, // High (5)
        ],
    },
    // 2: Light - subtle noise reduction
    Preset {
        name: "Light",
        alpha_base: 2.0,
        alpha_min: 0.8,
        alpha_max: 3.5,
        beta: 0.08,
        delta: [
            0.6, 0.8, 1.0, 1.2, 1.3, 1.5, 1.6, // Low (7)
            1.8, 2.0, 2.2, 2.2, 2.2, 2.2, 2.0, // Mid (7)
            1.9, 1.6, 1.5, 1.5, 1.5, // High-mid (5)
            1.2, 1.1, 1.0, 0.9, 0.8, // High (5)
        ],
        gamma: [
            0.45, 0.47, 0.49, 0.51, 0.53, 0.55, 0.57, // Low (7)
            0.60, 0.63, 0.67, 0.70, 0.73, 0.75, 0.77, // Mid (7)
            0.79, 0.81, 0.82, 0.83, 0.84, // High-mid (5)
            0.85, 0.86, 0.87, 0.88, 0.89, // High (5)
        ],
    },
    // 3: Moderate - balanced (default)
    Preset {
        name: "Moderate",
        alpha_base: 3.0,
        alpha_min: 1.0,
        alpha_max: 5.0,
        beta: 0.05,
        delta: DEFAULT_DELTA, // Use shared default
        gamma: DEFAULT_GAMMA, // Use shared default
    },
    // 4: Strong - noticeable noise reduction
    Preset {
        name: "Strong",
        alpha_base: 4.5,
        alpha_min: 2.5,
        alpha_max: 7.0,
        beta: 0.02,
        delta: [
            1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.2, // Low (7)
            2.5, 2.8, 3.2, 3.2, 3.2, 3.2, 2.8, // Mid (7)
            2.6, 2.3, 2.1, 2.1, 2.1, // High-mid (5)
            1.7, 1.5, 1.4, 1.2, 1.0, // High (5)
        ],
        gamma: [
            0.55, 0.57, 0.59, 0.61, 0.63, 0.65, 0.67, // Low (7)
            0.70, 0.73, 0.77, 0.80, 0.83, 0.85, 0.87, // Mid (7)
            0.89, 0.91, 0.92, 0.93, 0.94, // High-mid (5)
            0.95, 0.96, 0.97, 0.97, 0.98, // High (5)
        ],
    },
    // 5: Aggressive - maximum removal, may affect speech
    Preset {
        name: "Aggressive",
        alpha_base: 6.0,
        alpha_min: 2.0,
        alpha_max: 10.0,
        beta: 0.008,
        delta: [
            1.2, 1.5, 1.8, 2.0, 2.2, 2.4, 2.6, // Low (7)
            3.0, 3.5, 4.0, 4.0, 4.0, 4.0, 3.5, // Mid (7)
            3.2, 2.8, 2.5, 2.5, 2.5, // High-mid (5)
            2.0, 1.7, 1.5, 1.3, 1.2, // High (5)
        ],
        gamma: [
            0.60, 0.62, 0.64, 0.66, 0.68, 0.70, 0.72, // Low (7)
            0.75, 0.78, 0.82, 0.85, 0.88, 0.90, 0.92, // Mid (7)
            0.94, 0.95, 0.96, 0.96, 0.97, // High-mid (5)
            0.97, 0.98, 0.98, 0.99, 0.99, // High (5)
        ],
    },
];

pub const DEFAULT_PRESET: u8 = 3; // 1-indexed in CLI

pub fn get_preset(level: usize) -> Option<&'static Preset> {
    if level >= 1 && level <= 5 {
        Some(&PRESETS[level - 1])
    } else {
        None
    }
}

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

// =============================================================================
// Alpha/Gamma Curve Computation (shared between batch and RT)
// =============================================================================

/// Compute per-bin alpha values with band-specific delta and smooth transitions
pub fn compute_alpha_curve(
    fft_size: usize,
    sample_rate: u32,
    snr_per_bin: &[f32],
    delta: &[f32; NUM_BANDS],
    alpha_base: f32,
    alpha_min: f32,
    alpha_max: f32,
) -> Vec<f32> {
    let n_bins = fft_size / 2 + 1;
    let mut delta_curve = vec![delta[NUM_BANDS - 1]; n_bins];

    // Assign delta per band
    for (i, &(start_hz, end_hz)) in BANDS.iter().enumerate() {
        let start_bin = hz_to_bin(start_hz, fft_size, sample_rate);
        let end_bin = hz_to_bin(end_hz, fft_size, sample_rate).min(n_bins);
        for k in start_bin..end_bin {
            delta_curve[k] = delta[i];
        }
    }

    // Compute alpha per bin
    let mut alpha: Vec<f32> = snr_per_bin
        .iter()
        .zip(delta_curve.iter())
        .map(|(&snr, &d)| (alpha_base - snr / (d + EPSILON)).clamp(alpha_min, alpha_max))
        .collect();

    // Smooth transitions
    for i in 0..BANDS.len() - 1 {
        let transition_bin = hz_to_bin(BANDS[i].1, fft_size, sample_rate);
        let half_width = TRANSITION_BINS / 2;
        let start = transition_bin.saturating_sub(half_width);
        let end = (transition_bin + half_width).min(n_bins);

        for k in start..end {
            let base = transition_bin.saturating_sub(half_width);
            let t = (k - base) as f32 / TRANSITION_BINS as f32;
            let w = 0.5 * (1.0 - (PI * t).cos());
            if k > 0 && k < n_bins - 1 && k > transition_bin {
                alpha[k] = (1.0 - w) * alpha[k - 1] + w * alpha[k + 1];
            }
        }
    }

    alpha
}

/// Compute per-bin gamma values with band-specific values and smooth transitions
pub fn compute_gamma_curve(
    fft_size: usize,
    sample_rate: u32,
    gamma_per_band: &[f32; NUM_BANDS],
) -> Vec<f32> {
    let n_bins = fft_size / 2 + 1;
    let mut gamma = vec![gamma_per_band[NUM_BANDS - 1]; n_bins];

    // Assign gamma per band
    for (i, &(start_hz, end_hz)) in BANDS.iter().enumerate() {
        let start_bin = hz_to_bin(start_hz, fft_size, sample_rate);
        let end_bin = hz_to_bin(end_hz, fft_size, sample_rate).min(n_bins);
        for k in start_bin..end_bin {
            gamma[k] = gamma_per_band[i];
        }
    }

    // Smooth transitions
    for i in 0..BANDS.len() - 1 {
        let transition_bin = hz_to_bin(BANDS[i].1, fft_size, sample_rate);
        let half_width = TRANSITION_BINS / 2;
        let start = transition_bin.saturating_sub(half_width);
        let end = (transition_bin + half_width).min(n_bins);

        for k in start..end {
            let base = transition_bin.saturating_sub(half_width);
            let t = (k - base) as f32 / TRANSITION_BINS as f32;
            let w = 0.5 * (1.0 - (PI * t).cos());
            gamma[k] = (1.0 - w) * gamma_per_band[i] + w * gamma_per_band[i + 1];
        }
    }

    gamma
}
