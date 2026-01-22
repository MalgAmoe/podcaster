//! DeepFilterNet-based AI denoiser for voice enhancement
//!
//! This module provides a deep learning-based noise reduction processor
//! optimized for voice/speech enhancement. It uses the DeepFilterNet model
//! which is specifically trained for speech denoising.
//!
//! # Features
//!
//! - Auto-tuned noise reduction based on SNR analysis
//! - Automatic resampling for non-48kHz audio
//! - Voice-focused processing (not for music)
//!
//! # Example
//!
//! ```ignore
//! use poddyclip::deepfilter::{DeepFilterDenoiser, analyze_for_deepfilter};
//!
//! // Analyze audio for optimal settings
//! let analysis = analyze_for_deepfilter(&samples, sample_rate);
//! println!("Estimated SNR: {:.1}dB", analysis.estimated_snr);
//!
//! // Create and process
//! let mut denoiser = DeepFilterDenoiser::new(sample_rate)?;
//! let output = denoiser.process_with_analysis(&samples, &analysis);
//! ```

pub mod analysis;
pub mod core;

pub use analysis::{analyze_for_deepfilter, DfAnalysis};
pub use core::DeepFilterDenoiser;
