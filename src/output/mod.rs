//! Output processing modules
//!
//! Contains LUFS measurement for loudness normalization.

pub mod lufs;

#[allow(unused_imports)]
pub use lufs::{calculate_gain_for_target, measure_integrated_lufs, DEFAULT_TARGET_LUFS};
