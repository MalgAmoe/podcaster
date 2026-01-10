//! Poddyclip - Audio processing library for podcast enhancement
//!
//! This library provides audio processors for:
//! - Spectral noise reduction (denoiser)
//! - Dynamic processing (compression, limiting, peak control)
//! - EQ processing (filters, de-esser, enhancement)
//! - Saturation effects

pub mod analysis;
pub mod denoiser;
pub mod dynamics;
pub mod eq;
pub mod saturation;
pub mod traits;

// Re-export commonly used items
pub use denoiser::{RealtimeDenoiser, StreamingDenoiser, PRESETS};
pub use dynamics::{ButterComp2, StereoRealtimeLimiter, StereoVcaPeakComp};
pub use dynamics::limiter::Limiter;
pub use eq::{DeEsser, FilterChain, FixEq, HighPassSlope, StereoDeEsser, StereoEnhanceEq};
pub use saturation::{Channel9, TapeGlue};
pub use traits::Stereo;
