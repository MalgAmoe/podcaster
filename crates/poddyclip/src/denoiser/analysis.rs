//! Audio Analysis Functions for CLI
//!
//! Provides noise floor estimation and audio metrics for batch processing.
//! These functions are CLI-only (plugin uses real-time adaptation).

#![allow(dead_code)]

use super::common::*;
use crate::stft::{StftProcessor, RT_HOP_SIZE, RT_WINDOW_SIZE, EPSILON};

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
    max_bin as f32 * sample_rate as f32 / RT_WINDOW_SIZE as f32
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
    let mut stft = StftProcessor::new_realtime(sample_rate);
    let window_size = stft.window_size();
    let hop_size = stft.hop_size();
    let n_bins = stft.n_bins();

    // Pad audio
    let pre_pad = window_size - hop_size;
    let mut input = vec![0.0; pre_pad];
    input.extend_from_slice(audio);

    // ONE FFT PASS - collect all power spectra
    let mut all_power_spectra = Vec::new();
    let mut i = 0;
    while i + window_size <= input.len() {
        let frame = &input[i..i + window_size];

        // Forward FFT (applies window internally)
        let spectrum = stft.forward_fft(frame);

        // Compute power spectrum
        let power = stft.compute_power(&spectrum);
        all_power_spectra.push(power);
        i += hop_size;
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
        ((sample_rate as f32 * window_duration_seconds) / RT_HOP_SIZE as f32) as usize;

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
