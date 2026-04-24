//! VCA-style Clinical Peak Compressor
//!
//! Transparent peak compression with histogram-based threshold detection.
//! Designed for catching peaks without affecting the body of the audio.

#![allow(dead_code)]

use super::peakcomp_analysis::{analyze_peak_profile, PeakProfile};
use crate::analysis::utils::{db_to_linear, linear_to_db, mix_to_mono};
use std::collections::VecDeque;

// =============================================================================
// Sliding Maximum (O(1) amortized peak tracking for lookahead windows)
// =============================================================================

/// Monotonic deque for O(1) amortized sliding window maximum
#[derive(Clone, Debug)]
struct SlidingMax {
    /// Deque stores (index, value) pairs
    deque: VecDeque<(usize, f32)>,
    /// Window size
    window_size: usize,
    /// Current sample index
    current_idx: usize,
}

impl SlidingMax {
    fn new(window_size: usize) -> Self {
        Self {
            deque: VecDeque::with_capacity(window_size),
            window_size,
            current_idx: 0,
        }
    }

    /// Push a new value and return the current maximum in the window
    #[inline]
    fn push(&mut self, val: f32) -> f32 {
        let idx = self.current_idx;
        self.current_idx += 1;

        // Remove old entries that are outside the window
        while let Some(&(front_idx, _)) = self.deque.front() {
            if idx >= self.window_size && front_idx <= idx - self.window_size {
                self.deque.pop_front();
            } else {
                break;
            }
        }

        // Remove entries from back that are smaller than the new value
        // (they can never be the maximum while the new value is in the window)
        while let Some(&(_, back_val)) = self.deque.back() {
            if back_val <= val {
                self.deque.pop_back();
            } else {
                break;
            }
        }

        self.deque.push_back((idx, val));

        // The front is always the maximum
        self.deque.front().map(|&(_, v)| v).unwrap_or(0.0)
    }

    fn reset(&mut self) {
        self.deque.clear();
        self.current_idx = 0;
    }
}

// =============================================================================
// PeakComp Presets (VCA lookahead: transparent, catches peaks)
// =============================================================================

/// PeakComp preset parameters
#[derive(Clone, Copy, Debug)]
pub struct PeakCompPreset {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub lookahead_ms: f32,
}

/// Preset names (1-3)
pub const PEAKCOMP_PRESET_NAMES: [&str; 3] = ["Subtle", "Balanced", "Intense"];

/// PeakComp presets (1-3 scale) - transparent VCA-style peak catching
pub const PEAKCOMP_PRESETS: [PeakCompPreset; 3] = [
    // 1: Subtle - only catch the loudest peaks
    PeakCompPreset {
        threshold_db: -6.0,
        ratio: 4.0,
        attack_ms: 1.0,
        release_ms: 150.0,
        knee_db: 6.0,
        lookahead_ms: 5.0,
    },
    // 2: Balanced - moderate peak control (default)
    PeakCompPreset {
        threshold_db: -9.0,
        ratio: 6.0,
        attack_ms: 0.8,
        release_ms: 120.0,
        knee_db: 5.0,
        lookahead_ms: 5.0,
    },
    // 3: Intense - tighter peak control
    PeakCompPreset {
        threshold_db: -12.0,
        ratio: 8.0,
        attack_ms: 0.5,
        release_ms: 100.0,
        knee_db: 4.0,
        lookahead_ms: 5.0,
    },
];

/// Get preset name by level (1-3), returns "Unknown" for invalid levels
pub fn get_peakcomp_preset_name(level: u8) -> &'static str {
    PEAKCOMP_PRESET_NAMES
        .get((level as usize).saturating_sub(1))
        .unwrap_or(&"Unknown")
}

/// VCA-style peak compressor with look-ahead
#[derive(Clone, Debug)]
pub struct VcaPeakComp {
    threshold_db: f32,
    ratio: f32,
    attack_coeff: f32,
    release_coeff: f32,
    knee_db: f32,
    lookahead_samples: usize,
    lookahead_buffer: VecDeque<f32>,
    sliding_max: SlidingMax,
    envelope: f32,
    gain_reduction_db: f32,
    max_gain_reduction_db: f32,
    sample_rate: f32,
    // Cached values for ratio calculation
    ratio_factor: f32, // 1.0 - 1.0 / ratio
}

