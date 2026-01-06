//! Spectral Subtraction Noise Reduction
//!
//! Rust port of the Python validation implementation.
//! Minimal dependencies: rustfft for FFT operations.

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

// =============================================================================
// Constants (from spec)
// =============================================================================

pub const WINDOW_SIZE: usize = 2048;
pub const HOP_SIZE: usize = 1024;
pub const SAMPLE_RATE: u32 = 48000;

// Noise estimation
const LAMBDA: f32 = 0.92; // forget factor
const SPIKE_THRESHOLD: f32 = 10.0; // 10 dB power spike
const WARMUP_FRAMES: usize = 20;

// SFM thresholds
const SFM_SPEECH: f32 = 0.1; // below = tonal (freeze)
const SFM_NOISE: f32 = 0.4; // above = flat (update)

// 9-Band configuration for adaptive alpha
// (start_hz, end_hz, delta)
const BANDS: [(f32, f32, f32); 9] = [
    (0.0, 80.0, 0.8),       // Band 0: Rumble, pops, HVAC
    (80.0, 250.0, 1.0),     // Band 1: Voice fundamental
    (250.0, 500.0, 1.5),    // Band 2: Warmth
    (500.0, 1000.0, 2.0),   // Band 3: Body
    (1000.0, 2000.0, 2.5),  // Band 4: Presence
    (2000.0, 4000.0, 2.5),  // Band 5: Intelligibility
    (4000.0, 8000.0, 2.0),  // Band 6: Sibilance
    (8000.0, 12000.0, 1.5), // Band 7: Air
    (12000.0, 24000.0, 1.0), // Band 8: Hiss region
];

const TRANSITION_BINS: usize = 10;
const EPSILON: f32 = 1e-10;

// =============================================================================
// Preset Configuration
// =============================================================================

#[derive(Clone, Copy, Debug)]
pub struct Preset {
    pub name: &'static str,
    pub alpha_base: f32,
    pub alpha_min: f32,
    pub alpha_max: f32,
    pub beta: f32,
    pub gamma: [f32; 9],
}

pub const PRESETS: [Preset; 5] = [
    // 1: Gentle - minimal processing, preserve everything
    Preset {
        name: "Gentle",
        alpha_base: 1.5,
        alpha_min: 0.5,
        alpha_max: 2.5,
        beta: 0.15,
        gamma: [0.40, 0.45, 0.50, 0.55, 0.60, 0.65, 0.70, 0.75, 0.80],
    },
    // 2: Light - subtle noise reduction
    Preset {
        name: "Light",
        alpha_base: 2.0,
        alpha_min: 0.8,
        alpha_max: 3.5,
        beta: 0.08,
        gamma: [0.45, 0.50, 0.58, 0.65, 0.72, 0.78, 0.82, 0.86, 0.88],
    },
    // 3: Moderate - balanced (default)
    Preset {
        name: "Moderate",
        alpha_base: 3.0,
        alpha_min: 1.0,
        alpha_max: 5.0,
        beta: 0.05,
        gamma: [0.50, 0.55, 0.65, 0.72, 0.78, 0.82, 0.86, 0.90, 0.92],
    },
    // 4: Strong - noticeable noise reduction
    Preset {
        name: "Strong",
        alpha_base: 4.5,
        alpha_min: 1.5,
        alpha_max: 7.0,
        beta: 0.02,
        gamma: [0.55, 0.60, 0.70, 0.78, 0.84, 0.88, 0.91, 0.94, 0.96],
    },
    // 5: Aggressive - maximum removal, may affect speech
    Preset {
        name: "Aggressive",
        alpha_base: 6.0,
        alpha_min: 2.0,
        alpha_max: 10.0,
        beta: 0.008,
        gamma: [0.60, 0.65, 0.75, 0.82, 0.88, 0.92, 0.94, 0.96, 0.98],
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

fn root_hann_window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| (PI * i as f32 / n as f32).sin())
        .collect()
}

// =============================================================================
// Spectral Flatness Measure
// =============================================================================

