//! Spectral Peak Attenuation
//!
//! Detects and attenuates narrow spectral peaks (tonal noise like hum, whine).
//! Works in FFT domain - no resonance issues like time-domain notch filters.

use std::f32::consts::PI;
use std::sync::Arc;

use rustfft::{num_complex::Complex, Fft, FftPlanner};

use super::common::{root_hann_window, EPSILON, HOP_SIZE, WINDOW_SIZE};

/// Result of peak analysis - detected tonal noise peaks
#[derive(Clone, Debug, Default)]
pub struct PeakProfile {
    /// Detected peak bin indices
    pub peak_bins: Vec<usize>,
    /// Prominence in dB for each peak
    pub prominences_db: Vec<f32>,
    /// Width in bins for each peak
    pub widths: Vec<usize>,
    /// Stability score (0-1) for each peak
    pub stabilities: Vec<f32>,
}

impl PeakProfile {
    /// Get peak frequencies given sample rate
    pub fn get_frequencies(&self, sample_rate: u32) -> Vec<f32> {
        let bin_freq = sample_rate as f32 / WINDOW_SIZE as f32;
        self.peak_bins
            .iter()
            .map(|&bin| bin as f32 * bin_freq)
            .collect()
    }

    /// Check if profile has any detected peaks
    pub fn has_peaks(&self) -> bool {
        !self.peak_bins.is_empty()
    }
}

/// Parameters for spectral peak attenuation
#[derive(Clone, Debug)]
pub struct PeakAttenuatorParams {
    /// Enable/disable peak attenuation
    pub enabled: bool,
    /// Minimum prominence to consider as peak (dB)
    pub prominence_threshold_db: f32,
    /// Minimum stability score (0-1) to treat as noise
    pub stability_threshold: f32,
    /// Maximum peak width in bins (wider = likely not tonal)
    pub max_peak_width_bins: usize,
    /// Maximum attenuation (dB, negative value)
    pub max_attenuation_db: f32,
    /// Attenuation scaling factor (prominence to attenuation)
    pub attenuation_factor: f32,
    /// Neighborhood size for median calculation
    pub neighborhood_bins: usize,
    /// Temporal smoothing coefficient
    pub smoothing: f32,
}

impl Default for PeakAttenuatorParams {
    fn default() -> Self {
        Self {
            enabled: true,
            prominence_threshold_db: 6.0,
            stability_threshold: 0.7,
            max_peak_width_bins: 3,
            max_attenuation_db: -18.0,
            attenuation_factor: 0.8,
            neighborhood_bins: 5,
            smoothing: 0.8,
        }
    }
}

/// Spectral peak attenuation processor
pub struct PeakAttenuator {
    sample_rate: u32,
    n_bins: usize,

    // Parameters
    params: PeakAttenuatorParams,

    // Learned peak profile (from analysis phase)
    peak_profile: Option<PeakProfile>,

    // Pre-computed static attenuation gains (from profile)
    static_attenuation: Vec<f32>,

    // Smoothed gains for temporal consistency
    smoothed_gain: Vec<f32>,

    // FFT infrastructure
    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    window: Vec<f32>,
    overlap_buffer: Vec<f32>,
}

impl PeakAttenuator {
    /// Create new peak attenuator
    pub fn new(sample_rate: u32) -> Self {
        Self::new_with_params(sample_rate, PeakAttenuatorParams::default())
    }

    /// Create with custom params
    pub fn new_with_params(sample_rate: u32, params: PeakAttenuatorParams) -> Self {
        let n_bins = WINDOW_SIZE / 2 + 1;

        // Setup FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(WINDOW_SIZE);
        let ifft = planner.plan_fft_inverse(WINDOW_SIZE);
        let scratch_len = fft.get_inplace_scratch_len().max(ifft.get_inplace_scratch_len());
        let fft_scratch = vec![Complex::new(0.0, 0.0); scratch_len];

        Self {
            sample_rate,
            n_bins,
            params,
            peak_profile: None,
            static_attenuation: vec![1.0; n_bins],
            smoothed_gain: vec![1.0; n_bins],
            fft,
            ifft,
            fft_scratch,
            window: root_hann_window(WINDOW_SIZE),
            overlap_buffer: vec![0.0; WINDOW_SIZE],
        }
    }

    /// Set parameters
    pub fn set_params(&mut self, params: PeakAttenuatorParams) {
        self.params = params;
    }