impl VcaPeakComp {
    /// Create a new VCA peak compressor
    pub fn new(
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        knee_db: f32,
        lookahead_ms: f32,
        sample_rate: f32,
    ) -> Self {
        let lookahead_samples = (lookahead_ms * sample_rate / 1000.0) as usize;

        // Time constants using -60dB decay point
        let attack_coeff = (-2.2 / (attack_ms * sample_rate / 1000.0)).exp();
        let release_coeff = (-2.2 / (release_ms * sample_rate / 1000.0)).exp();

        Self {
            threshold_db,
            ratio,
            attack_coeff,
            release_coeff,
            knee_db,
            lookahead_samples,
            lookahead_buffer: VecDeque::with_capacity(lookahead_samples + 1),
            sliding_max: SlidingMax::new(lookahead_samples + 1),
            envelope: 0.0,
            gain_reduction_db: 0.0,
            max_gain_reduction_db: 0.0,
            sample_rate,
            ratio_factor: 1.0 - 1.0 / ratio,
        }
    }

    /// Create with default parameters for voice peak catching
    pub fn new_default(sample_rate: f32) -> Self {
        Self::new(
            -12.0, // threshold (will be overridden by analysis)
            8.0,   // ratio 12:1
            0.5,   // attack 0.5ms
            100.0, // release 100ms
            4.0,   // knee 4dB
            5.0,   // lookahead 5ms
            sample_rate,
        )
    }

    /// Set threshold in dB
    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.threshold_db = threshold_db;
    }

    /// Set ratio
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
        self.ratio_factor = 1.0 - 1.0 / self.ratio;
    }

    /// Set attack time in ms
    pub fn set_attack(&mut self, attack_ms: f32) {
        self.attack_coeff = (-2.2 / (attack_ms * self.sample_rate / 1000.0)).exp();
    }

    /// Set release time in ms
    pub fn set_release(&mut self, release_ms: f32) {
        self.release_coeff = (-2.2 / (release_ms * self.sample_rate / 1000.0)).exp();
    }

    /// VCA-style gain calculation with soft knee
    #[inline]
    fn calculate_gain_reduction(&self, level_db: f32) -> f32 {
        let knee_start = self.threshold_db - self.knee_db / 2.0;
        let knee_end = self.threshold_db + self.knee_db / 2.0;

        if level_db < knee_start {
            0.0 // Below knee - no reduction
        } else if level_db > knee_end {
            // Above knee - full ratio (use cached ratio_factor)
            (level_db - self.threshold_db) * self.ratio_factor
        } else {
            // In knee - quadratic interpolation for smooth transition
            let x = level_db - knee_start;
            let knee_ratio = x / self.knee_db;
            knee_ratio * knee_ratio * (level_db - self.threshold_db) * self.ratio_factor / 2.0
        }
    }

    /// Process a single sample with look-ahead
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        // Add input to look-ahead buffer
        self.lookahead_buffer.push_back(input);

        // If buffer not full yet, return silence (latency compensation)
        if self.lookahead_buffer.len() <= self.lookahead_samples {
            // Still add to sliding max for when we start processing
            self.sliding_max.push(input.abs());
            return 0.0;
        }

        // Get the delayed sample (the one we'll actually output)
        let delayed_sample = self.lookahead_buffer.pop_front().unwrap_or(0.0);

        // Use sliding max for O(1) peak detection (instead of O(lookahead_samples) scan)
        let peak_in_window = self.sliding_max.push(input.abs());

        // Convert to dB using fast approximation
        let peak_db = linear_to_db(peak_in_window);

        // Calculate target gain reduction
        let target_gr_db = self.calculate_gain_reduction(peak_db);

        // Envelope follower for gain reduction (attack/release smoothing)
        if target_gr_db > self.gain_reduction_db {
            // Attack - increase gain reduction
            self.gain_reduction_db = self.attack_coeff * self.gain_reduction_db
                + (1.0 - self.attack_coeff) * target_gr_db;
        } else {
            // Release - decrease gain reduction
            self.gain_reduction_db = self.release_coeff * self.gain_reduction_db
                + (1.0 - self.release_coeff) * target_gr_db;
        }
        self.max_gain_reduction_db = self.max_gain_reduction_db.max(self.gain_reduction_db);

        // Apply gain reduction using fast approximation
        let gain = db_to_linear(-self.gain_reduction_db);
        delayed_sample * gain
    }

    /// Get current gain reduction in dB (negative value)
    pub fn get_gain_reduction_db(&self) -> f32 {
        -self.gain_reduction_db
    }

    /// Get maximum gain reduction in dB (negative value)
    pub fn get_max_gain_reduction_db(&self) -> f32 {
        -self.max_gain_reduction_db
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.lookahead_buffer.clear();
        self.sliding_max.reset();
        self.envelope = 0.0;
        self.gain_reduction_db = 0.0;
        self.max_gain_reduction_db = 0.0;
    }

    /// Get latency in samples
    pub fn get_latency_samples(&self) -> usize {
        self.lookahead_samples
    }
}

