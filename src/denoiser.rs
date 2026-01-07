//! Spectral Subtraction Noise Reduction
//!
//! Rust port of the Python validation implementation.
//! Minimal dependencies: rustfft for FFT operations.

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

// =============================================================================
// Constants (from spec)
// =============================================================================

// WINDOW_SIZE and HOP_SIZE are fixed (not scaled with sample rate).
// At 48kHz: Window=42.67ms, Hop=21.33ms (50% overlap)
// At 44.1kHz: Window=46.44ms, Hop=23.22ms
// At 96kHz: Window=21.33ms, Hop=10.67ms
// The algorithm works at any sample rate; timing semantics change slightly.
pub const WINDOW_SIZE: usize = 2048;
pub const HOP_SIZE: usize = 1024;

// Reference sample rate - code works with any sample rate.
// The denoiser accepts sample_rate as a parameter and scales frequency-dependent
// calculations (Bark bands, analysis windows) appropriately.
pub const SAMPLE_RATE: u32 = 48000;

// Default noise estimation parameters (can be overridden)
pub const DEFAULT_LAMBDA: f32 = 0.95; // forget factor
pub const DEFAULT_SPIKE_THRESHOLD: f32 = 10.0; // power ratio spike
pub const DEFAULT_SFM_SPEECH: f32 = 0.1; // below = tonal (freeze)
pub const DEFAULT_SFM_NOISE: f32 = 0.4; // above = flat (update)

// 24 Bark critical bands configuration (psychoacoustic scale)
// (start_hz, end_hz, delta)
const BANDS: [(f32, f32, f32); 24] = [
    // Low (0-770Hz): Conservative
    (0.0, 100.0, 0.8),
    (100.0, 200.0, 1.0),
    (200.0, 300.0, 1.2),
    (300.0, 400.0, 1.4),
    (400.0, 510.0, 1.6),
    (510.0, 630.0, 1.8),
    (630.0, 770.0, 2.0),
    // Mid (770-3150Hz): Aggressive on speech
    (770.0, 920.0, 2.2),
    (920.0, 1080.0, 2.5),
    (1080.0, 1270.0, 2.8),
    (1270.0, 1480.0, 2.8),
    (1480.0, 1720.0, 2.8),
    (1720.0, 2000.0, 2.8),
    (2000.0, 2320.0, 2.5),
    // High-mid (3150-7700Hz): Intelligibility
    (2320.0, 2700.0, 2.3),
    (2700.0, 3150.0, 2.0),
    (3150.0, 3700.0, 1.8),
    (3700.0, 4400.0, 1.8),
    (4400.0, 5300.0, 1.8),
    // High (7700-15500Hz): Air/hiss
    (5300.0, 6400.0, 1.5),
    (6400.0, 7700.0, 1.3),
    (7700.0, 9500.0, 1.2),
    (9500.0, 12000.0, 1.0),
    (12000.0, 15500.0, 0.9),
];

const TRANSITION_BINS: usize = 10;
const EPSILON: f32 = 1e-10;
const WARMUP_FRAMES: usize = 20;

// Shorthand constants for internal use
const LAMBDA: f32 = DEFAULT_LAMBDA;
const SPIKE_THRESHOLD: f32 = DEFAULT_SPIKE_THRESHOLD;
const SFM_SPEECH: f32 = DEFAULT_SFM_SPEECH;
const SFM_NOISE: f32 = DEFAULT_SFM_NOISE;

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
    pub gamma: [f32; 24],
}

