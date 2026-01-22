//! Poddyclip - Audio processing library for podcast enhancement
//!
//! This library provides audio processors for:
//! - Spectral noise reduction (denoiser)
//! - Spectral de-reverb (dereverb)
//! - Dynamic processing (compression, limiting, peak control)
//! - EQ processing (filters, de-esser, enhancement)
//! - Saturation effects
//! - Audio repair (declicking) - offline only
//! - AI-based denoising (DeepFilterNet) - optional, voice-focused

pub mod analysis;
pub mod denoiser;
pub mod dereverb;
pub mod dynamics;
pub mod eq;
pub mod repair;
pub mod saturation;
pub mod stft;
pub mod traits;

#[cfg(feature = "deepfilter")]
pub mod deepfilter;

// Re-export commonly used items
pub use denoiser::{RealtimeDenoiser, StreamingDenoiser, PRESETS};
pub use dereverb::{DeReverbProcessor, DeReverbParams, DEREVERB_PRESETS};
pub use dynamics::{ButterComp2, Expander, StereoExpander, StereoFetCompressor, StereoRealtimeLimiter, StereoVcaPeakComp};
pub use dynamics::limiter::Limiter;
pub use eq::{DeEsser, FilterChain, FixEq, HighPassSlope, RadioVoiceProcessor, StereoDeEsser, StereoEnhanceEq, StereoRadioVoice};
pub use repair::Declicker;
pub use saturation::{Channel9, TapeGlue};
pub use traits::Stereo;

#[cfg(feature = "deepfilter")]
pub use deepfilter::DeepFilterDenoiser;
