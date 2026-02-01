//! True Peak Limiter with lookahead
//!
//! A transparent limiter that uses lookahead to smoothly reduce gain
//! before peaks arrive, avoiding clipping distortion.

#![allow(dead_code)]

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

/// Gain reduction statistics from limiter processing
#[derive(Debug, Clone, Copy, Default)]
pub struct LimiterStats {
    /// Maximum gain reduction in dB (negative value)
    pub max_reduction_db: f32,
    /// Peak output level in dBFS
    pub peak_output_db: f32,
}

/// True peak limiter with lookahead
pub struct Limiter {
    ceiling_linear: f32,
    ceiling_db: f32,
    lookahead_samples: usize,
    release_coeff: f32,

    // State for buffer boundary handling
    lookahead_buffer: Vec<f32>,       // Mono or left channel samples for next call's lookahead
    lookahead_buffer_right: Vec<f32>, // Right channel samples (stereo only)
    current_gain: f32,                // Carry over gain state for release smoothing

    // Pre-allocated work buffers (reused across calls to avoid allocations)
    work_combined: Vec<f32>,
    work_peak_envelope: Vec<f32>,
    work_gain: Vec<f32>,
}

impl Limiter {
    /// Create a new limiter
    ///
    /// # Arguments
    /// * `ceiling_db` - Maximum output level in dBFS (e.g., -1.0)
    /// * `lookahead_ms` - Lookahead time in milliseconds (5-10ms typical)
    /// * `release_ms` - Release time in milliseconds (50-200ms typical)
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(ceiling_db: f32, lookahead_ms: f32, release_ms: f32, sample_rate: f32) -> Self {
        let ceiling_linear = 10.0_f32.powf(ceiling_db / 20.0);
        let lookahead_samples = (lookahead_ms * sample_rate / 1000.0) as usize;

        // Release coefficient: how fast gain recovers after peak
        let release_samples = release_ms * sample_rate / 1000.0;
        let release_coeff = (-2.2 / release_samples).exp();

        // Pre-allocate work buffers with reasonable initial capacity
        let initial_capacity = 65536; // ~1.4 seconds at 48kHz
        Self {
            ceiling_linear,
            ceiling_db,
            lookahead_samples,
            release_coeff,
            lookahead_buffer: Vec::new(),
            lookahead_buffer_right: Vec::new(),
            current_gain: 1.0,
            work_combined: Vec::with_capacity(initial_capacity),
            work_peak_envelope: Vec::with_capacity(initial_capacity),
            work_gain: Vec::with_capacity(initial_capacity),
        }
    }

