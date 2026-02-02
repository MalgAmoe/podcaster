//! Webhook client for notifying Phoenix of job status changes.

use std::time::Duration;

use reqwest::Client;
use serde::Serialize;
use tracing::{debug, error, info, warn};

use crate::models::{Job, JobProgress, JobStatus};

/// Maximum number of retry attempts for webhook notifications
const MAX_RETRIES: u32 = 3;
/// Initial delay before first retry (doubles with each attempt)
const INITIAL_RETRY_DELAY_MS: u64 = 1000;

/// Webhook payload sent to Phoenix
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub status: String,
    pub progress: JobProgress,
    pub error: Option<String>,
    pub download_url: Option<String>,
    pub result_s3_key: Option<String>,
    /// Actual audio duration in seconds (for billing)
    pub audio_duration_seconds: Option<u32>,
}

/// HTTP client for sending webhooks
#[derive(Clone)]
pub struct WebhookClient {
    client: Client,
}

impl WebhookClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Send a webhook notification for a job status change.
    /// Uses exponential backoff retry for reliability.
    pub async fn notify(&self, job: &Job, download_url: Option<String>) {
        let (webhook_url, phoenix_job_id, webhook_secret) = match (
            &job.webhook_url,
            job.phoenix_job_id,
            &job.webhook_secret,
        ) {
            (Some(url), Some(id), secret) => (url.clone(), id, secret.clone()),
            _ => return, // No webhook configured
        };

        let url = format!("{}/{}/status", webhook_url, phoenix_job_id);
        let payload = WebhookPayload {
            status: status_to_string(job.status),
            progress: job.progress.clone(),
            error: job.error.clone(),
            download_url,
            result_s3_key: job.result_s3_key.clone(),
            audio_duration_seconds: job.audio_duration_seconds,
        };

        let is_terminal = matches!(payload.status.as_str(), "completed" | "failed");
        let mut delay_ms = INITIAL_RETRY_DELAY_MS;

        for attempt in 1..=MAX_RETRIES {
            let mut request = self.client.post(&url).json(&payload);

            if let Some(ref secret) = webhook_secret {
                request = request.header("X-Webhook-Secret", secret.clone());
            }

            match request.send().await {
                Ok(response) if response.status().is_success() => {
                    // Success - log appropriately and return
                    if is_terminal {
                        info!(
                            job_id = %job.id,
                            user_id = job.user_id.unwrap_or(-1),
                            phoenix_id = phoenix_job_id,
                            status = %payload.status,
                            attempt = attempt,
                            "Webhook sent"
                        );
                    } else {
                        debug!(
                            job_id = %job.id,
                            user_id = job.user_id.unwrap_or(-1),
                            phoenix_id = phoenix_job_id,
                            status = %payload.status,
                            "Webhook sent"
                        );
                    }
                    return;
                }
                Ok(response) => {
                    // HTTP error response
                    if attempt < MAX_RETRIES {
                        warn!(
                            job_id = %job.id,
                            user_id = job.user_id.unwrap_or(-1),
                            phoenix_id = phoenix_job_id,
                            http_status = %response.status(),
                            attempt = attempt,
                            retry_in_ms = delay_ms,
                            "Webhook failed, retrying"
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        delay_ms *= 2; // Exponential backoff
                    } else {
                        error!(
                            job_id = %job.id,
                            user_id = job.user_id.unwrap_or(-1),
                            phoenix_id = phoenix_job_id,
                            http_status = %response.status(),
                            attempts = MAX_RETRIES,
                            "Webhook failed after all retries"
                        );
                    }
                }
                Err(e) => {
                    // Network error
                    if attempt < MAX_RETRIES {
                        warn!(
                            job_id = %job.id,
                            user_id = job.user_id.unwrap_or(-1),
                            phoenix_id = phoenix_job_id,
                            error = %e,
                            attempt = attempt,
                            retry_in_ms = delay_ms,
                            "Webhook request failed, retrying"
                        );
                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        delay_ms *= 2;
                    } else {
                        error!(
                            job_id = %job.id,
                            user_id = job.user_id.unwrap_or(-1),
                            phoenix_id = phoenix_job_id,
                            error = %e,
                            attempts = MAX_RETRIES,
                            "Webhook request failed after all retries"
                        );
                    }
                }
            }
        }
    }
}

fn status_to_string(status: JobStatus) -> String {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Processing => "processing",
        JobStatus::Completed => "completed",
        JobStatus::Failed => "failed",
    }
    .to_string()
}

impl Default for WebhookClient {
    fn default() -> Self {
        Self::new()
    }
}
