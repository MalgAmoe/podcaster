//! Unified Spectral Subtraction Denoiser
//!
//! Core implementation in `core.rs` is used by both CLI and plugin.
//! Analysis functions in `analysis.rs` are CLI-only (offline noise floor estimation).

#![allow(unused_imports)]

pub(crate) mod common;

// Re-export common constants
pub use common::*;

// Core denoiser - available for both CLI and plugin
pub mod core;

// Shared exports (both features)
pub use core::RealtimeDenoiser;

// CLI-only exports: analysis functions
#[cfg(feature = "cli")]
pub mod analysis;

#[cfg(feature = "cli")]
pub use analysis::{analyze_audio, AudioAnalysisResult, SimpleAnalysis};

// Plugin-only exports: StreamingDenoiser wrapper, params, visualization
#[cfg(feature = "plugin")]
pub use core::{DenoiserParams, StreamingDenoiser, VisualizationData};
