use serde::Serialize;
use uuid::Uuid;

use super::JobProgress;

#[derive(Debug, Serialize)]
pub struct ProcessResponse {
    pub job_id: Uuid,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    pub id: Uuid,
    pub status: String,
    pub progress: JobProgress,
    pub created_at: u64,
    pub updated_at: u64,
    pub error: Option<String>,
    pub input_filename: String,
    pub input_size_bytes: usize,
    pub result_ready: bool,
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
