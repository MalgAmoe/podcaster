//! Audio Click/Pop Detection and Removal (FFT-accelerated)
//!
//! Based on:
//! - Oudre (2015) - AR-based click detection via prediction error
//! - Janssen et al. (1986) - AR-based interpolation of missing samples
//!
//! This is an offline processor - it requires the full audio signal
//! and cannot run in real-time.

use rustfft::{num_complex::Complex, FftPlanner};

// ============================================================================
// Public API
// ============================================================================

/// Complete audio declicker: detection + interpolation
///
/// This is an offline processor that analyzes the entire signal.
/// Parameters are automatically tuned based on sample rate.
pub struct Declicker {
    pub detector: ClickDetector,
    pub interpolator: Interpolator,
}

impl Declicker {
    /// Create a new declicker tuned for the given sample rate
    pub fn new(sample_rate: u32) -> Self {
        Self {
            detector: ClickDetector::new(sample_rate),
            interpolator: Interpolator::new(),
        }
    }

    /// Process f32 audio (convenience wrapper)
    pub fn process_f32(&self, audio: &[f32]) -> Vec<f32> {
        let audio_f64: Vec<f64> = audio.iter().map(|&x| x as f64).collect();
        let result = self.process(&audio_f64);
        result.iter().map(|&x| x as f32).collect()
    }

    /// Detect and fix clicks in audio, returns cleaned signal
    pub fn process(&self, audio: &[f64]) -> Vec<f64> {
        let bursts = self.detector.detect(audio);
        if bursts.is_empty() {
            return audio.to_vec();
        }
        self.interpolator.interpolate(audio, &bursts)
    }

    /// Just detect, return burst ranges (start, end) where end is exclusive
    pub fn detect(&self, audio: &[f64]) -> Vec<(usize, usize)> {
        self.detector.detect(audio)
    }

    /// Fix known burst locations
    pub fn fix(&self, audio: &[f64], bursts: &[(usize, usize)]) -> Vec<f64> {
        self.interpolator.interpolate(audio, bursts)
    }
}

// ============================================================================
// Detection
// ============================================================================

/// Click detector using AR-based prediction error analysis
pub struct ClickDetector {
    pub frame_size: usize,
    pub hop_size: usize,
    pub order: usize,
    pub threshold_k: f64,
    pub merge_distance: usize,
}

impl ClickDetector {
    /// Create detector with parameters tuned for sample rate
    ///
    /// Parameters are calculated based on speech processing research:
    /// - Frame size: ~25ms (standard for speech analysis)
    /// - Hop size: ~6ms (75% overlap)
    /// - AR order: ~1 pole per 1kHz of sample rate (speech recommendation)
    /// - Threshold k: 4.0 (balanced between detection and false positives)
    /// - Merge distance: ~0.5ms worth of samples
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as usize;

        // 25ms frame size, rounded to power of 2 for FFT efficiency
        let frame_ms = 25.0;
        let frame_samples = (sr as f64 * frame_ms / 1000.0) as usize;
        let frame_size = frame_samples.next_power_of_two();

        // 6ms hop size (75% overlap)
        let hop_ms = 6.0;
        let hop_size = (sr as f64 * hop_ms / 1000.0) as usize;

        // AR order: ~1 pole per 1kHz, clamped to reasonable range
        let order = (sr / 1000).clamp(16, 32);

        // Merge clicks within 0.5ms of each other
        let merge_distance = (sr as f64 * 0.5 / 1000.0) as usize;

