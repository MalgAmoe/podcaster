//! Unified Spectral Subtraction Denoiser
//!
//! Core implementation in `core.rs` is used by both CLI and plugin.
//! Analysis functions in `analysis.rs` for offline noise floor estimation.

#![allow(unused_imports)]

pub(crate) mod common;
pub mod core;
pub mod analysis;

// Re-export common constants
pub use common::*;

// Core exports
pub use core::{DenoiserParams, RealtimeDenoiser, StreamingDenoiser, VisualizationData};

// Analysis exports
pub use analysis::{analyze_audio, AudioAnalysisResult, SimpleAnalysis};
