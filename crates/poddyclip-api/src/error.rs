use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Job not found: {0}")]
    JobNotFound(uuid::Uuid),

    #[error("Job not completed yet")]
    JobNotCompleted,

    #[error("Job failed: {0}")]
    JobFailed(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Could not decode audio: {0}")]
    DecodeFailed(String),

    #[error("Unsupported audio format: {0}")]
    UnsupportedFormat(String),

    #[error("File too large: {0} MB (max: {1} MB)")]
    FileTooLarge(usize, usize),

    #[error("Processing error: {0}")]
    ProcessingError(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Preview exceeds allowed duration")]
    PreviewTooLong {
        max_seconds: u32,
        tolerance_seconds: f64,
    },

    #[error("Preview service is busy")]
    PreviewBusy,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            ApiError::JobNotFound(_) => (StatusCode::NOT_FOUND, "job_not_found", self.to_string()),
            ApiError::JobNotCompleted => {
                (StatusCode::CONFLICT, "job_not_completed", self.to_string())
            }
            ApiError::JobFailed(m) => (StatusCode::OK, "job_failed", m.clone()),
            ApiError::InvalidRequest(m) => (StatusCode::BAD_REQUEST, "invalid_request", m.clone()),
            ApiError::DecodeFailed(m) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "decode_failed", m.clone())
            }
            ApiError::UnsupportedFormat(f) => {
                (StatusCode::BAD_REQUEST, "unsupported_format", f.clone())
            }
            ApiError::FileTooLarge(size, max) => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "file_too_large",
                format!("{} MB exceeds {} MB limit", size, max),
            ),
            ApiError::ProcessingError(m) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "processing_error",
                m.clone(),
            ),
            ApiError::Internal(m) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                m.clone(),
            ),
            ApiError::PreviewTooLong {
                max_seconds,
                tolerance_seconds,
            } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "too_long",
                format!(
                    "Preview exceeds {}s limit (+{}s tolerance)",
                    max_seconds, tolerance_seconds
                ),
            ),
            ApiError::PreviewBusy => (
                StatusCode::SERVICE_UNAVAILABLE,
                "preview_busy",
                "Preview service is busy".to_string(),
            ),
        };

        let mut payload = json!({
            "error": {
                "type": error_type,
                "message": message,
            }
        });

        if let ApiError::PreviewTooLong {
            max_seconds,
            tolerance_seconds,
        } = &self
        {
            payload["max_seconds"] = json!(max_seconds);
            payload["tolerance_seconds"] = json!(tolerance_seconds);
        }

        let body = Json(payload);

        (status, body).into_response()
    }
}