pub const PRESETS: [Preset; 5] = [
    // 1: Gentle - minimal processing, preserve everything
    Preset {
        name: "Gentle",
        alpha_base: 1.5,
        alpha_min: 0.5,
        alpha_max: 2.5,
        beta: 0.15,
        gamma: [
            0.40, 0.42, 0.44, 0.46, 0.48, 0.50, 0.52,  // Low (7)
            0.55, 0.58, 0.62, 0.65, 0.68, 0.70, 0.72,  // Mid (7)
            0.74, 0.76, 0.77, 0.78, 0.79,              // High-mid (5)
            0.80, 0.81, 0.82, 0.83, 0.84,              // High (5)
        ],
    },
    // 2: Light - subtle noise reduction
    Preset {
        name: "Light",
        alpha_base: 2.0,
        alpha_min: 0.8,
        alpha_max: 3.5,
        beta: 0.08,
        gamma: [
            0.45, 0.47, 0.49, 0.51, 0.53, 0.55, 0.57,  // Low (7)
            0.60, 0.63, 0.67, 0.70, 0.73, 0.75, 0.77,  // Mid (7)
            0.79, 0.81, 0.82, 0.83, 0.84,              // High-mid (5)
            0.85, 0.86, 0.87, 0.88, 0.89,              // High (5)
        ],
    },
    // 3: Moderate - balanced (default)
    Preset {
        name: "Moderate",
        alpha_base: 3.0,
        alpha_min: 1.0,
        alpha_max: 5.0,
        beta: 0.05,
        gamma: [
            0.50, 0.52, 0.54, 0.56, 0.58, 0.60, 0.62,  // Low (7)
            0.65, 0.68, 0.72, 0.75, 0.78, 0.80, 0.82,  // Mid (7)
            0.84, 0.86, 0.87, 0.88, 0.89,              // High-mid (5)
            0.90, 0.91, 0.92, 0.93, 0.94,              // High (5)
        ],
    },
    // 4: Strong - noticeable noise reduction
    Preset {
        name: "Strong",
        alpha_base: 4.5,
        alpha_min: 2.5,
        alpha_max: 7.0,
        beta: 0.02,
        gamma: [
            0.55, 0.57, 0.59, 0.61, 0.63, 0.65, 0.67,  // Low (7)
            0.70, 0.73, 0.77, 0.80, 0.83, 0.85, 0.87,  // Mid (7)
            0.89, 0.91, 0.92, 0.93, 0.94,              // High-mid (5)
            0.95, 0.96, 0.97, 0.97, 0.98,              // High (5)
        ],
    },
    // 5: Aggressive - maximum removal, may affect speech
    Preset {
        name: "Aggressive",
        alpha_base: 6.0,
        alpha_min: 2.0,
        alpha_max: 10.0,
        beta: 0.008,
        gamma: [
            0.60, 0.62, 0.64, 0.66, 0.68, 0.70, 0.72,  // Low (7)
            0.75, 0.78, 0.82, 0.85, 0.88, 0.90, 0.92,  // Mid (7)
            0.94, 0.95, 0.96, 0.96, 0.97,              // High-mid (5)
            0.97, 0.98, 0.98, 0.99, 0.99,              // High (5)
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

fn compute_gamma_curve(fft_size: usize, sample_rate: u32, gamma_per_band: &[f32; 24]) -> Vec<f32> {
    let n_bins = fft_size / 2 + 1;
    let mut gamma = vec![gamma_per_band[23]; n_bins]; // Default to last band

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
    needs_initialization: bool,

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
            noise_pow: vec![EPSILON; n_bins],  // Small non-zero placeholder
            prev_gain: vec![1.0; n_bins],
            prev_sfm_decision: true,
            frame_count: 0,
            needs_initialization: true,  // Will initialize from first frame
            gamma: compute_gamma_curve(window_size, sample_rate, &p.gamma),
            overlap_buffer: vec![0.0; window_size],
            fft,
            ifft,
            fft_scratch,
        }
    }

    /// Initialize with pre-computed noise floor from Pass 1 analysis
    pub fn init_with_noise_floor(&mut self, noise_floor: &[f32]) {
        assert_eq!(noise_floor.len(), self.n_bins, "Noise floor size mismatch");
        self.noise_pow.copy_from_slice(noise_floor);
        self.needs_initialization = false;
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

    fn update_noise_estimate_with_lambda(&mut self, power: &[f32], force_update: bool, lambda: f32) {
        for k in 0..self.n_bins {
            // Spike protection
            if !force_update && power[k] > SPIKE_THRESHOLD * self.noise_pow[k] {
                continue;
            }
            // Recursive update with custom lambda
            self.noise_pow[k] = lambda * self.noise_pow[k] + (1.0 - lambda) * power[k];
        }
    }

    fn initialize_noise_from_first_frame(&mut self, power: &[f32]) {
        // Use bottom 20th percentile of first frame as initial guess
        let mut sorted = power.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let percentile_20 = sorted[sorted.len() / 5];

        for k in 0..self.n_bins {
            // Initialize conservatively: minimum of bin power or 20th percentile * 1.5
            self.noise_pow[k] = power[k].min(percentile_20 * 1.5).max(EPSILON);
        }
        self.needs_initialization = false;
    }

    fn get_adaptive_lambda(&self) -> f32 {
        // Fast convergence in first few frames
        if self.frame_count < 5 {
            0.5   // Very fast initial convergence
        } else if self.frame_count < WARMUP_FRAMES {
            0.75  // Medium convergence
        } else {
            LAMBDA  // Normal 0.95 - maintains adaptation
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

        // Initialize noise estimate from first frame
        if self.needs_initialization {
            self.initialize_noise_from_first_frame(&power);
        }

        // Get adaptive lambda for faster initial convergence
        let lambda = self.get_adaptive_lambda();

        let enhanced_spectrum = if self.frame_count < WARMUP_FRAMES {
            // During warmup: use fast convergence lambda, but apply full denoising
            // Update noise estimate with adaptive lambda for fast convergence
            self.update_noise_estimate_with_lambda(&power, true, lambda);

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

            // Compute and smooth gain - apply at full strength from frame 0
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
        } else {
            // Normal processing after warmup
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
}

// =============================================================================
// Analysis: Minimum Statistics for Noise Floor
// =============================================================================

/// Compute noise floor using minimum statistics
/// Tracks minimum power per bin over 1.5 second sliding windows
pub fn compute_minimum_statistics(audio: &[f32], sample_rate: u32) -> Vec<f32> {
    let n_bins = WINDOW_SIZE / 2 + 1;
    let window_duration_seconds = 1.5;
    let frames_per_window = ((sample_rate as f32 * window_duration_seconds) / HOP_SIZE as f32) as usize;

    // FFT setup
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);
    let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

    // Hann window
    let window: Vec<f32> = (0..WINDOW_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (WINDOW_SIZE - 1) as f32).cos()))
        .collect();

    // Pad audio
    let pre_pad = WINDOW_SIZE - HOP_SIZE;
    let mut input = vec![0.0; pre_pad];
    input.extend_from_slice(audio);

    // Collect power spectra from all frames
    let mut all_power_spectra: Vec<Vec<f32>> = Vec::new();

    let mut i = 0;
    while i + WINDOW_SIZE <= input.len() {
        let frame = &input[i..i + WINDOW_SIZE];

        // Window and FFT
        let windowed: Vec<f32> = frame
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        let mut spectrum: Vec<Complex<f32>> = windowed
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();

        fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

        // Power spectrum
        let power: Vec<f32> = spectrum[..n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        all_power_spectra.push(power);
        i += HOP_SIZE;
    }

    if all_power_spectra.is_empty() {
        return vec![EPSILON; n_bins];
    }

    // Compute minimum statistics per bin
    let mut noise_floor = vec![f32::INFINITY; n_bins];

    for window_start in 0..all_power_spectra.len() {
        let window_end = (window_start + frames_per_window).min(all_power_spectra.len());

        for bin in 0..n_bins {
            let mut window_min = f32::INFINITY;
            for frame_idx in window_start..window_end {
                window_min = window_min.min(all_power_spectra[frame_idx][bin]);
            }
            noise_floor[bin] = noise_floor[bin].min(window_min);
        }
    }

    // Ensure no zeros or infinities
    for bin in 0..n_bins {
        noise_floor[bin] = noise_floor[bin].max(EPSILON);
    }

    noise_floor
}

// =============================================================================
// Analysis: Simple Audio Metrics
// =============================================================================

/// Simple audio analysis for displaying recommendations
pub struct SimpleAnalysis {
    pub overall_snr_db: f32,
    pub stationarity_score: f32,  // 0.0 = variable, 1.0 = constant
    pub speech_density: f32,       // 0.0-1.0
    pub dominant_freq_hz: f32,     // Where most noise energy is
}

/// Estimate overall SNR in dB
fn estimate_overall_snr(audio: &[f32], noise_floor: &[f32]) -> f32 {
    let n_bins = WINDOW_SIZE / 2 + 1;

    // FFT setup
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);
    let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

    // Hann window
    let window: Vec<f32> = (0..WINDOW_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (WINDOW_SIZE - 1) as f32).cos()))
        .collect();

    // Pad audio
    let pre_pad = WINDOW_SIZE - HOP_SIZE;
    let mut input = vec![0.0; pre_pad];
    input.extend_from_slice(audio);

    // Compute average signal power across all frames
    let mut total_signal_power = 0.0;
    let mut frame_count = 0;

    let mut i = 0;
    while i + WINDOW_SIZE <= input.len() {
        let frame = &input[i..i + WINDOW_SIZE];

        // Window and FFT
        let windowed: Vec<f32> = frame
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        let mut spectrum: Vec<Complex<f32>> = windowed
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();

        fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

        // Power spectrum
        let power: Vec<f32> = spectrum[..n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        total_signal_power += power.iter().sum::<f32>();
        frame_count += 1;
        i += HOP_SIZE;
    }

    if frame_count == 0 {
        return 0.0;
    }

    let avg_signal_power = total_signal_power / (frame_count * n_bins) as f32;
    let avg_noise_power: f32 = noise_floor.iter().sum::<f32>() / noise_floor.len() as f32;

    let snr_linear = avg_signal_power / (avg_noise_power + EPSILON);
    10.0 * snr_linear.max(EPSILON).log10()
}

/// Estimate stationarity (how constant the noise is over time)
fn estimate_stationarity(audio: &[f32]) -> f32 {
    let n_bins = WINDOW_SIZE / 2 + 1;

    // FFT setup
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);
    let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

    // Hann window
    let window: Vec<f32> = (0..WINDOW_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (WINDOW_SIZE - 1) as f32).cos()))
        .collect();

    // Pad audio
    let pre_pad = WINDOW_SIZE - HOP_SIZE;
    let mut input = vec![0.0; pre_pad];
    input.extend_from_slice(audio);

    // Collect power spectra for noise-like frames (SFM > threshold)
    let mut noise_powers: Vec<f32> = Vec::new();

    let mut i = 0;
    while i + WINDOW_SIZE <= input.len() {
        let frame = &input[i..i + WINDOW_SIZE];

        // Window and FFT
        let windowed: Vec<f32> = frame
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        let mut spectrum: Vec<Complex<f32>> = windowed
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();

        fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

        // Power spectrum
        let power: Vec<f32> = spectrum[..n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        // Check if this is a noise-like frame
        let sfm = compute_sfm(&power);
        if sfm > SFM_NOISE {
            noise_powers.push(power.iter().sum::<f32>());
        }

        i += HOP_SIZE;
    }

    if noise_powers.len() < 2 {
        return 0.5;  // Not enough data
    }

    // Compute coefficient of variation (std/mean)
    let mean: f32 = noise_powers.iter().sum::<f32>() / noise_powers.len() as f32;
    let variance: f32 = noise_powers.iter()
        .map(|&p| (p - mean).powi(2))
        .sum::<f32>() / noise_powers.len() as f32;
    let std_dev = variance.sqrt();

    let cv = std_dev / (mean + EPSILON);

    // Convert to stationarity score (0 = variable, 1 = constant)
    // Lower CV = more stationary
    (1.0 - cv.min(1.0)).max(0.0)
}