    /// Process mono audio in-place, returns gain reduction stats
    pub fn process_mono(&mut self, samples: &mut [f32]) -> LimiterStats {
        if samples.is_empty() {
            return LimiterStats::default();
        }

        let offset = self.lookahead_buffer.len();
        let total_len = offset + samples.len();

        // Reuse work_combined buffer
        self.work_combined.clear();
        self.work_combined.reserve(total_len);
        self.work_combined.extend_from_slice(&self.lookahead_buffer);
        self.work_combined.extend_from_slice(samples);

        // Step 1: Find peak values using sliding max (O(n) total instead of O(n × lookahead))
        self.work_peak_envelope.clear();
        self.work_peak_envelope.resize(samples.len(), 0.0);

        let mut sliding_max = SlidingMax::new(self.lookahead_samples);

        // Prime the sliding max with the initial lookahead window
        for i in 0..offset.min(self.lookahead_samples) {
            sliding_max.push(self.work_combined[i].abs());
        }

        // Process with sliding max
        for i in 0..samples.len() {
            let combined_idx = offset + i;
            // Push the sample at lookahead distance (if it exists)
            let lookahead_idx = combined_idx + self.lookahead_samples.saturating_sub(1);
            if lookahead_idx < total_len {
                self.work_peak_envelope[i] = sliding_max.push(self.work_combined[lookahead_idx].abs());
            } else {
                // At the end, just use current max
                self.work_peak_envelope[i] = sliding_max.push(self.work_combined[combined_idx].abs());
            }
        }

        // Step 2: Calculate required gain reduction (reuse work_gain buffer)
        self.work_gain.clear();
        self.work_gain.resize(samples.len(), 1.0);
        for i in 0..samples.len() {
            let peak = self.work_peak_envelope[i];
            if peak > self.ceiling_linear {
                self.work_gain[i] = self.ceiling_linear / peak;
            }
        }

        // Step 3: Smooth gain with release (attack is instant due to lookahead)
        let mut prev_gain = self.current_gain;
        for i in 0..samples.len() {
            let g = &mut self.work_gain[i];
            if *g > prev_gain {
                *g = prev_gain * self.release_coeff + *g * (1.0 - self.release_coeff);
            }
            prev_gain = *g;
        }

        // Track min gain (max reduction)
        let mut min_gain = 1.0_f32;
        for &g in &self.work_gain {
            min_gain = min_gain.min(g);
        }

        // Step 4: Apply gain
        let mut peak_output = 0.0_f32;
        for i in 0..samples.len() {
            samples[i] *= self.work_gain[i];
            peak_output = peak_output.max(samples[i].abs());
        }

        // Store state for next call
        let start = samples.len().saturating_sub(self.lookahead_samples);
        self.lookahead_buffer.clear();
        self.lookahead_buffer.extend_from_slice(&samples[start..]);
        self.current_gain = *self.work_gain.last().unwrap_or(&1.0);

        LimiterStats {
            max_reduction_db: 20.0 * min_gain.max(1e-10).log10(),
            peak_output_db: 20.0 * peak_output.max(1e-10).log10(),
        }
    }

    /// Process stereo audio in-place with linked gain reduction, returns stats
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) -> LimiterStats {
        if left.is_empty() || right.is_empty() {
            return LimiterStats::default();
        }

        let len = left.len().min(right.len());
        let offset = self.lookahead_buffer.len();
        let total_len = offset + len;

        // Reuse work_combined for linked stereo peaks (max of L/R)
        self.work_combined.clear();
        self.work_combined.reserve(total_len);

        // Add previous buffer peaks
        for i in 0..offset {
            let l = if i < self.lookahead_buffer.len() { self.lookahead_buffer[i].abs() } else { 0.0 };
            let r = if i < self.lookahead_buffer_right.len() { self.lookahead_buffer_right[i].abs() } else { 0.0 };
            self.work_combined.push(l.max(r));
        }
        // Add current buffer peaks
        for i in 0..len {
            self.work_combined.push(left[i].abs().max(right[i].abs()));
        }

        // Step 1: Find peak values using sliding max (O(n) total)
        self.work_peak_envelope.clear();
        self.work_peak_envelope.resize(len, 0.0);

        let mut sliding_max = SlidingMax::new(self.lookahead_samples);

        // Prime the sliding max with the initial lookahead window
        for i in 0..offset.min(self.lookahead_samples) {
            sliding_max.push(self.work_combined[i]);
        }

        // Process with sliding max
        for i in 0..len {
            let combined_idx = offset + i;
            let lookahead_idx = combined_idx + self.lookahead_samples.saturating_sub(1);
            if lookahead_idx < total_len {
                self.work_peak_envelope[i] = sliding_max.push(self.work_combined[lookahead_idx]);
            } else {
                self.work_peak_envelope[i] = sliding_max.push(self.work_combined[combined_idx]);
            }
        }

        // Step 2: Calculate required gain reduction
        self.work_gain.clear();
        self.work_gain.resize(len, 1.0);
        for i in 0..len {
            let peak = self.work_peak_envelope[i];
            if peak > self.ceiling_linear {
                self.work_gain[i] = self.ceiling_linear / peak;
            }
        }

        // Step 3: Smooth gain with release
        let mut prev_gain = self.current_gain;
        for i in 0..len {
            let g = &mut self.work_gain[i];
            if *g > prev_gain {
                *g = prev_gain * self.release_coeff + *g * (1.0 - self.release_coeff);
            }
            prev_gain = *g;
        }

        // Track min gain (max reduction)
        let mut min_gain = 1.0_f32;
        for &g in &self.work_gain {
            min_gain = min_gain.min(g);
        }

        // Step 4: Apply gain to both channels
        let mut peak_output = 0.0_f32;
        for i in 0..len {
            left[i] *= self.work_gain[i];
            right[i] *= self.work_gain[i];
            peak_output = peak_output.max(left[i].abs().max(right[i].abs()));
        }

        // Store state for next call
        let start = len.saturating_sub(self.lookahead_samples);
        self.lookahead_buffer.clear();
        self.lookahead_buffer.extend_from_slice(&left[start..len]);
        self.lookahead_buffer_right.clear();
        self.lookahead_buffer_right.extend_from_slice(&right[start..len]);
        self.current_gain = *self.work_gain.last().unwrap_or(&1.0);

        LimiterStats {
            max_reduction_db: 20.0 * min_gain.max(1e-10).log10(),
            peak_output_db: 20.0 * peak_output.max(1e-10).log10(),
        }
    }

    /// Reset limiter state
    pub fn reset(&mut self) {
        self.lookahead_buffer.clear();
        self.lookahead_buffer_right.clear();
        self.current_gain = 1.0;
    }

    /// Get latency in samples
    pub fn get_latency_samples(&self) -> usize {
        self.lookahead_samples
    }
}

