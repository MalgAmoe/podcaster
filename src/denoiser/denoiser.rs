//! Spectral Subtraction Noise Reduction
//!
//! Rust port of the Python validation implementation.
//! Minimal dependencies: rustfft for FFT operations.

use super::common::*;
use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;

// =============================================================================
// Constants (from spec)
// =============================================================================

// Reference sample rate - code works with any sample rate.
// The denoiser accepts sample_rate as a parameter and scales frequency-dependent
// calculations (Bark bands, analysis windows) appropriately.
pub const SAMPLE_RATE: u32 = 48000;

// =============================================================================
// Preset Configuration
// =============================================================================
// Note: Preset struct and PRESETS array are now in common.rs (shared with plugin)

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
    delta: [f32; NUM_BANDS],

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
            delta: p.delta,
            window: root_hann_window(window_size),
            noise_pow: vec![EPSILON; n_bins], // Small non-zero placeholder
            prev_gain: vec![1.0; n_bins],
            prev_sfm_decision: true,
            frame_count: 0,
            needs_initialization: true, // Will initialize from first frame
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
            if !force_update && power[k] > DEFAULT_SPIKE_THRESHOLD * self.noise_pow[k] {
                continue;
            }
            // Recursive update
            self.noise_pow[k] =
                DEFAULT_LAMBDA * self.noise_pow[k] + (1.0 - DEFAULT_LAMBDA) * power[k];
        }
    }

    fn update_noise_estimate_with_lambda(
        &mut self,
        power: &[f32],
        force_update: bool,
        lambda: f32,
    ) {
        for k in 0..self.n_bins {
            // Spike protection
            if !force_update && power[k] > DEFAULT_SPIKE_THRESHOLD * self.noise_pow[k] {
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
            0.5 // Very fast initial convergence
        } else if self.frame_count < WARMUP_FRAMES {
            0.75 // Medium convergence
        } else {
            DEFAULT_LAMBDA // Normal 0.95 - maintains adaptation
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
        let smoothed: Vec<f32> = self
            .gamma
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
        let mut spectrum: Vec<Complex<f32>> =
            windowed.iter().map(|&s| Complex::new(s, 0.0)).collect();
        self.fft
            .process_with_scratch(&mut spectrum, &mut self.fft_scratch);

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
                &self.delta,
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

            if sfm > DEFAULT_SFM_NOISE {
                self.update_noise_estimate(&power, false);
                self.prev_sfm_decision = true;
            } else if sfm < DEFAULT_SFM_SPEECH {
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
                &self.delta,
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
        self.ifft
            .process_with_scratch(&mut time_domain, &mut self.fft_scratch);

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
        let mut output: Vec<f32> = self.overlap_buffer[..self.hop_size].to_vec();

        // Apply fade-in during warmup to hide artifacts
        if self.frame_count < WARMUP_FRAMES {
            let fade = self.frame_count as f32 / WARMUP_FRAMES as f32;
            for sample in output.iter_mut() {
                *sample *= fade * fade;
            }
        }

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
// Analysis: Simple Audio Metrics
// =============================================================================

/// Simple audio analysis for displaying recommendations
pub struct SimpleAnalysis {
    pub overall_snr_db: f32,
    pub stationarity_score: f32, // 0.0 = variable, 1.0 = constant
    pub speech_density: f32,     // 0.0-1.0
    pub dominant_freq_hz: f32,   // Where most noise energy is
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

// =============================================================================
// Unified Analysis (Single FFT Pass)
// =============================================================================

/// Result from unified audio analysis
pub struct AudioAnalysisResult {
    pub noise_floor: Vec<f32>,
    pub analysis: SimpleAnalysis,
}

/// Analyze audio with ONE FFT pass (replaces compute_minimum_statistics + analyze_audio_simple)
pub fn analyze_audio(audio: &[f32], sample_rate: u32) -> AudioAnalysisResult {
    let n_bins = WINDOW_SIZE / 2 + 1;

    // FFT setup (once!)
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);
    let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

    // Hann window (once!)
    let window: Vec<f32> = (0..WINDOW_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (WINDOW_SIZE - 1) as f32).cos()))
        .collect();

    // Pad audio
    let pre_pad = WINDOW_SIZE - HOP_SIZE;
    let mut input = vec![0.0; pre_pad];
    input.extend_from_slice(audio);

    // ONE FFT PASS - collect all power spectra
    let mut all_power_spectra = Vec::new();
    let mut i = 0;
    while i + WINDOW_SIZE <= input.len() {
        let frame = &input[i..i + WINDOW_SIZE];

        let windowed: Vec<f32> = frame
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        let mut spectrum: Vec<Complex<f32>> =
            windowed.iter().map(|&s| Complex::new(s, 0.0)).collect();

        fft.process_with_scratch(&mut spectrum, &mut fft_scratch);

        let power: Vec<f32> = spectrum[..n_bins].iter().map(|c| c.norm_sqr()).collect();
        all_power_spectra.push(power);
        i += HOP_SIZE;
    }

    // Now compute everything from the cached power spectra (no more FFTs!)

    // 1. Noise floor (minimum statistics)
    let noise_floor = compute_noise_floor_from_spectra(&all_power_spectra, sample_rate, n_bins);

    // 2. SNR
    let overall_snr_db = compute_snr_from_spectra(&all_power_spectra, &noise_floor, n_bins);

    // 3. Stationarity
    let stationarity_score = compute_stationarity_from_spectra(&all_power_spectra);

    // 4. Speech density
    let speech_density = compute_speech_density_from_spectra(&all_power_spectra);

    // 5. Dominant frequency
    let dominant_freq_hz = find_dominant_noise_freq(&noise_floor, sample_rate);

    AudioAnalysisResult {
        noise_floor,
        analysis: SimpleAnalysis {
            overall_snr_db,
            stationarity_score,
            speech_density,
            dominant_freq_hz,
        },
    }
}

// Helper functions that work on cached spectra (no FFT!)

fn compute_noise_floor_from_spectra(
    all_power_spectra: &[Vec<f32>],
    sample_rate: u32,
    n_bins: usize,
) -> Vec<f32> {
    if all_power_spectra.is_empty() {
        return vec![EPSILON; n_bins];
    }

    // 1.5 second sliding windows
    let window_duration_seconds = 1.5;
    let frames_per_window =
        ((sample_rate as f32 * window_duration_seconds) / HOP_SIZE as f32) as usize;

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
    for val in noise_floor.iter_mut() {
        *val = val.max(EPSILON);
    }

    noise_floor
}

fn compute_snr_from_spectra(
    all_power_spectra: &[Vec<f32>],
    noise_floor: &[f32],
    n_bins: usize,
) -> f32 {
    if all_power_spectra.is_empty() {
        return 0.0;
    }

    let mut total_signal_power = 0.0;
    let mut total_noise_power = 0.0;

    for power_spectrum in all_power_spectra {
        for (bin, &power) in power_spectrum.iter().enumerate() {
            total_signal_power += power;
            total_noise_power += noise_floor[bin];
        }
    }

    let avg_signal = total_signal_power / (all_power_spectra.len() * n_bins) as f32;
    let avg_noise = total_noise_power / (all_power_spectra.len() * n_bins) as f32;

    10.0 * ((avg_signal / (avg_noise + EPSILON)).max(EPSILON)).log10()
}

fn compute_stationarity_from_spectra(all_power_spectra: &[Vec<f32>]) -> f32 {
    let mut noise_powers = Vec::new();

    for power_spectrum in all_power_spectra {
        let sfm = compute_sfm(power_spectrum);
        if sfm > DEFAULT_SFM_NOISE {
            noise_powers.push(power_spectrum.iter().sum::<f32>());
        }
    }

    if noise_powers.len() < 2 {
        return 0.5;
    }

    let mean: f32 = noise_powers.iter().sum::<f32>() / noise_powers.len() as f32;
    let variance: f32 = noise_powers
        .iter()
        .map(|&p| (p - mean).powi(2))
        .sum::<f32>()
        / noise_powers.len() as f32;
    let std_dev = variance.sqrt();
    let cv = std_dev / (mean + EPSILON);

    (1.0 - cv.min(1.0)).max(0.0)
}

fn compute_speech_density_from_spectra(all_power_spectra: &[Vec<f32>]) -> f32 {
    if all_power_spectra.is_empty() {
        return 0.0;
    }

    let speech_count = all_power_spectra
        .iter()
        .filter(|power| compute_sfm(power) < DEFAULT_SFM_SPEECH)
        .count();

    speech_count as f32 / all_power_spectra.len() as f32
}

// =============================================================================
// Stereo Processing (L/R)
// =============================================================================

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
