//! Audio repair modules for offline processing
//!
//! These processors require looking at future samples and cannot run in real-time.

mod declicker;

pub use declicker::{ClickDetector, Declicker, Interpolator};
