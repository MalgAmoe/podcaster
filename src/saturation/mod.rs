//! Saturation effects

pub mod channel9;
pub mod tape;

pub use channel9::Channel9;
#[allow(unused_imports)] // Used by lib.rs (plugin)
pub use tape::TapeGlue;
