use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::models::{Job, JobStatus};
use crate::storage::Storage;
use crate::webhook::WebhookClient;

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<DashMap<Uuid, Job>>,
    pub config: Arc<AppConfig>,
    pub storage: Option<Arc<Storage>>,
    pub webhook: WebhookClient,
}

pub struct AppConfig {
    pub max_file_size_mb: usize,
    pub max_concurrent_jobs: usize,
    pub job_timeout_seconds: u64,
    pub result_retention_seconds: u64,
    pub chains_dir: PathBuf,
    pub port: u16,
    pub api_key: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            max_file_size_mb: 100,
            max_concurrent_jobs: 4,
            job_timeout_seconds: 600,
            result_retention_seconds: 3600,
            chains_dir: PathBuf::from("chains"),
            port: 3000,
            api_key: None,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            max_file_size_mb: std::env::var("MAX_FILE_SIZE_MB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
            max_concurrent_jobs: std::env::var("MAX_CONCURRENT_JOBS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(4),
            job_timeout_seconds: std::env::var("JOB_TIMEOUT_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600),
            result_retention_seconds: std::env::var("RESULT_RETENTION_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600),
            chains_dir: std::env::var("CHAINS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("chains")),
            port: std::env::var("PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000),
            api_key: std::env::var("API_KEY").ok().filter(|s| !s.is_empty()),
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
