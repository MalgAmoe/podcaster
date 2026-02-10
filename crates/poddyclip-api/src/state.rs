use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::models::{Job, JobStatus};
use crate::storage::Storage;
use crate::webhook::WebhookClient;

const DEFAULT_MAX_FILE_SIZE_MB: usize = 500;

/// Maximum concurrent processing tasks (matches CCX13 2 vCPU)
const MAX_CONCURRENT_PROCESSING: usize = 2;

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<DashMap<Uuid, Job>>,
    pub config: Arc<AppConfig>,
    pub storage: Option<Arc<Storage>>,
    pub webhook: WebhookClient,
    /// Semaphore to limit concurrent CPU-bound processing tasks
    pub processing_semaphore: Arc<Semaphore>,
}

pub struct AppConfig {
    pub max_file_size_mb: usize,
    pub job_timeout_seconds: u64,
    pub result_retention_seconds: u64,
    pub port: u16,
    pub api_key: Option<String>,
    /// Comma-separated list of allowed CORS origins. Empty = allow any (dev mode).
    pub cors_origins: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            max_file_size_mb: DEFAULT_MAX_FILE_SIZE_MB,
            job_timeout_seconds: 600,
            result_retention_seconds: 3600,
            port: 3000,
            api_key: None,
            cors_origins: None, // None = allow any (dev mode)
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            max_file_size_mb: std::env::var("MAX_FILE_SIZE_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MAX_FILE_SIZE_MB),
            job_timeout_seconds: std::env::var("JOB_TIMEOUT_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600),
            result_retention_seconds: std::env::var("RESULT_RETENTION_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600),
            port: std::env::var("PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000),
            api_key: std::env::var("API_KEY").ok().filter(|s| !s.is_empty()),
            cors_origins: std::env::var("CORS_ORIGINS").ok().filter(|s| !s.is_empty()),
        }
    }
}

impl AppState {
    pub fn new(config: AppConfig, storage: Option<Storage>) -> Self {
        Self {
            jobs: Arc::new(DashMap::new()),
            config: Arc::new(config),
            storage: storage.map(Arc::new),
            webhook: WebhookClient::new(),
            processing_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSING)),
        }
    }

    pub fn insert_job(&self, job: Job) {
        self.jobs.insert(job.id, job);
    }

    pub fn get_job(&self, id: &Uuid) -> Option<Job> {
        self.jobs.get(id).map(|j| j.clone())
    }

    pub fn update_job<F>(&self, id: &Uuid, f: F)
    where
        F: FnOnce(&mut Job),
    {
        if let Some(mut job) = self.jobs.get_mut(id) {
            f(&mut job);
        }
    }

    pub fn job_count(&self) -> (usize, usize) {
        let active = self
            .jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Queued | JobStatus::Processing))
            .count();
        let completed = self
            .jobs
            .iter()
            .filter(|j| matches!(j.status, JobStatus::Completed))
            .count();
        (active, completed)
    }
}