        Self {
            frame_size,
            hop_size,
            order,
            threshold_k: 6.0, // Conservative for speech (plosives have high prediction error)
            merge_distance: merge_distance.max(10),
        }
    }

    pub fn with_params(
        frame_size: usize,
        hop_size: usize,
        order: usize,
        threshold_k: f64,
        merge_distance: usize,
    ) -> Self {
        Self {
            frame_size,
            hop_size,
            order,
            threshold_k,
            merge_distance,
        }
    }

    /// Detect clicks from f32 audio (convenience wrapper)
    pub fn detect_f32(&self, audio: &[f32]) -> Vec<(usize, usize)> {
        let audio_f64: Vec<f64> = audio.iter().map(|&x| x as f64).collect();
        self.detect(&audio_f64)
    }

    /// Detect clicks, returns Vec of (start, end) ranges (end is exclusive)
    pub fn detect(&self, audio: &[f64]) -> Vec<(usize, usize)> {
        if audio.len() < self.frame_size {
            return vec![];
        }

        // Normalize
        let max_val = audio.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
        if max_val < 1e-10 {
            return vec![];
        }
        let normalized: Vec<f64> = audio.iter().map(|x| x / max_val).collect();

        // Reusable FFT planner
        let mut planner = FftPlanner::new();
        let mut all_clicks = Vec::new();
        let mut pos = 0;

        while pos + self.frame_size <= normalized.len() {
            let frame = &normalized[pos..pos + self.frame_size];

            // LPC analysis with FFT-based autocorrelation
            let r = autocorrelation_fft_with_planner(frame, self.order, &mut planner);
            let (ar_coeffs, sigma_e) = levinson_durbin(&r, self.order);

            if sigma_e > 1e-10 {
                // Compute prediction error
                let pred_error = prediction_error(frame, &ar_coeffs);

                // Threshold
                let threshold = self.threshold_k * sigma_e;

                // Find samples above threshold
                for (i, &err) in pred_error.iter().enumerate().skip(self.order) {
                    if i < pred_error.len() - self.order && err > threshold {
                        all_clicks.push(pos + i);
                    }
                }
            }

            pos += self.hop_size;
        }

        // Merge nearby clicks into bursts
        merge_clicks(&mut all_clicks, self.merge_distance)
    }
}

// ============================================================================
// Interpolation (Janssen method)
// ============================================================================

/// AR-based interpolator for missing samples
///
/// Uses Janssen's iterative AR-based interpolation method.
/// Parameters are fixed based on research recommendations.
pub struct Interpolator {
    pub order: usize,
    pub iterations: usize,
    pub margin: usize,
}

impl Interpolator {
    /// Create interpolator with research-based defaults
    ///
    /// - order: 50 (sufficient for typical click bursts, per Janssen: 3*Nmax+2)
    /// - iterations: 3 (convergence typically achieved in 2-3 iterations)
    /// - margin: 10 samples context around burst
    pub fn new() -> Self {
        Self {
            order: 50,
            iterations: 3,
            margin: 10,
        }
    }

    pub fn with_params(order: usize, iterations: usize, margin: usize) -> Self {
        Self {
            order,
            iterations,
            margin,
        }
    }

    /// Interpolate missing samples at given burst locations
    pub fn interpolate(&self, audio: &[f64], bursts: &[(usize, usize)]) -> Vec<f64> {
        let mut output = audio.to_vec();
        let mut planner = FftPlanner::new();

        for &(start, end) in bursts {
            self.fix_burst(&mut output, start, end, &mut planner);
        }

        output
    }

    /// Fix a single burst in-place
    fn fix_burst(
        &self,
        signal: &mut [f64],
        burst_start: usize,
        burst_end: usize,
        planner: &mut FftPlanner<f64>,
    ) {
        let n = signal.len();
        let p = self.order;

        // Ensure we have enough context
        if burst_start < p || burst_end + p >= n {
            return;
        }

        // Define the window around the burst
        let win_start = burst_start.saturating_sub(p + self.margin);
        let win_end = (burst_end + p + self.margin).min(n);

        if win_end <= win_start {
            return;
        }

        // Extract window
        let mut window: Vec<f64> = signal[win_start..win_end].to_vec();

        // Missing indices relative to window
        let missing_start = burst_start - win_start;
        let missing_end = burst_end - win_start;
        let missing: Vec<usize> = (missing_start..missing_end).collect();

        if missing.is_empty() {
            return;
        }

        // Initialize missing samples to linear interpolation
        if missing_start > 0 && missing_end < window.len() {
            let left = window[missing_start - 1];
            let right = window[missing_end];
            let span = (missing_end - missing_start + 2) as f64;
            for (i, &idx) in missing.iter().enumerate() {
                let t = (i + 1) as f64 / span;
                window[idx] = left * (1.0 - t) + right * t;
            }
        } else {
            for &i in &missing {
                window[i] = 0.0;
            }
        }

        // Iterate: estimate AR -> interpolate -> repeat
        for _ in 0..self.iterations {
            let r = autocorrelation_fft_with_planner(&window, p, planner);
            let (ar_coeffs, _) = levinson_durbin(&r, p);

            if let Some(interpolated) = self.solve_interpolation(&window, &ar_coeffs, &missing) {
                for (&idx, &val) in missing.iter().zip(interpolated.iter()) {
                    window[idx] = val;
                }
            }
        }

        // Write back to signal
        for (i, &val) in window.iter().enumerate() {
            signal[win_start + i] = val;
        }
    }