/// Estimate speech density (percentage of frames that are speech-like)
fn estimate_speech_density(audio: &[f32]) -> f32 {
    let n_bins = WINDOW_SIZE / 2 + 1;

    // FFT setup
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);
    let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

    // Hann window
    let window: Vec<f32> = (0..WINDOW_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (WINDOW_SIZE - 1) as f32).cos()))
        .collect();

    // Pad audio
    let pre_pad = WINDOW_SIZE - HOP_SIZE;
    let mut input = vec![0.0; pre_pad];
    input.extend_from_slice(audio);

    let mut speech_frame_count = 0;
    let mut total_frame_count = 0;

    let mut i = 0;
    while i + WINDOW_SIZE <= input.len() {
        let frame = &input[i..i + WINDOW_SIZE];

        // Window and FFT
        let windowed: Vec<f32> = frame
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        let mut spectrum: Vec<Complex<f32>> = windowed
            .iter()
            .map(|&s| Complex::new(s, 0.0))
            .collect();

        fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

        // Power spectrum
        let power: Vec<f32> = spectrum[..n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        // Check if this is a speech-like frame (low SFM = tonal)
        let sfm = compute_sfm(&power);
        if sfm < SFM_SPEECH {
            speech_frame_count += 1;
        }

        total_frame_count += 1;
        i += HOP_SIZE;
    }

    if total_frame_count == 0 {
        return 0.0;
    }

    speech_frame_count as f32 / total_frame_count as f32
}

