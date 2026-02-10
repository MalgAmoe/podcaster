use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ProcessResponse {
    pub job_id: Uuid,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub active_jobs: usize,
    pub completed_jobs: usize,
    /// Number of available processing slots (0 = at capacity)
    pub available_slots: usize,
}