    /// Solve B * s_missing = -d for missing samples
    fn solve_interpolation(
        &self,
        signal: &[f64],
        ar_coeffs: &[f64],
        missing: &[usize],
    ) -> Option<Vec<f64>> {
        let m = missing.len();
        let p = ar_coeffs.len();
        let n = signal.len();

        if m == 0 {
            return Some(vec![]);
        }

        // Build extended AR coefficients with a[0] = 1
        let mut a = vec![1.0];
        a.extend_from_slice(ar_coeffs);

        // Precompute b[k] = sum_{l=0}^{p-k} a[l] * a[l+k]
        let mut b = vec![0.0; p + 1];
        for k in 0..=p {
            for l in 0..=(p - k) {
                b[k] += a[l] * a[l + k];
            }
        }

        // Use HashSet for O(1) lookup instead of Vec::contains
        let missing_set: std::collections::HashSet<usize> = missing.iter().copied().collect();

        // Build matrix B (m x m)
        let mut mat_b = vec![vec![0.0; m]; m];
        for (i, &ti) in missing.iter().enumerate() {
            for (j, &tj) in missing.iter().enumerate() {
                let diff = ti.abs_diff(tj);
                if diff <= p {
                    mat_b[i][j] = b[diff];
                }
            }
        }

        // Build vector d (m)
        let mut d = vec![0.0; m];
        for (i, &ti) in missing.iter().enumerate() {
            let mut sum = 0.0;
            for k in -(p as isize)..=(p as isize) {
                let idx = ti as isize + k;
                if idx >= 0 && (idx as usize) < n {
                    let idx_usize = idx as usize;
                    if !missing_set.contains(&idx_usize) {
                        let k_abs = k.unsigned_abs();
                        if k_abs <= p {
                            sum += b[k_abs] * signal[idx_usize];
                        }
                    }
                }
            }
            d[i] = sum;
        }

        // Solve B * x = -d
        cholesky_solve(&mat_b, &d.iter().map(|x| -x).collect::<Vec<_>>())
    }
}

impl Default for Interpolator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// FFT-Based Autocorrelation
// ============================================================================

/// Compute autocorrelation using FFT (Wiener-Khinchin theorem)
/// R[k] = IFFT(|FFT(x)|²) / N
///
/// O(N log N) instead of O(N * max_lag)
#[allow(dead_code)]
pub fn autocorrelation_fft(signal: &[f64], max_lag: usize) -> Vec<f64> {
    let mut planner = FftPlanner::new();
    autocorrelation_fft_with_planner(signal, max_lag, &mut planner)
}

