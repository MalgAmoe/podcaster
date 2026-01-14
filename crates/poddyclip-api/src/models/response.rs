use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ProcessResponse {
    pub job_id: Uuid,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PresetInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct PresetsResponse {
    pub chain_presets: Vec<PresetInfo>,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub active_jobs: usize,
    pub completed_jobs: usize,
}
