//! Distortion and saturation effects

pub mod channel9;
pub mod tape;

pub use channel9::{Channel9, StereoChannel9};
pub use tape::{StereoTapeGlue, StereoTapeHysteresis, TapeGlue, TapeHysteresis};
