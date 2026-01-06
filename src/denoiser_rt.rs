//! Real-time Spectral Subtraction Denoiser
//!
//! Optimized for plugin use with all parameters exposed

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;
use std::sync::Arc;

pub const WINDOW_SIZE: usize = 2048;
pub const HOP_SIZE: usize = 1024;

// Band configuration (Hz ranges)
pub const BANDS: [(f32, f32); 9] = [
    (0.0, 80.0),        // Band 0: Rumble
    (80.0, 250.0),      // Band 1: Fundamental
    (250.0, 500.0),     // Band 2: Warmth
    (500.0, 1000.0),    // Band 3: Body
    (1000.0, 2000.0),   // Band 4: Presence
    (2000.0, 4000.0),   // Band 5: Intelligibility
    (4000.0, 8000.0),   // Band 6: Sibilance
    (8000.0, 12000.0),  // Band 7: Air
    (12000.0, 24000.0), // Band 8: Hiss
];

pub const NUM_BANDS: usize = 9;
const TRANSITION_BINS: usize = 10;
const EPSILON: f32 = 1e-10;

// =============================================================================
// Parameter Defaults
// =============================================================================

pub const DEFAULT_ALPHA_BASE: f32 = 3.0;
pub const DEFAULT_ALPHA_MIN: f32 = 1.0;
pub const DEFAULT_ALPHA_MAX: f32 = 5.0;
pub const DEFAULT_BETA: f32 = 0.05;

pub const DEFAULT_LAMBDA: f32 = 0.95;
pub const DEFAULT_SPIKE_THRESHOLD: f32 = 10.0;
pub const DEFAULT_SFM_SPEECH: f32 = 0.1;
pub const DEFAULT_SFM_NOISE: f32 = 0.4;

pub const DEFAULT_DELTA: [f32; NUM_BANDS] = [0.8, 1.0, 1.5, 2.0, 2.5, 2.5, 2.0, 1.5, 1.0];
pub const DEFAULT_GAMMA: [f32; NUM_BANDS] = [0.50, 0.55, 0.65, 0.72, 0.78, 0.82, 0.86, 0.90, 0.92];

// =============================================================================
// Processing Parameters (updateable in real-time)
// =============================================================================

#[derive(Clone, Debug)]
pub struct DenoiserParams {
    // Subtraction
    pub alpha_base: f32,
    pub alpha_min: f32,
    pub alpha_max: f32,
    pub beta: f32,

    // Noise estimation
    pub lambda: f32,
    pub spike_threshold: f32,
    pub sfm_speech: f32,
    pub sfm_noise: f32,

    // Per-band parameters
    pub delta: [f32; NUM_BANDS],
    pub gamma: [f32; NUM_BANDS],
}