/// FFT autocorrelation with reusable planner (avoids repeated allocations)
fn autocorrelation_fft_with_planner(
    signal: &[f64],
    max_lag: usize,
    planner: &mut FftPlanner<f64>,
) -> Vec<f64> {
    let n = signal.len();
    if n == 0 {
        return vec![0.0; max_lag + 1];
    }

    // Pad to next power of 2, at least 2*n to avoid circular correlation
    let fft_size = (2 * n).next_power_of_two();

    let fft = planner.plan_fft_forward(fft_size);
    let ifft = planner.plan_fft_inverse(fft_size);

    // Zero-padded complex buffer
    let mut buffer: Vec<Complex<f64>> = vec![Complex::new(0.0, 0.0); fft_size];
    for (i, &s) in signal.iter().enumerate() {
        buffer[i] = Complex::new(s, 0.0);
    }

    // Forward FFT
    fft.process(&mut buffer);

    // Power spectrum: |FFT(x)|²
    for c in buffer.iter_mut() {
        *c = Complex::new(c.norm_sqr(), 0.0);
    }

    // Inverse FFT
    ifft.process(&mut buffer);

    // Extract real part, normalize
    // rustfft doesn't normalize, so divide by fft_size
    // Also divide by n for biased autocorrelation estimate
    let norm = (fft_size * n) as f64;

    (0..=max_lag.min(n - 1))
        .map(|i| buffer[i].re / norm)
        .collect()
}

// ============================================================================
// DSP Utilities
// ============================================================================

/// Levinson-Durbin algorithm: R -> (AR coefficients, sigma_e)
fn levinson_durbin(r: &[f64], order: usize) -> (Vec<f64>, f64) {
    if r.is_empty() || r[0] <= 0.0 {
        return (vec![0.0; order], 0.0);
    }

    let mut a = vec![0.0; order];
    let mut a_prev = vec![0.0; order];
    let mut e = r[0];

    for i in 0..order {
        if i + 1 >= r.len() {
            break;
        }

        // Reflection coefficient
        let mut lambda = r[i + 1];
        for j in 0..i {
            lambda += a[j] * r[i - j];
        }

        if e.abs() < 1e-10 {
            break;
        }

        let k = -lambda / e;

        // Update coefficients
        a_prev.copy_from_slice(&a);
        a[i] = k;
        for j in 0..i {
            a[j] = a_prev[j] + k * a_prev[i - 1 - j];
        }

        // Update error variance
        e *= 1.0 - k * k;

        if e <= 0.0 {
            break;
        }
    }

    (a, e.sqrt())
}

/// Compute AR prediction error |e[t]| = |s[t] + sum(a[k] * s[t-k])|
fn prediction_error(signal: &[f64], ar_coeffs: &[f64]) -> Vec<f64> {
    let n = signal.len();
    let p = ar_coeffs.len();
    let mut errors = vec![0.0; n];

    for i in p..n {
        let mut e = signal[i];
        for (j, &coef) in ar_coeffs.iter().enumerate() {
            e += coef * signal[i - 1 - j];
        }
        errors[i] = e.abs();
    }

    errors
}

/// Merge nearby click indices into (start, end) burst ranges
fn merge_clicks(clicks: &mut Vec<usize>, merge_distance: usize) -> Vec<(usize, usize)> {
    if clicks.is_empty() {
        return vec![];
    }

    clicks.sort_unstable();
    clicks.dedup();

    let mut bursts = Vec::new();
    let mut burst_start = clicks[0];
    let mut burst_end = clicks[0];

    for &click in clicks.iter().skip(1) {
        if click <= burst_end + merge_distance {
            burst_end = click;
        } else {
            bursts.push((burst_start, burst_end + 1));
            burst_start = click;
            burst_end = click;
        }
    }
    bursts.push((burst_start, burst_end + 1));

    bursts
}

/// Cholesky decomposition solve: A * x = b where A is symmetric positive definite
fn cholesky_solve(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = a.len();
    if n == 0 || b.len() != n {
        return None;
    }

    // Cholesky decomposition: A = L * L^T
    let mut l = vec![vec![0.0; n]; n];

    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i][j];
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }

            if i == j {
                if sum <= 0.0 {
                    return simple_solve(a, b);
                }
                l[i][j] = sum.sqrt();
            } else {
                if l[j][j].abs() < 1e-10 {
                    return simple_solve(a, b);
                }
                l[i][j] = sum / l[j][j];
            }
        }
    }

    // Forward substitution: L * y = b
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut sum = b[i];
        for j in 0..i {
            sum -= l[i][j] * y[j];
        }
        if l[i][i].abs() < 1e-10 {
            return simple_solve(a, b);
        }
        y[i] = sum / l[i][i];
    }

    // Backward substitution: L^T * x = y
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = y[i];
        for j in (i + 1)..n {
            sum -= l[j][i] * x[j];
        }
        if l[i][i].abs() < 1e-10 {
            return simple_solve(a, b);
        }
        x[i] = sum / l[i][i];
    }

    Some(x)
}

