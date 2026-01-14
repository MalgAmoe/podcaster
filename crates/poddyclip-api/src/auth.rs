use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// Middleware to validate API key from X-API-Key header.
/// If no API key is configured, all requests are allowed (dev mode).
pub async fn require_api_key(
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // If no API key configured, allow all requests (dev mode)
    let Some(expected_key) = &state.config.api_key else {
        return Ok(next.run(request).await);
    };

    // Check X-API-Key header
    let provided_key = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok());

    match provided_key {
        Some(key) if constant_time_eq(key.as_bytes(), expected_key.as_bytes()) => {
            Ok(next.run(request).await)
        }
        _ => {
            tracing::warn!("Unauthorized API request - invalid or missing API key");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}
