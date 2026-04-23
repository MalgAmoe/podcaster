use dashmap::DashMap;
#[cfg(feature = "mossformer2")]
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
#[cfg(feature = "mossformer2")]
use tokio::runtime::Handle;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

#[cfg(feature = "mossformer2")]
use poddyclip::ai_clean::AiCleanRuntime;

use crate::models::{Job, JobStatus};
use crate::storage::Storage;
use crate::webhook::WebhookClient;

const DEFAULT_MAX_FILE_SIZE_MB: usize = 2048;

/// Maximum concurrent processing tasks (matches CCX13 2 vCPU)
const MAX_CONCURRENT_PROCESSING: usize = 2;
const DEFAULT_PREVIEW_MAX_CONCURRENCY: usize = 1;
#[cfg(feature = "mossformer2")]
const DEFAULT_AI_CLEAN_POOL_SIZE: usize = 2;

#[derive(Clone)]
pub struct AppState {
    pub jobs: Arc<DashMap<Uuid, Job>>,
    pub config: Arc<AppConfig>,
    pub storage: Option<Arc<Storage>>,
    pub webhook: WebhookClient,
    #[cfg(feature = "mossformer2")]
    pub ai_clean_runtime_pool: Arc<AiCleanRuntimePool>,
    pub active_background_tasks: Arc<AtomicUsize>,
    pub shutdown_notify: Arc<Notify>,
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
    pub ai_clean_use_cuda: bool,
    #[cfg(feature = "mossformer2")]
    pub ai_clean_pool_size: usize,
    /// Comma-separated list of allowed CORS origins. Empty = allow any (dev mode).
    pub cors_origins: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            max_file_size_mb: DEFAULT_MAX_FILE_SIZE_MB,
            job_timeout_seconds: 600,
            result_retention_seconds: 3600,
            preview_max_seconds: 20,
            preview_tolerance_seconds: 2.0,
            preview_max_concurrency: DEFAULT_PREVIEW_MAX_CONCURRENCY,
            port: 3000,
            api_key: None,
            ai_clean_use_cuda: true,
            #[cfg(feature = "mossformer2")]
            ai_clean_pool_size: DEFAULT_AI_CLEAN_POOL_SIZE,
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
            ai_clean_use_cuda: std::env::var("AI_CLEAN_USE_CUDA")
                .ok()
                .map(|s| matches!(s.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
                .unwrap_or(true),
            #[cfg(feature = "mossformer2")]
            ai_clean_pool_size: std::env::var("AI_CLEAN_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .map(|n| n.max(1))
                .unwrap_or(DEFAULT_AI_CLEAN_POOL_SIZE),
            cors_origins: std::env::var("CORS_ORIGINS").ok().filter(|s| !s.is_empty()),
        }
    }
}

#[cfg(feature = "mossformer2")]
pub struct AiCleanRuntimePool {
    available: Arc<Mutex<Vec<Arc<AiCleanRuntime>>>>,
    semaphore: Arc<Semaphore>,
    size: usize,
}

#[cfg(feature = "mossformer2")]
impl AiCleanRuntimePool {
    pub fn new(runtimes: Vec<AiCleanRuntime>) -> Self {
        let size = runtimes.len();
        let available = runtimes.into_iter().map(Arc::new).collect();

        Self {
            available: Arc::new(Mutex::new(available)),
            semaphore: Arc::new(Semaphore::new(size)),
            size,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub async fn acquire(&self) -> Result<AiCleanRuntimeLease, &'static str> {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "AI clean runtime pool closed")?;

        let runtime = self
            .available
            .lock()
            .map_err(|_| "AI clean runtime pool poisoned")?
            .pop()
            .ok_or("AI clean runtime unavailable")?;

        Ok(AiCleanRuntimeLease {
            runtime: Some(runtime),
            available: self.available.clone(),
            _permit: permit,
        })
    }

    pub fn acquire_blocking(&self) -> Result<AiCleanRuntimeLease, &'static str> {
        Handle::current().block_on(self.acquire())
    }
}

#[cfg(feature = "mossformer2")]
pub struct AiCleanRuntimeLease {
    runtime: Option<Arc<AiCleanRuntime>>,
    available: Arc<Mutex<Vec<Arc<AiCleanRuntime>>>>,
    _permit: OwnedSemaphorePermit,
}

#[cfg(feature = "mossformer2")]
impl AiCleanRuntimeLease {
    pub fn runtime(&self) -> Arc<AiCleanRuntime> {
        self.runtime
            .as_ref()
            .expect("runtime lease should always hold a runtime")
            .clone()
    }
}

#[cfg(feature = "mossformer2")]
impl Drop for AiCleanRuntimeLease {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            if let Ok(mut available) = self.available.lock() {
                available.push(runtime);
            }
        }
    }
}

