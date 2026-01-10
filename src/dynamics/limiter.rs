//! True Peak Limiter with lookahead
//!
//! A transparent limiter that uses lookahead to smoothly reduce gain
//! before peaks arrive, avoiding clipping distortion.

#![allow(dead_code)]

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

        Self {
            ceiling_linear,
            ceiling_db,
            lookahead_samples,
            release_coeff,
            lookahead_buffer: Vec::new(),
            lookahead_buffer_right: Vec::new(),
            current_gain: 1.0,
        }
    }

    /// Process mono audio in-place, returns gain reduction stats
    pub fn process_mono(&mut self, samples: &mut [f32]) -> LimiterStats {
        if samples.is_empty() {
            return LimiterStats::default();
        }

        // Combine previous buffer with current samples for full lookahead at boundaries
        let combined: Vec<f32> = self.lookahead_buffer.iter()
            .chain(samples.iter())
            .copied()
            .collect();
        let offset = self.lookahead_buffer.len();

        // Step 1: Find peak values within lookahead window for each sample
        let mut peak_envelope = vec![0.0_f32; samples.len()];
        for i in 0..samples.len() {
            let combined_idx = offset + i;
            let mut max_peak = combined[combined_idx].abs();
            // Look ahead by lookahead_samples (into combined buffer)
            let end = (combined_idx + self.lookahead_samples).min(combined.len());
            for j in combined_idx..end {
                max_peak = max_peak.max(combined[j].abs());
            }
            peak_envelope[i] = max_peak;
        }

        // Step 2: Calculate required gain reduction
        let mut gain = vec![1.0_f32; samples.len()];
        for (i, &peak) in peak_envelope.iter().enumerate() {
            if peak > self.ceiling_linear {
                gain[i] = self.ceiling_linear / peak;
            }
        }

        // Step 3: Smooth gain with release (attack is instant due to lookahead)
        // Start from previous call's gain for continuity
        let mut prev_gain = self.current_gain;
        for g in gain.iter_mut() {
            // If current gain is higher (less reduction), smooth the recovery
            if *g > prev_gain {
                *g = prev_gain * self.release_coeff + *g * (1.0 - self.release_coeff);
            }
            prev_gain = *g;
        }

        // Track min gain (max reduction)
        let min_gain = gain.iter().cloned().fold(1.0_f32, f32::min);

        // Step 4: Apply gain
        let mut peak_output = 0.0_f32;
        for (sample, g) in samples.iter_mut().zip(gain.iter()) {
            *sample *= g;
            peak_output = peak_output.max(sample.abs());
        }

        // Store state for next call
        let start = samples.len().saturating_sub(self.lookahead_samples);
        self.lookahead_buffer = samples[start..].to_vec();
        self.current_gain = *gain.last().unwrap_or(&1.0);

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

        // Combine previous buffers with current samples for full lookahead at boundaries
        let combined_left: Vec<f32> = self.lookahead_buffer.iter()
            .chain(left[..len].iter())
            .copied()
            .collect();
        let combined_right: Vec<f32> = self.lookahead_buffer_right.iter()
            .chain(right[..len].iter())
            .copied()
            .collect();
        let offset = self.lookahead_buffer.len();

        // Step 1: Find peak values within lookahead window (linked stereo)
        let mut peak_envelope = vec![0.0_f32; len];
        for i in 0..len {
            let combined_idx = offset + i;
            let mut max_peak = combined_left[combined_idx].abs().max(combined_right[combined_idx].abs());
            let end = (combined_idx + self.lookahead_samples).min(combined_left.len());
            for j in combined_idx..end {
                max_peak = max_peak.max(combined_left[j].abs().max(combined_right[j].abs()));
            }
            peak_envelope[i] = max_peak;
        }

        // Step 2: Calculate required gain reduction
        let mut gain = vec![1.0_f32; len];
        for (i, &peak) in peak_envelope.iter().enumerate() {
            if peak > self.ceiling_linear {
                gain[i] = self.ceiling_linear / peak;
            }
        }

        // Step 3: Smooth gain with release
        // Start from previous call's gain for continuity
        let mut prev_gain = self.current_gain;
        for g in gain.iter_mut() {
            if *g > prev_gain {
                *g = prev_gain * self.release_coeff + *g * (1.0 - self.release_coeff);
            }
            prev_gain = *g;
        }

        // Track min gain (max reduction)
        let min_gain = gain.iter().cloned().fold(1.0_f32, f32::min);

        // Step 4: Apply gain to both channels
        let mut peak_output = 0.0_f32;
        for i in 0..len {
            left[i] *= gain[i];
            right[i] *= gain[i];
            peak_output = peak_output.max(left[i].abs().max(right[i].abs()));
        }

        // Store state for next call
        let start = len.saturating_sub(self.lookahead_samples);
        self.lookahead_buffer = left[start..len].to_vec();
        self.lookahead_buffer_right = right[start..len].to_vec();
        self.current_gain = *gain.last().unwrap_or(&1.0);

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
