//! Audio analysis module
//!
//! Provides spectral analysis, LUFS measurement, and shared utilities.

pub mod lufs;
mod spectral;
pub mod utils;

pub use spectral::SpectralAnalysis;
