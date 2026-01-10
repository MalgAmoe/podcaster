#![allow(dead_code)]

pub(crate) mod common;

// Re-export common constants so they're accessible from both modules
#[allow(unused_imports)]
pub use common::*;

// Core denoiser - available for both CLI and plugin
pub mod denoiser_rt;

// Re-export core types for both features
#[allow(unused_imports)]
pub use denoiser_rt::{DenoiserParams, RealtimeDenoiser, StreamingDenoiser, VisualizationData};

// Analysis functions - CLI only (plugin doesn't need them)
#[cfg(feature = "cli")]
pub mod denoiser;

#[cfg(feature = "cli")]
#[allow(unused_imports)]
pub use denoiser::{analyze_audio, AudioAnalysisResult, SimpleAnalysis};

// Legacy alias for CLI migration
#[cfg(feature = "cli")]
pub type SpectralSubtractionDenoiser = RealtimeDenoiser;
