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

pub use buttercomp::{
    get_buttercomp_preset, get_buttercomp_preset_name, ButterComp2, BUTTERCOMP_PRESETS,
    BUTTERCOMP_PRESET_NAMES,
};
pub use expander::{
    get_expander_preset_name, Expander, ExpanderPreset, StereoExpander, EXPANDER_PRESETS,
    EXPANDER_PRESET_NAMES,
};
pub use fetcomp::{
    get_fetcomp_preset_name, FetCompPreset, FetCompressor, StereoFetCompressor, FETCOMP_PRESETS,
    FETCOMP_PRESET_NAMES,
};
#[allow(unused_imports)]
pub use limiter::StereoRealtimeLimiter;
pub use peakcomp::{
    get_peakcomp_preset_name, PeakCompPreset, StereoVcaPeakComp, PEAKCOMP_PRESETS,
    PEAKCOMP_PRESET_NAMES,
};