    /// Set maximum attenuation
    pub fn set_max_attenuation_db(&mut self, max_db: f32) {
        self.params.max_attenuation_db = -max_db.abs();
    }

    /// Initialize with a peak profile
    pub fn init_with_profile(&mut self, profile: PeakProfile) {
        // Pre-compute static attenuation gains from profile
        self.static_attenuation.fill(1.0);

        for (i, &bin) in profile.peak_bins.iter().enumerate() {
            if bin >= self.n_bins {
                continue;
            }

            let prominence_db = profile.prominences_db[i];
            let width = profile.widths[i];

            // Calculate attenuation based on prominence
            let attenuation_db =
                (prominence_db * self.params.attenuation_factor).min(-self.params.max_attenuation_db);
            let center_gain = 10.0f32.powf(-attenuation_db / 20.0);

            // Apply smooth transition around peak
            let half_width = (width / 2).max(1);
            let left_edge = bin.saturating_sub(half_width);
            let right_edge = (bin + half_width + 1).min(self.n_bins);

            for b in left_edge..right_edge {
                // Raised cosine window for smooth transition
                let dist = (b as i32 - bin as i32).abs() as f32;
                let t = dist / (half_width as f32 + 1.0);
                let smooth_factor = 0.5 * (1.0 + (PI * t).cos());
                let gain = 1.0 - (1.0 - center_gain) * smooth_factor;

                // Take minimum (most attenuation) if peaks overlap
                self.static_attenuation[b] = self.static_attenuation[b].min(gain);
            }
        }

        self.peak_profile = Some(profile);
    }

    /// Process entire audio buffer (batch mode, for CLI usage)
    pub fn process(&mut self, audio: &[f32]) -> Vec<f32> {
        let original_len = audio.len();

        // Pre-pad for first window overlap
        let pre_pad = WINDOW_SIZE - HOP_SIZE;

        // Pad to multiple of hop size
        let pad_len = (HOP_SIZE - audio.len() % HOP_SIZE) % HOP_SIZE;
        let mut padded = audio.to_vec();
        padded.resize(audio.len() + pad_len, 0.0);

        // Post-pad to ensure enough frames for output
        padded.resize(padded.len() + pre_pad, 0.0);

        // Add pre-pad
        let mut input = vec![0.0; pre_pad];
        input.extend(padded);

        let mut output = Vec::new();

        // Process frame by frame
        let mut i = 0;
        while i + WINDOW_SIZE <= input.len() {
            let frame = &input[i..i + WINDOW_SIZE];
            let out_frame = self.process_frame(frame);
            output.extend(out_frame);
            i += HOP_SIZE;
        }

        // Remove pre-padding and trim to original length
        if output.len() > pre_pad {
            output = output[pre_pad..].to_vec();
        }
        output.truncate(original_len);

        output
    }

    /// Process single STFT frame
    fn process_frame(&mut self, frame: &[f32]) -> Vec<f32> {
        // 1. Apply analysis window
        let windowed: Vec<f32> = frame
            .iter()
            .zip(self.window.iter())
            .map(|(&s, &w)| s * w)
            .collect();

        // 2. Forward FFT
        let mut spectrum: Vec<Complex<f32>> =
            windowed.iter().map(|&s| Complex::new(s, 0.0)).collect();
        self.fft
            .process_with_scratch(&mut spectrum, &mut self.fft_scratch);

        // 3. Compute power spectrum
        let power: Vec<f32> = spectrum[..self.n_bins]
            .iter()
            .map(|c| c.norm_sqr())
            .collect();

        // 4. Compute attenuation gains
        let gains = self.compute_attenuation_internal(&power);

        // 5. Apply gains (maintain conjugate symmetry)
        for k in 0..self.n_bins {
            spectrum[k] *= gains[k];
        }
        for k in self.n_bins..WINDOW_SIZE {
            spectrum[k] = spectrum[WINDOW_SIZE - k].conj();
        }

        // 6. Inverse FFT
        self.ifft
            .process_with_scratch(&mut spectrum, &mut self.fft_scratch);

        // 7. Apply synthesis window & normalize
        let scale = 1.0 / WINDOW_SIZE as f32;
        let enhanced: Vec<f32> = spectrum
            .iter()
            .zip(self.window.iter())
            .map(|(c, &w)| c.re * scale * w)
            .collect();

        // 8. Overlap-add
        for (i, &e) in enhanced.iter().enumerate() {
            self.overlap_buffer[i] += e;
        }
        let output: Vec<f32> = self.overlap_buffer[..HOP_SIZE].to_vec();
        self.overlap_buffer.rotate_left(HOP_SIZE);
        for i in (WINDOW_SIZE - HOP_SIZE)..WINDOW_SIZE {
            self.overlap_buffer[i] = 0.0;
        }

        output
    }

