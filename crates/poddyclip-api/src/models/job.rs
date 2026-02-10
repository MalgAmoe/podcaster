use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

use crate::processing::get_total_stages;
use super::ProcessConfig;

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub status: JobStatus,
    pub progress: JobProgress,
    pub created_at: u64,
    pub updated_at: u64,
    pub config: ProcessConfig,
    #[serde(skip)]
    pub result: Option<Arc<Vec<u8>>>,
    #[serde(skip)]
    pub result_content_type: Option<String>,
    /// S3 object key for the result (if stored in S3)
    pub result_s3_key: Option<String>,
    pub error: Option<String>,
    pub input_filename: String,
    pub input_size_bytes: usize,
    /// Actual audio duration in seconds (set after decoding)
    pub audio_duration_seconds: Option<u32>,
    /// User ID (for logging and S3 paths)
    #[serde(skip)]
    pub user_id: Option<i64>,
    /// External job ID from Phoenix (for webhook callbacks)
    #[serde(skip)]
    pub phoenix_job_id: Option<i64>,
    /// Webhook URL to notify on status changes
    #[serde(skip)]
    pub webhook_url: Option<String>,
    /// Webhook secret for authentication
    #[serde(skip)]
    pub webhook_secret: Option<String>,
    /// Cancellation flag - set to true to request job cancellation
    #[serde(skip)]
    pub cancelled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct JobProgress {
    pub stage: String,
    pub stage_index: u8,
    pub total_stages: u8,
    pub percent_complete: u8,
}

impl JobProgress {
    pub fn new() -> Self {
        Self {
            stage: "queued".to_string(),
            stage_index: 0,
            total_stages: get_total_stages() + 1, // +1 for "waiting" stage
            percent_complete: 0,
        }
    }

    pub fn update(&mut self, stage: &str, index: u8) {
        self.stage = stage.to_string();
        self.stage_index = index;
        self.percent_complete = ((index as f32 / self.total_stages as f32) * 100.0) as u8;
    }
}

impl Job {
    pub fn new(id: Uuid, config: ProcessConfig, input_filename: String, input_size_bytes: usize) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            status: JobStatus::Queued,
            progress: JobProgress::new(),
            created_at: now,
            updated_at: now,
            config,
            result: None,
            result_content_type: None,
            result_s3_key: None,
            error: None,
            input_filename,
            input_size_bytes,
            audio_duration_seconds: None,
            user_id: None,
            phoenix_job_id: None,
            webhook_url: None,
            webhook_secret: None,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_webhook(mut self, user_id: Option<i64>, phoenix_job_id: Option<i64>, webhook_url: Option<String>, webhook_secret: Option<String>) -> Self {
        self.user_id = user_id;
        self.phoenix_job_id = phoenix_job_id;
        self.webhook_url = webhook_url;
        self.webhook_secret = webhook_secret;
        self
    }

    /// Request cancellation of this job
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Check if cancellation has been requested
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
