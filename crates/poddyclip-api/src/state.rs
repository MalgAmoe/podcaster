use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::models::{Job, JobStatus};
use crate::storage::Storage;
use crate::webhook::WebhookClient;

const DEFAULT_MAX_FILE_SIZE_MB: usize = 2048;

/// Maximum concurrent processing tasks (matches CCX13 2 vCPU)
const MAX_CONCURRENT_PROCESSING: usize = 2;
const DEFAULT_PREVIEW_MAX_CONCURRENCY: usize = 1;

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<DashMap<Uuid, Job>>,
    pub config: Arc<AppConfig>,
    pub storage: Option<Arc<Storage>>,
    pub webhook: WebhookClient,
    /// Semaphore to limit concurrent CPU-bound processing tasks
    pub processing_semaphore: Arc<Semaphore>,
    /// Semaphore to reserve preview traffic to a smaller dedicated capacity
    pub preview_semaphore: Arc<Semaphore>,
}

pub struct AppConfig {
    pub max_file_size_mb: usize,
    pub job_timeout_seconds: u64,
    pub result_retention_seconds: u64,
    pub preview_max_seconds: u32,
    pub preview_tolerance_seconds: f64,
    pub preview_max_concurrency: usize,
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
            preview_max_seconds: 30,
            preview_tolerance_seconds: 2.0,
            preview_max_concurrency: DEFAULT_PREVIEW_MAX_CONCURRENCY,
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
            preview_max_seconds: std::env::var("PREVIEW_MAX_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            preview_tolerance_seconds: std::env::var("PREVIEW_TOLERANCE_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2.0),
            preview_max_concurrency: std::env::var("PREVIEW_MAX_CONCURRENCY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_PREVIEW_MAX_CONCURRENCY),
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
        let preview_max_concurrency = config.preview_max_concurrency;
        Self {
            jobs: Arc::new(DashMap::new()),
            config: Arc::new(config),
            storage: storage.map(Arc::new),
            webhook: WebhookClient::new(),
            processing_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSING)),
            preview_semaphore: Arc::new(Semaphore::new(preview_max_concurrency)),
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
