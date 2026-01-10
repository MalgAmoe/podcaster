//! Fixed filters for audio cleanup (HP + LP "safety net")
//!
//! Removes rumble, plosives, hiss, and aliasing noise using
//! numerically stable State Variable Filters (SVF).

#![allow(dead_code)]

pub mod common;
pub mod dynamic;

// Re-export common types
pub use common::{FilterChain, HighPassSlope, HighShelfSvf, SvfBiquad};
