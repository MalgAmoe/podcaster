//! Window functions for STFT processing
//!
//! Provides unified window creation to replace scattered implementations.

use std::f32::consts::PI;

use super::types::WindowType;

/// Create a window of the specified type and size
///
/// # Arguments
/// * `size` - Window size in samples
/// * `window_type` - Type of window (Hann or SqrtHann)
///
/// # Returns
/// Vector of window coefficients
pub fn create_window(size: usize, window_type: WindowType) -> Vec<f32> {
    match window_type {
        WindowType::Hann => hann_window(size),
        WindowType::SqrtHann => sqrt_hann_window(size),
    }
}

/// Regular Hann window: 0.5 * (1 - cos(2*PI*i/(n-1)))
///
/// Use for analysis where overlap-add reconstruction is not needed.
/// Provides good frequency resolution with low sidelobes.
pub fn hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (size - 1) as f32).cos()))
        .collect()
}

/// Square root Hann window: sin(PI*i/n)
///
/// Use for 50% overlap-add synthesis. When used for both analysis and
/// synthesis windows with 50% overlap, provides perfect reconstruction.
pub fn sqrt_hann_window(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| (PI * i as f32 / size as f32).sin())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hann_window() {
        let win = hann_window(1024);
        assert_eq!(win.len(), 1024);
        // Hann window starts and ends at 0
        assert!(win[0].abs() < 1e-6);
        // Peak in the middle
        assert!(win[512] > 0.99);
    }

    #[test]
    fn test_sqrt_hann_window() {
        let win = sqrt_hann_window(1024);
        assert_eq!(win.len(), 1024);
        // Sqrt Hann starts at 0
        assert!(win[0].abs() < 1e-6);
        // Peak in the middle (sin(PI/2) = 1)
        assert!(win[512] > 0.99);
        // Near 0 at end
        assert!(win[1023] < 0.01);
    }

    #[test]
    fn test_sqrt_hann_cola() {
        // Verify COLA (Constant Overlap-Add) property for 50% overlap
        let size = 2048;
        let hop = 1024;
        let win = sqrt_hann_window(size);

        // For sqrt_hann with 50% overlap, win[i]^2 + win[i+hop]^2 should = 1
        for i in 0..hop {
            let sum = win[i].powi(2) + win[i + hop].powi(2);
            assert!(
                (sum - 1.0).abs() < 0.01,
                "COLA failed at {}: sum = {}",
                i,
                sum
            );
        }
    }

    #[test]
    fn test_create_window() {
        let hann = create_window(512, WindowType::Hann);
        let sqrt = create_window(512, WindowType::SqrtHann);
        assert_eq!(hann.len(), 512);
        assert_eq!(sqrt.len(), 512);
        // They should be different at edges (Hann is 0, SqrtHann is small)
        // Check near quarter point where they differ more noticeably
        assert!((hann[128] - sqrt[128]).abs() > 0.01);
    }
}
