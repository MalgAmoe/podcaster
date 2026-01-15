use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::task;
use tracing::{error, info};
use uuid::Uuid;

use crate::audio::{decode_audio, encode_mp3, encode_wav};
use crate::error::ApiError;
use crate::models::{Job, JobStatus, OutputFormat, ProcessConfig, ProcessResponse};
use crate::processing::{process_audio, CancelledError};
use crate::state::AppState;

#[derive(Serialize)]
pub struct DeleteResponse {
    pub id: Uuid,
    pub deleted: bool,
    pub message: String,
}

/// DELETE /jobs/{id} - Delete a job and its result
pub async fn delete_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ApiError> {
    // Check if job exists
    let job = state.get_job(&job_id).ok_or(ApiError::JobNotFound(job_id))?;

    // Signal cancellation to the processing task (if still running)
    job.cancel();
    info!("Job {} cancellation requested", job_id);

    // Delete from S3 if present
    if let (Some(ref s3_key), Some(ref storage)) = (&job.result_s3_key, &state.storage) {
        if let Err(e) = storage.delete(s3_key).await {
            tracing::warn!("Failed to delete S3 object {}: {}", s3_key, e);
        }
    }

    // Remove job from state
    state.jobs.remove(&job_id);

    let message = match job.status {
        JobStatus::Queued | JobStatus::Processing => "Job cancelled and removed".to_string(),
        JobStatus::Completed => "Completed job removed".to_string(),
        JobStatus::Failed => "Failed job removed".to_string(),
    };

    Ok(Json(DeleteResponse {
        id: job_id,
        deleted: true,
        message,
    }))
}

/// Request body for creating a job from S3
#[derive(Debug, Deserialize)]
pub struct CreateS3JobRequest {
    pub phoenix_job_id: Option<i64>,
    pub input_s3_key: String,
    /// Original filename (for output naming)
    pub filename: Option<String>,
    pub chain: Option<String>,
    #[serde(default)]
    pub output_format: Option<String>,
    #[serde(default = "default_mp3_bitrate")]
    pub mp3_bitrate: u32,
    /// Webhook URL for status updates (e.g., "http://localhost:4000/api/internal/jobs")
    pub webhook_url: Option<String>,
    /// Secret for webhook authentication
    pub webhook_secret: Option<String>,
}

fn default_mp3_bitrate() -> u32 {
    192
}

