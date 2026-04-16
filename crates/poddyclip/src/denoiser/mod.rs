//! Unified Spectral Subtraction Denoiser
//!
//! Core implementation in `core.rs`.
//! Analysis functions in `analysis.rs` for offline noise floor estimation.
//! Spectral gating in `spectral_gate.rs` for non-stationary noise.
//! Peak attenuation in `peak_attenuator.rs` for tonal noise.

#![allow(unused_imports)]

pub mod analysis;
pub(crate) mod common;
pub mod core;
pub mod peak_attenuator;
pub mod spectral_gate;

// Re-export common constants
pub use common::*;

// Core exports
pub use core::{DenoiserParams, RealtimeDenoiser, StreamingDenoiser, VisualizationData};

// Analysis exports
pub use analysis::{analyze_audio, AudioAnalysisResult, SimpleAnalysis};

// Spectral gate exports
pub use spectral_gate::{
    get_gate_preset_name, SpectralGate, SpectralGateParams, SPECTRAL_GATE_PRESETS,
    SPECTRAL_GATE_PRESET_NAMES,
};

// Peak attenuator exports
pub use peak_attenuator::{detect_tonal_peaks, PeakAttenuator, PeakAttenuatorParams, PeakProfile};