impl crate::traits::AudioProcessor for VcaPeakComp {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_samples
    }
}

impl crate::traits::MonoProcessor for VcaPeakComp {}

impl crate::traits::Processor for VcaPeakComp {
    fn new(sample_rate: f32) -> Self {
        Self::new_default(sample_rate)
    }
}

/// Stereo VCA peak compressor with linked detection
#[derive(Clone, Debug)]
pub struct StereoVcaPeakComp {
    pub left: VcaPeakComp,
    pub right: VcaPeakComp,
    is_stereo: bool,
    sample_rate: f32,
    last_profile: Option<PeakProfile>,
    /// Sliding max for linked stereo peak detection
    stereo_sliding_max: SlidingMax,
}

impl StereoVcaPeakComp {
    /// Create a new stereo VCA peak compressor
    pub fn new(sample_rate: f32) -> Self {
        let left = VcaPeakComp::new_default(sample_rate);
        let lookahead = left.lookahead_samples + 1;
        Self {
            left,
            right: VcaPeakComp::new_default(sample_rate),
            is_stereo: false,
            sample_rate,
            last_profile: None,
            stereo_sliding_max: SlidingMax::new(lookahead),
        }
    }

    /// Create with preset (1-5)
    pub fn new_with_preset(sample_rate: f32, preset: u8) -> Option<Self> {
        let p = PEAKCOMP_PRESETS.get((preset as usize).saturating_sub(1))?;
        let left = VcaPeakComp::new(
            p.threshold_db,
            p.ratio,
            p.attack_ms,
            p.release_ms,
            p.knee_db,
            p.lookahead_ms,
            sample_rate,
        );
        let lookahead = left.lookahead_samples + 1;
        let right = VcaPeakComp::new(
            p.threshold_db,
            p.ratio,
            p.attack_ms,
            p.release_ms,
            p.knee_db,
            p.lookahead_ms,
            sample_rate,
        );
        Some(Self {
            left,
            right,
            is_stereo: false,
            sample_rate,
            last_profile: None,
            stereo_sliding_max: SlidingMax::new(lookahead),
        })
    }

    /// Analyze audio and configure compressor
    /// Returns the peak profile for display
    pub fn configure(&mut self, samples: &[Vec<f32>]) -> &PeakProfile {
        self.is_stereo = samples.len() >= 2;

        // Mix to mono for analysis
        let mono = if self.is_stereo {
            mix_to_mono(&samples[0], &samples[1])
        } else {
            samples[0].clone()
        };

        let profile = analyze_peak_profile(&mono);

        // Set threshold from histogram analysis
        self.left.set_threshold(profile.histogram_threshold_db);
        self.right.set_threshold(profile.histogram_threshold_db);

        self.last_profile = Some(profile);
        self.last_profile.as_ref().unwrap()
    }

    /// Set threshold manually (overrides auto)
    pub fn set_threshold(&mut self, threshold_db: f32) {
        self.left.set_threshold(threshold_db);
        self.right.set_threshold(threshold_db);
    }

    /// Set ratio
    pub fn set_ratio(&mut self, ratio: f32) {
        self.left.set_ratio(ratio);
        self.right.set_ratio(ratio);
    }

    /// Set attack time in ms
    pub fn set_attack(&mut self, attack_ms: f32) {
        self.left.set_attack(attack_ms);
        self.right.set_attack(attack_ms);
    }

    /// Set release time in ms
    pub fn set_release(&mut self, release_ms: f32) {
        self.left.set_release(release_ms);
        self.right.set_release(release_ms);
    }

