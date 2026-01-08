//! Fixed filters for audio cleanup (HP + LP "safety net")
//!
//! Removes rumble, plosives, hiss, and aliasing noise using
//! numerically stable State Variable Filters (SVF).

pub mod common;
pub mod dynamic;

// Re-export common types
#[cfg_attr(feature = "cli", allow(unused_imports))]
pub use common::{FilterChain, HighPassSlope, HighShelfSvf, SvfBiquad};

// CLI also uses StereoFilterChain
#[cfg(feature = "cli")]
#[allow(unused_imports)]
pub use common::StereoFilterChain;