/// Find the dominant frequency in the noise floor
fn find_dominant_noise_freq(noise_floor: &[f32], sample_rate: u32) -> f32 {
    // Find the bin with maximum noise power
    let max_bin = noise_floor
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    // Convert bin to frequency
    max_bin as f32 * sample_rate as f32 / WINDOW_SIZE as f32
}

/// Analyze audio and return simple metrics
pub fn analyze_audio_simple(
    audio: &[f32],
    sample_rate: u32,
    noise_floor: &[f32],
) -> SimpleAnalysis {
    let overall_snr_db = estimate_overall_snr(audio, noise_floor);
    let stationarity_score = estimate_stationarity(audio);
    let speech_density = estimate_speech_density(audio);
    let dominant_freq_hz = find_dominant_noise_freq(noise_floor, sample_rate);

    SimpleAnalysis {
        overall_snr_db,
        stationarity_score,
        speech_density,
        dominant_freq_hz,
    }
}

// =============================================================================
// Analysis: Recommendations
// =============================================================================

/// Recommend a preset based on audio analysis
pub fn recommend_preset(analysis: &SimpleAnalysis) -> usize {
    // Simple heuristic based on SNR
    if analysis.overall_snr_db < 0.0 {
        5  // Aggressive
    } else if analysis.overall_snr_db < 10.0 {
        4  // Strong
    } else if analysis.overall_snr_db < 15.0 {
        3  // Moderate
    } else if analysis.overall_snr_db < 20.0 {
        2  // Light
    } else {
        1  // Gentle
    }
}