/// Fallback: Gaussian elimination with partial pivoting
fn simple_solve(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let n = a.len();
    if n == 0 {
        return Some(vec![]);
    }

    let mut aug: Vec<Vec<f64>> = a.iter().cloned().collect();
    for (i, row) in aug.iter_mut().enumerate() {
        row.push(b[i]);
    }

    for col in 0..n {
        let mut max_row = col;
        for row in (col + 1)..n {
            if aug[row][col].abs() > aug[max_row][col].abs() {
                max_row = row;
            }
        }
        aug.swap(col, max_row);

        let pivot = aug[col][col];
        if pivot.abs() < 1e-10 {
            return Some(vec![0.0; n]);
        }

        for row in (col + 1)..n {
            let factor = aug[row][col] / pivot;
            for j in col..=n {
                aug[row][j] -= factor * aug[col][j];
            }
        }
    }

    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut sum = aug[i][n];
        for j in (i + 1)..n {
            sum -= aug[i][j] * x[j];
        }
        if aug[i][i].abs() < 1e-10 {
            x[i] = 0.0;
        } else {
            x[i] = sum / aug[i][i];
        }
    }

    Some(x)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn make_sine(freq: f64, sample_rate: usize, duration_sec: f64) -> Vec<f64> {
        let n = (sample_rate as f64 * duration_sec) as usize;
        (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sample_rate as f64).sin())
            .collect()
    }

    fn add_click(signal: &mut [f64], pos: usize, width: usize, amplitude: f64) {
        for i in 0..width {
            if pos + i < signal.len() {
                signal[pos + i] = amplitude;
            }
        }
    }

    #[test]
    fn test_autocorrelation_fft() {
        let signal = make_sine(100.0, 1000, 0.1);
        let max_lag = 50;

        let r_fft = autocorrelation_fft(&signal, max_lag);

        // Naive version for comparison
        let mut r_naive = vec![0.0; max_lag + 1];
        let n = signal.len();
        for lag in 0..=max_lag {
            let mut sum = 0.0;
            for i in 0..(n - lag) {
                sum += signal[i] * signal[i + lag];
            }
            r_naive[lag] = sum / n as f64;
        }

        // Should match closely
        for i in 0..=max_lag {
            let diff = (r_fft[i] - r_naive[i]).abs();
            assert!(
                diff < 1e-10,
                "Mismatch at lag {}: FFT={}, naive={}, diff={}",
                i,
                r_fft[i],
                r_naive[i],
                diff
            );
        }
    }

    #[test]
    fn test_full_pipeline() {
        let sample_rate = 44100;
        let mut signal = make_sine(440.0, sample_rate, 0.2);
        let original = signal.clone();

        add_click(&mut signal, 1000, 8, 0.85);
        add_click(&mut signal, 4000, 12, -0.9);
        add_click(&mut signal, 7000, 5, 0.95);

        let declicker = Declicker::new(sample_rate as u32);
        let fixed = declicker.process(&signal);

        let mut noise_before = 0.0;
        let mut noise_after = 0.0;
        for i in 0..signal.len() {
            noise_before += (signal[i] - original[i]).powi(2);
            noise_after += (fixed[i] - original[i]).powi(2);
        }

        let snr_improvement = 10.0 * (noise_before / noise_after.max(1e-10)).log10();
        println!("SNR improvement: {:.2} dB", snr_improvement);
        assert!(snr_improvement > 5.0);
    }

    #[test]
    fn test_no_clicks() {
        let sample_rate = 44100;
        let signal = make_sine(440.0, sample_rate, 0.1);
        let declicker = Declicker::new(sample_rate as u32);
        let bursts = declicker.detect(&signal);
        assert!(bursts.is_empty(), "Should not detect clicks in clean sine");
    }
}
