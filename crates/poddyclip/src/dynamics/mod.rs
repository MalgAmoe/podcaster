//! Dynamic processing modules
//!
//! Contains processors for gain staging, compression/leveling, limiting, and expansion.

pub mod autogain;
pub mod buttercomp;
pub mod expander;
pub mod fetcomp;
pub mod limiter;
pub mod peakcomp;
pub mod peakcomp_analysis;

// Re-exports used by plugin (lib.rs)
pub use buttercomp::ButterComp2;
pub use expander::{Expander, StereoExpander};
pub use fetcomp::{FetCompressor, StereoFetCompressor};
#[allow(unused_imports)]
pub use limiter::StereoRealtimeLimiter;
pub use peakcomp::StereoVcaPeakComp;