    /// Compute attenuation gains for current frame (internal)
    fn compute_attenuation_internal(&mut self, _power: &[f32]) -> Vec<f32> {
        if !self.params.enabled {
            return vec![1.0; self.n_bins];
        }

        let smoothing = self.params.smoothing;
        let mut gains = vec![1.0; self.n_bins];

        for k in 0..self.n_bins {
            // Use pre-computed static attenuation from profile
            let target_gain = self.static_attenuation[k];

            // Temporal smoothing
            self.smoothed_gain[k] =
                smoothing * self.smoothed_gain[k] + (1.0 - smoothing) * target_gain;

            gains[k] = self.smoothed_gain[k];
        }

        gains
    }

    /// Get detected peak frequencies (for display)
    pub fn get_peak_frequencies(&self) -> Vec<f32> {
        self.peak_profile
            .as_ref()
            .map(|p| p.get_frequencies(self.sample_rate))
            .unwrap_or_default()
    }

    /// Check if profile has peaks
    pub fn has_peaks(&self) -> bool {
        self.peak_profile
            .as_ref()
            .map(|p| p.has_peaks())
            .unwrap_or(false)
    }

    /// Get number of detected peaks
    pub fn num_peaks(&self) -> usize {
        self.peak_profile
            .as_ref()
            .map(|p| p.peak_bins.len())
            .unwrap_or(0)
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.smoothed_gain.fill(1.0);
        self.overlap_buffer.fill(0.0);
    }

    /// Get current max attenuation for display
    pub fn get_max_attenuation_db(&self) -> f32 {
        let min_gain = self.smoothed_gain.iter().cloned().fold(1.0f32, f32::min);
        20.0 * min_gain.max(EPSILON).log10()
    }
}

// =============================================================================
// Peak Detection (for analysis phase)
// =============================================================================

/// Internal tracker for peak detection during analysis
#[derive(Clone, Default)]
struct PeakTracker {
    count: f32,
    sum_prominence: f32,
}

/// Detect tonal peaks from audio samples
///
/// # Arguments
/// * `samples` - Audio samples to analyze
/// * `sample_rate` - Sample rate in Hz
/// * `params` - Detection parameters
///
/// # Returns
/// PeakProfile with detected stable tonal peaks
pub fn detect_tonal_peaks(
    samples: &[f32],
    sample_rate: u32,
    params: &PeakAttenuatorParams,
) -> PeakProfile {
    // Extract power spectra from audio
    let power_spectra = extract_power_spectra(samples, sample_rate);

    if power_spectra.is_empty() {
        return PeakProfile::default();
    }

    let n_bins = WINDOW_SIZE / 2 + 1;
    let mut trackers: Vec<PeakTracker> = vec![PeakTracker::default(); n_bins];
    let neighborhood = params.neighborhood_bins;
    let prominence_threshold = params.prominence_threshold_db;

    // Process each frame
    for power in &power_spectra {
        for k in neighborhood..(n_bins.saturating_sub(neighborhood)).min(power.len()) {
            // Get local neighborhood (excluding center)
            let mut neighbors: Vec<f32> = Vec::with_capacity(2 * neighborhood);
            for i in (k.saturating_sub(neighborhood))..k {
                if i < power.len() {
                    neighbors.push(power[i]);
                }
            }
            for i in (k + 1)..=(k + neighborhood).min(power.len() - 1) {
                neighbors.push(power[i]);
            }

            if neighbors.is_empty() {
                continue;
            }

            // Compute median of neighbors
            neighbors.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let local_median = neighbors[neighbors.len() / 2];

            // Peak prominence
            let prominence_ratio = power[k] / (local_median + EPSILON);
            let prominence_db = 10.0 * prominence_ratio.max(EPSILON).log10();

            // Track peak if prominent
            if prominence_db > prominence_threshold {
                trackers[k].count += 1.0;
                trackers[k].sum_prominence += prominence_db;
            } else {
                // Decay count for non-prominent frames
                trackers[k].count = (trackers[k].count - 0.5).max(0.0);
            }
        }
    }

    // Build profile from stable peaks
    let total_frames = power_spectra.len() as f32;
    let mut profile = PeakProfile::default();

    for k in neighborhood..(n_bins.saturating_sub(neighborhood)) {
        let stability = trackers[k].count / total_frames;
        let avg_prominence = if trackers[k].count > 0.0 {
            trackers[k].sum_prominence / trackers[k].count
        } else {
            0.0
        };

        if stability >= params.stability_threshold && avg_prominence > prominence_threshold {
            // Estimate peak width
            let width = estimate_peak_width(&power_spectra, k, n_bins);

            // Only include narrow peaks (tonal noise)
            if width <= params.max_peak_width_bins {
                profile.peak_bins.push(k);
                profile.prominences_db.push(avg_prominence);
                profile.widths.push(width);
                profile.stabilities.push(stability);
            }
        }
    }

    profile
}

