//! De-Reverb Processor
//!
//! Spectral gating de-reverb that reduces room reverb in recordings.
//! Uses reverb analysis from cepstral domain to estimate decay rates.

#![allow(unused_imports)]

pub(crate) mod common;
pub mod core;

// Re-export common constants
pub use common::*;

// Core exports
pub use core::DeReverbProcessor;
