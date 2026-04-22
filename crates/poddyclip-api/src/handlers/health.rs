use axum::{extract::State, Json};

use crate::models::HealthResponse;
use crate::state::AppState;

/// GET /health - Health check endpoint
pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let (active, completed) = state.job_count();

    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        active_jobs: active,
        completed_jobs: completed,
        available_slots: state.processing_semaphore.available_permits(),
    })
}

/// GET /ping - RunPod readiness/health endpoint
pub async fn ping(State(state): State<AppState>) -> Json<HealthResponse> {
    health(State(state)).await
}
