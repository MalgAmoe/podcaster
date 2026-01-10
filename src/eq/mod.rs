//! EQ and frequency-domain processing modules
//!
//! Contains filters, de-esser, and EQ processors.

pub mod deesser;
pub mod deesser_analysis;
pub mod dynamic;
pub mod enhanceeq;
pub mod filters;
pub mod fixeq;
pub mod fixeq_analysis;

// Re-exports used by plugin (lib.rs)
pub use filters::{FilterChain, HighPassSlope};
pub use fixeq::FixEq;
pub use enhanceeq::StereoEnhanceEq;
#[allow(unused_imports)]
pub use deesser::DeEsser;