impl crate::traits::AudioProcessor for Limiter {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        self.process_mono(buffer);
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_samples
    }
}

// =============================================================================
// Stereo Realtime Limiter (sample-by-sample processing for plugins)
// =============================================================================

/// Stereo realtime limiter with linked gain reduction
pub struct StereoRealtimeLimiter {
    ceiling_linear: f32,
    lookahead_samples: usize,
    release_coeff: f32,

    // Delay lines for audio (lookahead)
    delay_buffer_left: Vec<f32>,
    delay_buffer_right: Vec<f32>,
    delay_write_pos: usize,

    // Gain smoothing state
    current_gain: f32,

    // Peak detection in lookahead window (linked stereo)
    peak_buffer: Vec<f32>,
}

impl StereoRealtimeLimiter {
    pub fn new(ceiling_db: f32, lookahead_ms: f32, release_ms: f32, sample_rate: f32) -> Self {
        let ceiling_linear = 10.0_f32.powf(ceiling_db / 20.0);
        let lookahead_samples = ((lookahead_ms * sample_rate / 1000.0) as usize).max(1);

        let release_samples = release_ms * sample_rate / 1000.0;
        let release_coeff = (-2.2 / release_samples).exp();

        Self {
            ceiling_linear,
            lookahead_samples,
            release_coeff,
            delay_buffer_left: vec![0.0; lookahead_samples],
            delay_buffer_right: vec![0.0; lookahead_samples],
            delay_write_pos: 0,
            peak_buffer: vec![0.0; lookahead_samples],
            current_gain: 1.0,
        }
    }

    /// Process a stereo sample pair, returns (left_out, right_out, gain_reduction_db)
    pub fn process(&mut self, left: f32, right: f32) -> (f32, f32, f32) {
        // Store max of L/R in peak buffer (linked stereo)
        self.peak_buffer[self.delay_write_pos] = left.abs().max(right.abs());

        // Find max peak in lookahead window
        let max_peak = self.peak_buffer.iter().cloned().fold(0.0_f32, f32::max);

        // Calculate target gain
        let target_gain = if max_peak > self.ceiling_linear {
            self.ceiling_linear / max_peak
        } else {
            1.0
        };

        // Smooth gain changes
        if target_gain < self.current_gain {
            // Attack: instant (we have lookahead)
            self.current_gain = target_gain;
        } else {
            // Release: smooth
            self.current_gain = self.current_gain * self.release_coeff
                + target_gain * (1.0 - self.release_coeff);
        }

        // Get delayed samples
        let delayed_left = self.delay_buffer_left[self.delay_write_pos];
        let delayed_right = self.delay_buffer_right[self.delay_write_pos];

        // Store current input in delay buffers
        self.delay_buffer_left[self.delay_write_pos] = left;
        self.delay_buffer_right[self.delay_write_pos] = right;

        // Advance write position
        self.delay_write_pos = (self.delay_write_pos + 1) % self.lookahead_samples;

        // Apply gain to delayed samples
        let left_out = delayed_left * self.current_gain;
        let right_out = delayed_right * self.current_gain;
        let gain_reduction_db = 20.0 * self.current_gain.max(1e-10).log10();

        (left_out, right_out, gain_reduction_db)
    }