impl AppState {
    pub fn new(
        config: AppConfig,
        storage: Option<Storage>,
        #[cfg(feature = "mossformer2")] ai_clean_runtime_pool: AiCleanRuntimePool,
    ) -> Self {
        let preview_max_concurrency = config.preview_max_concurrency;
        Self {
            jobs: Arc::new(DashMap::new()),
            config: Arc::new(config),
            storage: storage.map(Arc::new),
            webhook: WebhookClient::new(),
            #[cfg(feature = "mossformer2")]
            ai_clean_runtime_pool: Arc::new(ai_clean_runtime_pool),
            active_background_tasks: Arc::new(AtomicUsize::new(0)),
            shutdown_notify: Arc::new(Notify::new()),
            processing_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSING)),
            preview_semaphore: Arc::new(Semaphore::new(preview_max_concurrency)),
        }
    }

    pub fn begin_background_task(&self) -> BackgroundTaskGuard {
        self.active_background_tasks.fetch_add(1, Ordering::SeqCst);
        BackgroundTaskGuard {
            active_background_tasks: self.active_background_tasks.clone(),
            shutdown_notify: self.shutdown_notify.clone(),
        }
    }

    pub fn active_background_task_count(&self) -> usize {
        self.active_background_tasks.load(Ordering::SeqCst)
    }

    pub async fn wait_for_background_tasks(&self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;

        loop {
            let notified = self.shutdown_notify.notified();

            if self.active_background_task_count() == 0 {
                return true;
            }

            let now = std::time::Instant::now();
            if now >= deadline {
                return false;
            }

            if tokio::time::timeout(deadline - now, notified)
                .await
                .is_err()
            {
                return self.active_background_task_count() == 0;
            }
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

pub struct BackgroundTaskGuard {
    active_background_tasks: Arc<AtomicUsize>,
    shutdown_notify: Arc<Notify>,
}

impl Drop for BackgroundTaskGuard {
    fn drop(&mut self) {
        let previous = self.active_background_tasks.fetch_sub(1, Ordering::SeqCst);
        if previous == 1 {
            self.shutdown_notify.notify_waiters();
        }
    }
}

#[cfg(all(test, feature = "mossformer2"))]
mod tests {
    use super::*;
    use poddyclip::ai_clean::AiCleanRuntime;
    use tokio::time::{timeout, Duration};

    fn build_test_pool(size: usize) -> AiCleanRuntimePool {
        let runtimes = (0..size)
            .map(|_| AiCleanRuntime::new_with_cuda(48_000, false).expect("cpu runtime should initialize"))
            .collect();
        AiCleanRuntimePool::new(runtimes)
    }

    #[tokio::test]
    async fn ai_clean_pool_uses_requested_size() {
        let pool = build_test_pool(2);
        assert_eq!(pool.size(), 2);
    }

    #[tokio::test]
    async fn ai_clean_pool_returns_capacity_on_drop() {
        let pool = build_test_pool(1);

        let lease = pool.acquire().await.expect("first lease");
        assert!(timeout(Duration::from_millis(50), pool.acquire()).await.is_err());
        drop(lease);

        let _lease = timeout(Duration::from_secs(1), pool.acquire())
            .await
            .expect("second acquire should stop waiting")
            .expect("lease after drop");
    }

    #[tokio::test]
    async fn ai_clean_pool_blocks_third_acquire_until_release() {
        let pool = Arc::new(build_test_pool(2));

        let lease1 = pool.acquire().await.expect("lease 1");
        let lease2 = pool.acquire().await.expect("lease 2");
        let pool_for_waiter = pool.clone();

        let mut waiter = tokio::spawn(async move { pool_for_waiter.acquire().await });

        assert!(timeout(Duration::from_millis(50), &mut waiter).await.is_err());

        drop(lease1);

        let lease3 = timeout(Duration::from_secs(1), waiter)
            .await
            .expect("third acquire should eventually complete")
            .expect("waiter task should succeed")
            .expect("third lease should be returned");

        drop(lease2);
        drop(lease3);
    }
}
