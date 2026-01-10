//! Dynamic processing modules
//!
//! Contains processors for gain staging, compression/leveling, and limiting.

pub mod autogain;
pub mod buttercomp;
pub mod limiter;

// Re-export commonly used items (CLI only - plugin doesn't use autogain)
#[cfg(feature = "cli")]
#[allow(unused_imports)]
pub use autogain::{
    analyze_gain, apply_gain, calculate_rms, calculate_rms_stereo, linear_to_db,
    DEFAULT_TARGET_RMS_DB, DEFAULT_TARGET_PEAK_DB,
};
pub use buttercomp::ButterComp2;
#[allow(unused_imports)]
pub use limiter::{Limiter, LimiterStats, RealtimeLimiter, StereoRealtimeLimiter};