impl Default for DenoiserParams {
    fn default() -> Self {
        Self {
            alpha_base: DEFAULT_ALPHA_BASE,
            alpha_min: DEFAULT_ALPHA_MIN,
            alpha_max: DEFAULT_ALPHA_MAX,
            beta: DEFAULT_BETA,
            lambda: DEFAULT_LAMBDA,
            spike_threshold: DEFAULT_SPIKE_THRESHOLD,
            sfm_speech: DEFAULT_SFM_SPEECH,
            sfm_noise: DEFAULT_SFM_NOISE,
            delta: DEFAULT_DELTA,
            gamma: DEFAULT_GAMMA,
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn root_hann_window(n: usize) -> Vec<f32> {
    (0..n).map(|i| (PI * i as f32 / n as f32).sin()).collect()
}

fn compute_sfm(power_spectrum: &[f32]) -> f32 {
    let n = power_spectrum.len() as f32;
    let log_sum: f32 = power_spectrum.iter().map(|&p| (p + EPSILON).ln()).sum();
    let geo_mean = (log_sum / n).exp();
    let arith_mean: f32 = power_spectrum.iter().sum::<f32>() / n;
    geo_mean / (arith_mean + EPSILON)
}

fn hz_to_bin(hz: f32, fft_size: usize, sample_rate: u32) -> usize {
    (hz * fft_size as f32 / sample_rate as f32) as usize
}

fn compute_alpha_curve(
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

fn compute_gamma_curve(
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

// =============================================================================
// Real-time Denoiser
// =============================================================================

pub struct RealtimeDenoiser {
    sample_rate: u32,
    window_size: usize,
    hop_size: usize,
    n_bins: usize,

    // Parameters (can be updated)
    params: DenoiserParams,

    // Window
    window: Vec<f32>,

    // State
    noise_pow: Vec<f32>,
    prev_gain: Vec<f32>,
    prev_sfm_decision: bool,
    frames_processed: usize,

    // Cached gamma curve (rebuild when params change)
    gamma_curve: Vec<f32>,
    gamma_dirty: bool,

    // Overlap buffer
    overlap_buffer: Vec<f32>,
    input_buffer: Vec<f32>,

    // FFT
    fft: Arc<dyn rustfft::Fft<f32>>,
    ifft: Arc<dyn rustfft::Fft<f32>>,
    fft_scratch: Vec<Complex<f32>>,
}

impl RealtimeDenoiser {
    pub fn new(sample_rate: u32) -> Self {
        let window_size = WINDOW_SIZE;
        let hop_size = HOP_SIZE;
        let n_bins = window_size / 2 + 1;

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(window_size);
        let ifft = planner.plan_fft_inverse(window_size);
        let fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

        let params = DenoiserParams::default();
        let gamma_curve = compute_gamma_curve(window_size, sample_rate, &params.gamma);

        Self {
            sample_rate,
            window_size,
            hop_size,
            n_bins,
            params,
            window: root_hann_window(window_size),
            noise_pow: vec![EPSILON; n_bins], // Initialize with small value instead of 0
            prev_gain: vec![1.0; n_bins],
            prev_sfm_decision: true,
            frames_processed: 0,
            gamma_curve,
            gamma_dirty: false,
            overlap_buffer: vec![0.0; window_size],
            input_buffer: Vec::new(),
            fft,
            ifft,
            fft_scratch,
        }
    }

    pub fn set_params(&mut self, params: DenoiserParams) {
        self.params = params;
        self.gamma_dirty = true;
    }

    fn rebuild_gamma_curve(&mut self) {
        if self.gamma_dirty {
            self.gamma_curve = compute_gamma_curve(
                self.window_size,
                self.sample_rate,
                &self.params.gamma,
            );
            self.gamma_dirty = false;
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

    fn update_noise_estimate(&mut self, power: &[f32]) {
        for k in 0..self.n_bins {
            // Spike protection
            if power[k] > self.params.spike_threshold * self.noise_pow[k] {
                continue;
            }
            // Recursive update
            self.noise_pow[k] = self.params.lambda * self.noise_pow[k]
                + (1.0 - self.params.lambda) * power[k];
        }
    }

    fn compute_gain(&self, power: &[f32], alpha: &[f32]) -> Vec<f32> {
        power
            .iter()
            .zip(alpha.iter())
            .zip(self.noise_pow.iter())
            .map(|((&p, &a), &n)| {
                let subtracted = p - a * n;
                let floored = self.params.beta * p;
                let numerator = subtracted.max(floored);
                let gain = (numerator / (p + EPSILON)).sqrt();
                gain.clamp(0.0, 1.0)
            })
            .collect()
    }

    fn smooth_gain(&mut self, gain: &[f32]) -> Vec<f32> {
        self.rebuild_gamma_curve();

        let smoothed: Vec<f32> = self.gamma_curve
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

        // Compute power spectrum
        let power: Vec<f32> = spectrum[..self.n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        // Bootstrap noise estimate with first 10 frames
        if self.frames_processed < 10 {
            // Force update during initial frames to learn noise floor
            for k in 0..self.n_bins {
                self.noise_pow[k] = self.params.lambda * self.noise_pow[k]
                    + (1.0 - self.params.lambda) * power[k] * 0.5; // Use 50% of signal as initial estimate
            }
            self.frames_processed += 1;
        } else {
            // Normal SFM-based VAD for noise estimation
            let sfm = compute_sfm(&power);

            if sfm > self.params.sfm_noise {
                self.update_noise_estimate(&power);
                self.prev_sfm_decision = true;
            } else if sfm < self.params.sfm_speech {
                self.prev_sfm_decision = false;
            } else if self.prev_sfm_decision {
                self.update_noise_estimate(&power);
            }
        }

        // Compute adaptive alpha
        let snr = self.compute_snr_per_bin(&power);
        let alpha = compute_alpha_curve(
            self.window_size,
            self.sample_rate,
            &snr,
            &self.params.delta,
            self.params.alpha_base,
            self.params.alpha_min,
            self.params.alpha_max,
        );

        // Compute and smooth gain
        let gain = self.compute_gain(&power, &alpha);
        let gain = self.smooth_gain(&gain);

        // Apply gain to spectrum (maintain conjugate symmetry)
        let mut result = spectrum.clone();
        for k in 0..self.n_bins {
            result[k] = spectrum[k] * gain[k];
        }
        for k in self.n_bins..self.window_size {
            let mirror = self.window_size - k;
            result[k] = result[mirror].conj();
        }

        // Inverse FFT
        self.ifft.process_with_scratch(&mut result, &mut self.fft_scratch);

        // Normalize and extract real part
        let scale = 1.0 / self.window_size as f32;
        let mut enhanced: Vec<f32> = result.iter().map(|c| c.re * scale).collect();

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

        output
    }

    pub fn reset(&mut self) {
        self.noise_pow.fill(EPSILON);
        self.prev_gain.fill(1.0);
        self.prev_sfm_decision = true;
        self.frames_processed = 0;
        self.overlap_buffer.fill(0.0);
        self.input_buffer.clear();
    }

    pub fn latency_samples(&self) -> u32 {
        // The latency is the window size minus the hop size
        // This is the lookahead needed for the STFT
        (self.window_size - self.hop_size) as u32
    }
}
