//! Distortion and saturation effects

pub mod channel9;
pub mod tape;

pub use channel9::Channel9;
pub use tape::{TapeGlue, TapeHysteresis};
