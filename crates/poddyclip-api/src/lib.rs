//! Production HTTP service and orchestration layer for the Poddyclip DSP library.
//!
//! `poddyclip-api` is the production processing path used by the Phoenix app.
//! It owns job orchestration, storage, webhooks, and the narrower web-facing
//! processing pipeline built on top of the broader `poddyclip` library.

pub mod audio;
pub mod auth;
pub mod error;
pub mod handlers;
pub mod models;
pub mod processing;
pub mod state;
pub mod storage;
pub mod webhook;

pub use auth::require_api_key;
pub use error::ApiError;
pub use state::{AppConfig, AppState};
pub use storage::{Storage, StorageConfig};
pub use webhook::WebhookClient;
