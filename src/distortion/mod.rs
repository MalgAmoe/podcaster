//! Distortion and saturation effects

#![allow(unused_imports)]

pub mod channel9;
pub mod tape;

pub use channel9::Channel9;
pub use tape::TapeGlue;
// TapeHysteresis available via tape:: if needed