    /// Process stereo audio in-place
    /// Uses linked peak detection for stereo coherence
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        let len = left.len().min(right.len());
        for i in 0..len {
            // Linked detection: use max of both channels
            let peak = left[i].abs().max(right[i].abs());

            // Add to both buffers
            self.left.lookahead_buffer.push_back(left[i]);
            self.right.lookahead_buffer.push_back(right[i]);

            // Check if buffers are full
            if self.left.lookahead_buffer.len() <= self.left.lookahead_samples {
                // Still add to sliding max
                self.stereo_sliding_max.push(peak);
                left[i] = 0.0;
                right[i] = 0.0;
                continue;
            }

            // Get delayed samples
            let delayed_l = self.left.lookahead_buffer.pop_front().unwrap_or(0.0);
            let delayed_r = self.right.lookahead_buffer.pop_front().unwrap_or(0.0);

            // Use sliding max for O(1) peak detection (linked across channels)
            let peak_in_window = self.stereo_sliding_max.push(peak);

            // Calculate gain reduction using left compressor (linked)
            let peak_db = linear_to_db(peak_in_window);
            let target_gr_db = self.left.calculate_gain_reduction(peak_db);

            // Envelope follower (shared)
            if target_gr_db > self.left.gain_reduction_db {
                self.left.gain_reduction_db = self.left.attack_coeff * self.left.gain_reduction_db
                    + (1.0 - self.left.attack_coeff) * target_gr_db;
            } else {
                self.left.gain_reduction_db = self.left.release_coeff * self.left.gain_reduction_db
                    + (1.0 - self.left.release_coeff) * target_gr_db;
            }
            self.left.max_gain_reduction_db = self
                .left
                .max_gain_reduction_db
                .max(self.left.gain_reduction_db);

            // Apply same gain to both channels using fast approximation
            let gain = db_to_linear(-self.left.gain_reduction_db);
            left[i] = delayed_l * gain;
            right[i] = delayed_r * gain;
        }
    }

    /// Process a single stereo sample pair with linked detection
    /// Returns (left_out, right_out)
    #[inline]
    pub fn process_sample_stereo(&mut self, left_in: f32, right_in: f32) -> (f32, f32) {
        // Linked detection: use max of both channels
        let peak = left_in.abs().max(right_in.abs());

        // Add to both lookahead buffers
        self.left.lookahead_buffer.push_back(left_in);
        self.right.lookahead_buffer.push_back(right_in);

        // Check if buffers are full (latency compensation)
        if self.left.lookahead_buffer.len() <= self.left.lookahead_samples {
            // Still track in sliding max
            self.stereo_sliding_max.push(peak);
            return (0.0, 0.0);
        }

        // Get delayed samples
        let delayed_l = self.left.lookahead_buffer.pop_front().unwrap_or(0.0);
        let delayed_r = self.right.lookahead_buffer.pop_front().unwrap_or(0.0);

        // Use sliding max for O(1) peak detection (linked across channels)
        let peak_in_window = self.stereo_sliding_max.push(peak);

        // Calculate gain reduction using left compressor (linked)
        let peak_db = linear_to_db(peak_in_window);
        let target_gr_db = self.left.calculate_gain_reduction(peak_db);

        // Envelope follower (shared between channels)
        if target_gr_db > self.left.gain_reduction_db {
            self.left.gain_reduction_db = self.left.attack_coeff * self.left.gain_reduction_db
                + (1.0 - self.left.attack_coeff) * target_gr_db;
        } else {
            self.left.gain_reduction_db = self.left.release_coeff * self.left.gain_reduction_db
                + (1.0 - self.left.release_coeff) * target_gr_db;
        }
        self.left.max_gain_reduction_db = self
            .left
            .max_gain_reduction_db
            .max(self.left.gain_reduction_db);

        // Apply same gain to both channels (linked) using fast approximation
        let gain = db_to_linear(-self.left.gain_reduction_db);
        (delayed_l * gain, delayed_r * gain)
    }

    /// Process mono audio in-place
    pub fn process_mono(&mut self, samples: &mut [f32]) {
        for sample in samples.iter_mut() {
            *sample = self.left.process(*sample);
        }
    }

    /// Get last analysis result
    pub fn get_profile(&self) -> Option<&PeakProfile> {
        self.last_profile.as_ref()
    }

    /// Get gain reduction from left channel (for metering)
    pub fn get_gain_reduction_db(&self) -> f32 {
        self.left.get_gain_reduction_db()
    }

    /// Get maximum gain reduction from left channel (for metering)
    pub fn get_max_gain_reduction_db(&self) -> f32 {
        self.left.get_max_gain_reduction_db()
    }

    /// Get latency in samples
    pub fn get_latency_samples(&self) -> usize {
        self.left.get_latency_samples()
    }

    /// Reset internal state
    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
        self.stereo_sliding_max.reset();
    }
}

impl crate::traits::StereoProcessor for StereoVcaPeakComp {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        StereoVcaPeakComp::process_stereo(self, left, right)
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn latency_samples(&self) -> usize {
        self.left.get_latency_samples()
    }
}
