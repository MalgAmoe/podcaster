//! Dynamic processing modules
//!
//! Contains processors for gain staging and compression/leveling.

pub mod autogain;
pub mod buttercomp;

// Re-export commonly used items (CLI only - plugin doesn't use autogain)
#[cfg(feature = "cli")]
#[allow(unused_imports)]
pub use autogain::{
    analyze_gain, apply_gain, calculate_rms, calculate_rms_stereo, rms_to_db, DEFAULT_TARGET_RMS_DB,
};
pub use buttercomp::StereoButterComp2;
