#![allow(dead_code)]

pub(crate) mod common;

// Re-export common constants so they're accessible from both modules
#[allow(unused_imports)]
pub use common::*;

#[cfg(feature = "cli")]
pub mod denoiser;

#[cfg(feature = "plugin")]
pub mod denoiser_rt;

// Re-export everything from the active module
#[cfg(feature = "cli")]
#[allow(unused_imports)]
pub use denoiser::*;

#[cfg(feature = "plugin")]
#[allow(unused_imports)]
pub use denoiser_rt::*;