fn compute_sfm(power_spectrum: &[f32]) -> f32 {
    let n = power_spectrum.len() as f32;

    // Geometric mean via log
    let log_sum: f32 = power_spectrum
        .iter()
        .map(|&p| (p + EPSILON).ln())
        .sum();
    let geo_mean = (log_sum / n).exp();

    // Arithmetic mean
    let arith_mean: f32 = power_spectrum.iter().sum::<f32>() / n;

    geo_mean / (arith_mean + EPSILON)
}

// =============================================================================
// Multi-band Alpha/Gamma Calculations
// =============================================================================

fn hz_to_bin(hz: f32, fft_size: usize, sample_rate: u32) -> usize {
    (hz * fft_size as f32 / sample_rate as f32) as usize
}

fn compute_alpha_curve(
    fft_size: usize,
    sample_rate: u32,
    snr_per_bin: &[f32],
    alpha_base: f32,
    alpha_min: f32,
    alpha_max: f32,
) -> Vec<f32> {
    let n_bins = fft_size / 2 + 1;
    let mut delta = vec![BANDS[8].2; n_bins]; // Default to last band's delta

    // Assign delta per band
    for &(start_hz, end_hz, d) in &BANDS {
        let start_bin = hz_to_bin(start_hz, fft_size, sample_rate);
        let end_bin = hz_to_bin(end_hz, fft_size, sample_rate).min(n_bins);
        for k in start_bin..end_bin {
            delta[k] = d;
        }
    }

    // Compute alpha per bin: α = clamp(α₀ - SNR/δ, min, max)
    let mut alpha: Vec<f32> = snr_per_bin
        .iter()
        .zip(delta.iter())
        .map(|(&snr, &d)| (alpha_base - snr / (d + EPSILON)).clamp(alpha_min, alpha_max))
        .collect();

    // Apply raised-cosine smoothing at band transitions
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

fn compute_gamma_curve(fft_size: usize, sample_rate: u32, gamma_per_band: &[f32; 9]) -> Vec<f32> {
    let n_bins = fft_size / 2 + 1;
    let mut gamma = vec![gamma_per_band[8]; n_bins]; // Default to last band

    // Assign gamma per band
    for (i, &(start_hz, end_hz, _)) in BANDS.iter().enumerate() {
        let start_bin = hz_to_bin(start_hz, fft_size, sample_rate);
        let end_bin = hz_to_bin(end_hz, fft_size, sample_rate).min(n_bins);
        for k in start_bin..end_bin {
            gamma[k] = gamma_per_band[i];
        }
    }

    // Smooth transitions between bands
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

// =============================================================================
// Core Denoiser
// =============================================================================

pub struct SpectralSubtractionDenoiser {
    sample_rate: u32,
    window_size: usize,
    hop_size: usize,
    n_bins: usize,

    // Preset parameters
    alpha_base: f32,
    alpha_min: f32,
    alpha_max: f32,
    beta: f32,

    // Window
    window: Vec<f32>,

    // State
    noise_pow: Vec<f32>,
    prev_gain: Vec<f32>,
    prev_sfm_decision: bool,
    frame_count: usize,

    // Precomputed gamma curve
    gamma: Vec<f32>,

    // Overlap buffer
    overlap_buffer: Vec<f32>,

    // FFT planners
    fft: std::sync::Arc<dyn rustfft::Fft<f32>>,
    ifft: std::sync::Arc<dyn rustfft::Fft<f32>>,

    // Scratch buffers for FFT
    fft_scratch: Vec<Complex<f32>>,
}

impl SpectralSubtractionDenoiser {
    pub fn new(sample_rate: u32, preset: usize) -> Self {
        let window_size = WINDOW_SIZE;
        let hop_size = HOP_SIZE;
        let n_bins = window_size / 2 + 1;

        let p = get_preset(preset).expect("Invalid preset (must be 1-5)");

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(window_size);
        let ifft = planner.plan_fft_inverse(window_size);

        let fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

        Self {
            sample_rate,
            window_size,
            hop_size,
            n_bins,
            alpha_base: p.alpha_base,
            alpha_min: p.alpha_min,
            alpha_max: p.alpha_max,
            beta: p.beta,
            window: root_hann_window(window_size),
            noise_pow: vec![0.0; n_bins],
            prev_gain: vec![1.0; n_bins],
            prev_sfm_decision: true,
            frame_count: 0,
            gamma: compute_gamma_curve(window_size, sample_rate, &p.gamma),
            overlap_buffer: vec![0.0; window_size],
            fft,
            ifft,
            fft_scratch,
        }
    }

    fn compute_snr_per_bin(&self, power: &[f32]) -> Vec<f32> {
        power
            .iter()
            .zip(self.noise_pow.iter())
            .map(|(&p, &n)| {
                let snr_linear = p / (n + EPSILON);
                10.0 * (snr_linear + EPSILON).log10()
            })
            .collect()
    }

    fn update_noise_estimate(&mut self, power: &[f32], force_update: bool) {
        for k in 0..self.n_bins {
            // Spike protection
            if !force_update && power[k] > SPIKE_THRESHOLD * self.noise_pow[k] {
                continue;
            }
            // Recursive update
            self.noise_pow[k] = LAMBDA * self.noise_pow[k] + (1.0 - LAMBDA) * power[k];
        }
    }

    fn compute_gain(&self, power: &[f32], alpha: &[f32]) -> Vec<f32> {
        power
            .iter()
            .zip(alpha.iter())
            .zip(self.noise_pow.iter())
            .map(|((&p, &a), &n)| {
                let subtracted = p - a * n;
                let floored = self.beta * p;
                let numerator = subtracted.max(floored);
                let gain = (numerator / (p + EPSILON)).sqrt();
                gain.clamp(0.0, 1.0)
            })
            .collect()
    }

    fn smooth_gain(&mut self, gain: &[f32]) -> Vec<f32> {
        let smoothed: Vec<f32> = self.gamma
            .iter()
            .zip(self.prev_gain.iter())
            .zip(gain.iter())
            .map(|((&g, &prev), &curr)| g * prev + (1.0 - g) * curr)
            .collect();

        self.prev_gain.copy_from_slice(&smoothed);
        smoothed
    }

    pub fn process_frame(&mut self, frame: &[f32]) -> Vec<f32> {
        assert_eq!(frame.len(), self.window_size);

        // Apply analysis window
        let windowed: Vec<f32> = frame
            .iter()
            .zip(self.window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        // Forward FFT
        let mut spectrum: Vec<Complex<f32>> = windowed
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();
        self.fft.process_with_scratch(&mut spectrum, &mut self.fft_scratch);

        // Compute power spectrum (only need first n_bins due to symmetry)
        let power: Vec<f32> = spectrum[..self.n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        let enhanced_spectrum = if self.frame_count < WARMUP_FRAMES {
            // Warmup: collect noise estimate, pass through
            self.update_noise_estimate(&power, true);
            spectrum
        } else {
            // SFM-based VAD
            let sfm = compute_sfm(&power);

            if sfm > SFM_NOISE {
                self.update_noise_estimate(&power, false);
                self.prev_sfm_decision = true;
            } else if sfm < SFM_SPEECH {
                self.prev_sfm_decision = false;
            } else if self.prev_sfm_decision {
                self.update_noise_estimate(&power, false);
            }

            // Compute adaptive alpha
            let snr = self.compute_snr_per_bin(&power);
            let alpha = compute_alpha_curve(
                self.window_size,
                self.sample_rate,
                &snr,
                self.alpha_base,
                self.alpha_min,
                self.alpha_max,
            );

            // Compute and smooth gain
            let gain = self.compute_gain(&power, &alpha);
            let gain = self.smooth_gain(&gain);

            // Apply gain to spectrum (maintain conjugate symmetry)
            let mut result = spectrum.clone();
            for k in 0..self.n_bins {
                result[k] = spectrum[k] * gain[k];
            }
            // Mirror for negative frequencies
            for k in self.n_bins..self.window_size {
                let mirror = self.window_size - k;
                result[k] = result[mirror].conj();
            }
            result
        };

        // Inverse FFT
        let mut time_domain = enhanced_spectrum;
        self.ifft.process_with_scratch(&mut time_domain, &mut self.fft_scratch);

        // Normalize and extract real part
        let scale = 1.0 / self.window_size as f32;
        let mut enhanced: Vec<f32> = time_domain.iter().map(|c| c.re * scale).collect();

        // Apply synthesis window
        for (e, &w) in enhanced.iter_mut().zip(self.window.iter()) {
            *e *= w;
        }

        // Overlap-add
        for (i, &e) in enhanced.iter().enumerate() {
            self.overlap_buffer[i] += e;
        }

        // Extract output
        let output: Vec<f32> = self.overlap_buffer[..self.hop_size].to_vec();

        // Shift buffer
        self.overlap_buffer.rotate_left(self.hop_size);
        for i in (self.window_size - self.hop_size)..self.window_size {
            self.overlap_buffer[i] = 0.0;
        }

        self.frame_count += 1;
        output
    }

    pub fn process(&mut self, audio: &[f32]) -> Vec<f32> {
        let original_len = audio.len();

        // Pad to multiple of hop size
        let pad_len = (self.hop_size - audio.len() % self.hop_size) % self.hop_size;
        let mut padded = audio.to_vec();
        padded.resize(audio.len() + pad_len, 0.0);

        // Pre-pad for first window
        let pre_pad = self.window_size - self.hop_size;
        let mut input = vec![0.0; pre_pad];
        input.extend(padded);

        let mut output = Vec::new();

        // Process frame by frame
        let mut i = 0;
        while i + self.window_size <= input.len() {
            let frame = &input[i..i + self.window_size];
            let out_frame = self.process_frame(frame);
            output.extend(out_frame);
            i += self.hop_size;
        }

        // Remove pre-padding and trim to original length
        if output.len() > pre_pad {
            output = output[pre_pad..].to_vec();
        }
        output.truncate(original_len);

        output
    }

    pub fn reset(&mut self) {
        self.noise_pow.fill(0.0);
        self.prev_gain.fill(1.0);
        self.prev_sfm_decision = true;
        self.frame_count = 0;
        self.overlap_buffer.fill(0.0);
    }
}

// =============================================================================
// Stereo Processing (M/S)
// =============================================================================

pub fn process_stereo(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    preset: usize,
) -> (Vec<f32>, Vec<f32>) {
    // Convert to M/S
    let mid: Vec<f32> = left
        .iter()
        .zip(right.iter())
        .map(|(&l, &r)| (l + r) / 2.0)
        .collect();

    let side: Vec<f32> = left
        .iter()
        .zip(right.iter())
        .map(|(&l, &r)| (l - r) / 2.0)
        .collect();

    // Process each channel
    let mut denoiser_mid = SpectralSubtractionDenoiser::new(sample_rate, preset);
    let mut denoiser_side = SpectralSubtractionDenoiser::new(sample_rate, preset);

    let mid_processed = denoiser_mid.process(&mid);
    let side_processed = denoiser_side.process(&side);

    // Ensure same length
    let min_len = mid_processed.len().min(side_processed.len());

    // Convert back to L/R
    let left_out: Vec<f32> = (0..min_len)
        .map(|i| mid_processed[i] + side_processed[i])
        .collect();

    let right_out: Vec<f32> = (0..min_len)
        .map(|i| mid_processed[i] - side_processed[i])
        .collect();

    (left_out, right_out)
}

// =============================================================================
// Level Matching
// =============================================================================

pub fn match_rms(input: &[f32], output: &mut [f32]) {
    let rms_in = (input.iter().map(|&s| s * s).sum::<f32>() / input.len() as f32 + EPSILON).sqrt();
    let rms_out = (output.iter().map(|&s| s * s).sum::<f32>() / output.len() as f32 + EPSILON).sqrt();

    let gain = rms_in / rms_out;

    for s in output.iter_mut() {
        *s *= gain;
    }
}