/// POST /jobs - Create a job from S3 input
pub async fn create_s3_job(
    State(state): State<AppState>,
    Json(req): Json<CreateS3JobRequest>,
) -> Result<Json<ProcessResponse>, ApiError> {
    // Ensure S3 storage is configured
    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::Internal("S3 storage not configured".to_string()))?;

    // Download input file from S3
    info!("Downloading input from S3: {}", req.input_s3_key);
    let audio_bytes = storage
        .download(&req.input_s3_key)
        .await
        .map_err(|e| ApiError::InvalidRequest(format!("Failed to download from S3: {}", e)))?;

    // Use provided filename or extract from S3 key as fallback
    let filename = req.filename.clone().unwrap_or_else(|| {
        req.input_s3_key
            .split('/')
            .last()
            .unwrap_or("audio.wav")
            .to_string()
    });

    // Check file size
    let size_mb = audio_bytes.len() / (1024 * 1024);
    if size_mb > state.config.max_file_size_mb {
        return Err(ApiError::FileTooLarge(size_mb, state.config.max_file_size_mb));
    }

    // Check concurrent jobs limit
    let (active, _) = state.job_count();
    if active >= state.config.max_concurrent_jobs {
        return Err(ApiError::ServerBusy);
    }

    // Validate chain preset exists if specified
    if let Some(ref chain_name) = req.chain {
        if !state.config.chains_dir.join(format!("{}.toml", chain_name)).exists() {
            return Err(ApiError::ChainNotFound(chain_name.clone()));
        }
    }

    // Build ProcessConfig from request
    let config = ProcessConfig {
        chain: req.chain,
        output_format: match req.output_format.as_deref() {
            Some("wav") => OutputFormat::Wav,
            _ => OutputFormat::Mp3,
        },
        mp3_bitrate: req.mp3_bitrate,
        ..ProcessConfig::default()
    };

    // Create job with webhook info
    let job_id = Uuid::new_v4();
    let job = Job::new(job_id, config.clone(), filename.clone(), audio_bytes.len())
        .with_webhook(req.phoenix_job_id, req.webhook_url.clone(), req.webhook_secret.clone());

    // Get cancellation flag before inserting job (Arc is cloned)
    let cancelled = job.cancelled.clone();
    state.insert_job(job);

    info!(
        "Created job {} from S3 input '{}' ({} bytes)",
        job_id, req.input_s3_key, audio_bytes.len()
    );

    // Spawn processing task
    let state_clone = state.clone();
    let chains_dir = state.config.chains_dir.clone();
    let job_timeout = state.config.job_timeout_seconds;
    let filename_for_upload = filename.clone();
    let webhook_client = state.webhook.clone();

    task::spawn(async move {
        // Update status to processing
        state_clone.update_job(&job_id, |j| {
            j.status = JobStatus::Processing;
            j.progress.update("decoding", 0);
            j.updated_at = now();
        });

        // Send webhook for processing start
        if let Some(job) = state_clone.get_job(&job_id) {
            webhook_client.notify(&job, None).await;
        }

        // Clone state for use inside blocking task
        let progress_state = state_clone.clone();
        let progress_webhook = webhook_client.clone();
        let runtime_handle = tokio::runtime::Handle::current();

        // Run processing in blocking task (CPU-bound) with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(job_timeout),
            task::spawn_blocking(move || {
                // Decode audio
                let (mut samples, metadata) = decode_audio(&audio_bytes, Some(&filename))?;

                // Progress callback that updates job state, sends webhook, and checks for cancellation
                let progress_callback = Box::new(move |stage: &str, index: u8| -> Result<(), CancelledError> {
                    // Check if job was cancelled
                    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                        info!("Job {} cancelled at stage {}", job_id, stage);
                        return Err(CancelledError);
                    }

                    tracing::debug!("Job {} progress: {} ({}/17)", job_id, stage, index);
                    progress_state.update_job(&job_id, |j| {
                        j.progress.update(stage, index);
                        j.updated_at = now();
                    });

                    // Send webhook for progress update
                    if let Some(job) = progress_state.get_job(&job_id) {
                        let webhook = progress_webhook.clone();
                        runtime_handle.block_on(async move {
                            webhook.notify(&job, None).await;
                        });
                    }

                    Ok(())
                });

                // Process with progress updates
                process_audio(
                    &mut samples,
                    metadata.sample_rate,
                    &config,
                    &chains_dir,
                    Some(progress_callback),
                )?;

                // Encode output
                let (output_bytes, content_type) = match config.output_format {
                    OutputFormat::Wav => (encode_wav(&samples, metadata.sample_rate)?, "audio/wav".to_string()),
                    OutputFormat::Mp3 => (
                        encode_mp3(&samples, metadata.sample_rate, config.mp3_bitrate)?,
                        "audio/mpeg".to_string(),
                    ),
                };

                Ok::<_, anyhow::Error>((output_bytes, content_type))
            }),
        )
        .await;

        match result {
            Ok(Ok(Ok((output_bytes, content_type)))) => {
                // Upload to S3 if storage is configured
                let s3_key = if let Some(ref storage) = state_clone.storage {
                    let extension = if content_type == "audio/mpeg" { ".mp3" } else { ".wav" };
                    let output_filename = format!(
                        "{}_processed{}",
                        std::path::Path::new(&filename_for_upload)
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        extension
                    );

                    match storage
                        .upload_result(job_id, &output_bytes, &content_type, &output_filename)
                        .await
                    {
                        Ok(key) => {
                            info!("Job {} result uploaded to S3: {}", job_id, key);
                            Some(key)
                        }
                        Err(e) => {
                            error!("Job {} failed to upload to S3: {}. Keeping in memory.", job_id, e);
                            None
                        }
                    }
                } else {
                    None
                };

                // Generate presigned download URL if we have S3
                let download_url = if let (Some(ref key), Some(ref storage)) = (&s3_key, &state_clone.storage) {
                    storage.presign_get(key).await.ok()
                } else {
                    None
                };

                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Completed;
                    // Only keep in memory if S3 upload failed
                    if s3_key.is_none() {
                        j.result = Some(Arc::new(output_bytes));
                    }
                    j.result_content_type = Some(content_type);
                    j.result_s3_key = s3_key;
                    j.progress.update("completed", 24);
                    j.updated_at = now();
                });
                info!("Job {} completed successfully", job_id);

                // Send webhook for completion
                if let Some(job) = state_clone.get_job(&job_id) {
                    webhook_client.notify(&job, download_url).await;
                }
            }
            Ok(Ok(Err(e))) => {
                error!("Job {} failed: {}", job_id, e);
                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some(e.to_string());
                    j.updated_at = now();
                });

                // Send webhook for failure
                if let Some(job) = state_clone.get_job(&job_id) {
                    webhook_client.notify(&job, None).await;
                }
            }
            Ok(Err(e)) => {
                error!("Job {} panicked: {}", job_id, e);
                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some("Internal processing error".to_string());
                    j.updated_at = now();
                });

                // Send webhook for failure
                if let Some(job) = state_clone.get_job(&job_id) {
                    webhook_client.notify(&job, None).await;
                }
            }
            Err(_) => {
                error!("Job {} timed out", job_id);
                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some("Processing timed out".to_string());
                    j.updated_at = now();
                });

                // Send webhook for timeout
                if let Some(job) = state_clone.get_job(&job_id) {
                    webhook_client.notify(&job, None).await;
                }
            }
        }
    });

    Ok(Json(ProcessResponse {
        job_id,
        status: "queued".to_string(),
        message: "Processing started".to_string(),
    }))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
