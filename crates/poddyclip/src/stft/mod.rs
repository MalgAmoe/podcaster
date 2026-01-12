//! Shared STFT (Short-Time Fourier Transform) infrastructure
//!
//! This module provides unified FFT processing to eliminate code duplication
//! across audio processors. Instead of each processor implementing its own
//! FFT setup, window creation, and overlap-add, they compose with `StftProcessor`.
//!
//! # Window Sizes
//!
//! Two window sizes are provided:
//! - **Real-time (2048/1024)**: Lower latency for processors like denoiser, dereverb
//! - **Analysis (4096/2048)**: Better frequency resolution for spectral analysis
//!
//! # Usage
//!
//! ```ignore
//! use poddyclip::stft::{StftProcessor, StftConfig};
//!
//! // Create processor with real-time config
//! let mut stft = StftProcessor::new_realtime(48000);
//!
//! // Process audio with custom spectrum modification
//! let output = stft.process(&audio, |power, spectrum| {
//!     // Modify spectrum bins based on power analysis
//!     for (k, s) in spectrum[..n_bins].iter_mut().enumerate() {
//!         *s *= calculate_gain(power[k]);
//!     }
//! });
//! ```

pub mod processor;
pub mod types;
pub mod window;

// Re-export main types
pub use processor::StftProcessor;
pub use types::{
    StftConfig, WindowType, ANALYSIS_HOP_SIZE, ANALYSIS_N_BINS, ANALYSIS_WINDOW_SIZE, EPSILON,
    RT_HOP_SIZE, RT_N_BINS, RT_WINDOW_SIZE,
};
pub use window::{create_window, hann_window, sqrt_hann_window};