/// Recommend thresholds based on audio analysis
/// Returns: (sfm_speech_threshold, sfm_noise_threshold, spike_threshold)
pub fn recommend_thresholds(analysis: &SimpleAnalysis) -> (f32, f32, f32) {
    let sfm_speech = if analysis.speech_density > 0.7 {
        0.15  // Higher threshold for speech-heavy content
    } else {
        0.10  // Standard threshold
    };

    let sfm_noise = if analysis.stationarity_score > 0.8 {
        0.35  // Lower for constant noise
    } else {
        0.45  // Higher for varying noise
    };

    let spike_threshold = if analysis.overall_snr_db < 10.0 {
        12.0  // More aggressive for noisy audio
    } else {
        10.0  // Standard threshold
    };

    (sfm_speech, sfm_noise, spike_threshold)
}

// =============================================================================
// Stereo Processing (M/S)
// =============================================================================

pub fn process_stereo(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    preset: usize,
    noise_floor: Option<&[f32]>,
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

    // Initialize with noise floor if provided
    if let Some(nf) = noise_floor {
        denoiser_mid.init_with_noise_floor(nf);
        denoiser_side.init_with_noise_floor(nf);
    }

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

/// Process stereo by processing left and right channels independently
pub fn process_stereo_lr(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    preset: usize,
    noise_floor: Option<&[f32]>,
) -> (Vec<f32>, Vec<f32>) {
    // Process each channel independently
    let mut denoiser_left = SpectralSubtractionDenoiser::new(sample_rate, preset);
    let mut denoiser_right = SpectralSubtractionDenoiser::new(sample_rate, preset);

    // Initialize with noise floor if provided
    if let Some(nf) = noise_floor {
        denoiser_left.init_with_noise_floor(nf);
        denoiser_right.init_with_noise_floor(nf);
    }

    let left_out = denoiser_left.process(left);
    let right_out = denoiser_right.process(right);

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