/// Extract power spectra from audio samples
fn extract_power_spectra(samples: &[f32], _sample_rate: u32) -> Vec<Vec<f32>> {
    let n_bins = WINDOW_SIZE / 2 + 1;
    let mut spectra = Vec::new();

    // Setup FFT
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);
    let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft.get_inplace_scratch_len()];

    // Window
    let window = root_hann_window(WINDOW_SIZE);

    let num_frames = samples.len().saturating_sub(WINDOW_SIZE) / HOP_SIZE + 1;
    if num_frames == 0 {
        return spectra;
    }

    for frame_idx in 0..num_frames {
        let start = frame_idx * HOP_SIZE;
        let end = (start + WINDOW_SIZE).min(samples.len());
        if end - start < WINDOW_SIZE {
            break;
        }

        // Apply window
        let mut fft_buffer: Vec<Complex<f32>> = samples[start..end]
            .iter()
            .zip(window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();

        // Forward FFT
        fft.process_with_scratch(&mut fft_buffer, &mut fft_scratch);

        // Compute power spectrum
        let power: Vec<f32> = fft_buffer[..n_bins].iter().map(|c| c.norm_sqr()).collect();
        spectra.push(power);
    }

    spectra
}

/// Estimate width of a peak at a given bin
fn estimate_peak_width(power_spectra: &[Vec<f32>], center_bin: usize, n_bins: usize) -> usize {
    if power_spectra.is_empty() {
        return 1;
    }

    // Average power spectrum
    let mut avg_power: Vec<f32> = vec![0.0; n_bins];
    for power in power_spectra {
        for (k, &p) in power.iter().enumerate().take(n_bins) {
            avg_power[k] += p;
        }
    }
    let scale = 1.0 / power_spectra.len() as f32;
    for p in &mut avg_power {
        *p *= scale;
    }

    let center_power = avg_power[center_bin];
    let half_power = center_power * 0.5; // -3dB point

    // Find left edge
    let mut left = center_bin;
    while left > 0 && avg_power[left] > half_power {
        left -= 1;
    }

    // Find right edge
    let mut right = center_bin;
    while right < n_bins - 1 && avg_power[right] > half_power {
        right += 1;
    }

    (right - left).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_attenuator() {
        let attenuator = PeakAttenuator::new(48000);
        assert_eq!(attenuator.n_bins, WINDOW_SIZE / 2 + 1);
        assert!(!attenuator.has_peaks());
    }

    #[test]
    fn test_with_profile() {
        let mut attenuator = PeakAttenuator::new(48000);

        let profile = PeakProfile {
            peak_bins: vec![50],
            prominences_db: vec![12.0],
            widths: vec![2],
            stabilities: vec![0.9],
        };

        attenuator.init_with_profile(profile);
        assert!(attenuator.has_peaks());
        assert_eq!(attenuator.num_peaks(), 1);
    }

    #[test]
    fn test_process_audio() {
        let mut attenuator = PeakAttenuator::new(48000);

        // Create test signal (1 second of audio)
        let samples: Vec<f32> = (0..48000).map(|i| (i as f32 * 0.01).sin() * 0.5).collect();

        let output = attenuator.process(&samples);

        // Output should be same length as input
        assert_eq!(output.len(), samples.len());
    }

    #[test]
    fn test_peak_detection_empty() {
        let profile = detect_tonal_peaks(&[], 48000, &PeakAttenuatorParams::default());
        assert!(!profile.has_peaks());
    }
}
