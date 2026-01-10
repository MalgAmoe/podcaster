//! Audio analysis module
//!
//! Provides spectral analysis, LUFS measurement, and shared utilities.

#[cfg(feature = "cli")]
pub mod lufs;
mod spectral;
pub mod utils;

pub use spectral::SpectralAnalysis;