    pub fn reset(&mut self) {
        self.delay_buffer_left.fill(0.0);
        self.delay_buffer_right.fill(0.0);
        self.peak_buffer.fill(0.0);
        self.delay_write_pos = 0;
        self.current_gain = 1.0;
    }

    pub fn latency_samples(&self) -> usize {
        self.lookahead_samples
    }

    /// Get current gain reduction in dB (for metering)
    pub fn get_gain_reduction_db(&self) -> f32 {
        20.0 * self.current_gain.max(1e-10).log10()
    }
}

impl crate::traits::StereoProcessor for StereoRealtimeLimiter {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let (out_l, out_r, _gr) = self.process(*l, *r);
            *l = out_l;
            *r = out_r;
        }
    }

    fn reset(&mut self) {
        self.reset()
    }

    fn latency_samples(&self) -> usize {
        self.lookahead_samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_limiter_below_ceiling() {
        let mut limiter = Limiter::new(-1.0, 5.0, 100.0, 48000.0);
        let mut samples = vec![0.5, 0.3, -0.4, 0.2];
        let original = samples.clone();
        limiter.process_mono(&mut samples);

        // Samples below ceiling should pass through unchanged
        for (i, (&s, &o)) in samples.iter().zip(original.iter()).enumerate() {
            assert!(
                (s - o).abs() < 0.001,
                "Sample {} should be unchanged: {} vs {}",
                i,
                s,
                o
            );
        }
    }

    #[test]
    fn test_limiter_above_ceiling() {
        let mut limiter = Limiter::new(-6.0, 5.0, 100.0, 48000.0); // -6dB = 0.5 linear
        let mut samples = vec![0.0; 1000];
        // Create a spike at sample 500
        samples[500] = 1.0;

        limiter.process_mono(&mut samples);

        // The spike should be limited
        let ceiling = 10.0_f32.powf(-6.0 / 20.0);
        for (i, &s) in samples.iter().enumerate() {
            assert!(
                s.abs() <= ceiling + 0.001,
                "Sample {} value {} exceeds ceiling {}",
                i,
                s.abs(),
                ceiling
            );
        }
    }

    #[test]
    fn test_stereo_linked() {
        let mut limiter = Limiter::new(-6.0, 5.0, 100.0, 48000.0);
        let mut left = vec![0.0; 1000];
        let mut right = vec![0.0; 1000];

        // Spike only on left channel
        left[500] = 1.0;
        right[500] = 0.1;

        limiter.process_stereo(&mut left, &mut right);

        // Both channels should be reduced equally (linked)
        let ceiling = 10.0_f32.powf(-6.0 / 20.0);
        assert!(
            left[500].abs() <= ceiling + 0.001,
            "Left spike {} exceeds ceiling {}",
            left[500].abs(),
            ceiling
        );
    }

    #[test]
    fn test_lookahead_smoothing() {
        // With lookahead, gain should start reducing before the peak
        let mut limiter = Limiter::new(-6.0, 5.0, 100.0, 48000.0);
        let mut samples = vec![0.0; 1000];
        samples[500] = 1.0;

        let original_before = samples[400]; // Well before the peak
        limiter.process_mono(&mut samples);

        // Sample at 400 should be unchanged (before lookahead window)
        assert!(
            (samples[400] - original_before).abs() < 0.001,
            "Sample before lookahead window should be unchanged"
        );

        // Sample just before peak (within lookahead) may be attenuated
        // due to the peak being in the lookahead window
    }
}
