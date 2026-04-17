//! Production processing pipeline used by the HTTP API.
//!
//! This layer intentionally exposes a narrower, fixed chain than the internal
//! CLI. The DSP library remains broader for experimentation.

pub mod engine;

pub use engine::{get_total_stages, process_audio, CancelledError, ProgressCallback, ProgressUpdate};
