//! Webhook client for notifying Phoenix of job status changes.

use reqwest::Client;
use serde::Serialize;
use tracing::{error, info};

use crate::models::{Job, JobProgress, JobStatus};

/// Webhook payload sent to Phoenix
#[derive(Debug, Serialize)]
pub struct WebhookPayload {
    pub status: String,
    pub progress: JobProgress,
    pub error: Option<String>,
    pub download_url: Option<String>,
    pub result_s3_key: Option<String>,
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
    /// This is fire-and-forget - errors are logged but not propagated.
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
        };

        let mut request = self.client.post(&url).json(&payload);

        if let Some(secret) = webhook_secret {
            request = request.header("X-Webhook-Secret", secret);
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        "Webhook sent for job {} (phoenix_id={}): status={}",
                        job.id, phoenix_job_id, payload.status
                    );
                } else {
                    error!(
                        "Webhook failed for job {} (phoenix_id={}): HTTP {}",
                        job.id,
                        phoenix_job_id,
                        response.status()
                    );
                }
            }
            Err(e) => {
                error!(
                    "Webhook request failed for job {} (phoenix_id={}): {}",
                    job.id, phoenix_job_id, e
                );
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
