//! Poddyclip - Audio processing library for podcast enhancement
//!
//! This library provides audio processors for:
//! - Spectral noise reduction (denoiser)
//! - Spectral de-reverb (dereverb)
//! - Dynamic processing (compression, limiting, peak control)
//! - EQ processing (filters, de-esser, enhancement)
//! - Saturation effects
//! - Audio repair (declicking) - offline only
//! - AI-based denoising (MossFormer2) - optional, voice-focused
//!
//! This crate stays intentionally broad so the API and internal CLI can
//! experiment without trimming library surface area to current production use.

#[cfg(feature = "mossformer2")]
pub mod ai_clean;
pub mod analysis;
pub mod denoiser;
pub mod dereverb;
pub mod dynamics;
pub mod eq;
pub mod repair;
#[cfg(feature = "mossformer2")]
pub mod sample_rate;
pub mod saturation;
pub mod stft;
pub mod traits;

// Re-export commonly used items
#[cfg(feature = "mossformer2")]
pub use ai_clean::{AiCleanProcessor, AiCleanRuntime};
pub use denoiser::{RealtimeDenoiser, StreamingDenoiser, PRESETS};
pub use dereverb::{DeReverbParams, DeReverbProcessor, DEREVERB_PRESETS};
pub use dynamics::limiter::Limiter;
pub use dynamics::{
    ButterComp2, Expander, StereoExpander, StereoFetCompressor, StereoRealtimeLimiter,
    StereoVcaPeakComp,
};
pub use eq::{
    DeEsser, FilterChain, FixEq, HighPassSlope, RadioVoiceProcessor, StereoDeEsser,
    StereoEnhanceEq, StereoRadioVoice,
};
pub use repair::Declicker;
pub use saturation::{Channel9, TapeGlue};
pub use traits::Stereo;
