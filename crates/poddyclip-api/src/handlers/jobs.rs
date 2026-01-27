use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tokio::task;
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

use crate::audio::{decode_audio, encode_mp3, encode_wav};
use crate::error::ApiError;
use crate::models::{Job, JobStatus, OutputFormat, ProcessConfig, ProcessResponse};
use crate::processing::{process_audio, CancelledError};
use crate::state::AppState;
use crate::storage::Storage;

/// Maximum retry attempts for S3 uploads
const S3_UPLOAD_MAX_RETRIES: u32 = 3;
/// Initial retry delay for S3 uploads (doubles each attempt)
const S3_UPLOAD_INITIAL_DELAY_MS: u64 = 1000;

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
            warn!("Failed to delete S3 object {}: {}", s3_key, e);
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
    /// User ID for output path (results/{user_id}/output.ext)
    pub user_id: Option<i64>,
    /// Original filename (for output naming)
    pub filename: Option<String>,
    /// Legacy chain preset (deprecated, use category/mode/strength)
    pub chain: Option<String>,
    /// Audio category: "voice" or "mixed"
    pub category: Option<String>,
    /// Processing mode: "repair", "natural", or "studio"
    pub mode: Option<String>,
    /// Processing strength: 1-5
    pub strength: Option<u8>,
    #[serde(default)]
    pub output_format: Option<String>,
    #[serde(default = "default_mp3_bitrate")]
    pub mp3_bitrate: u32,
    /// Webhook URL for status updates (e.g., "http://localhost:4000/api/internal/jobs")
    pub webhook_url: Option<String>,
    /// Secret for webhook authentication
    pub webhook_secret: Option<String>,
    /// Enable AI (DeepFilterNet) denoiser for voice cleaning
    #[serde(default)]
    pub ai_clean: Option<bool>,
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
    info!(
        user_id = req.user_id.unwrap_or(-1),
        input_key = %req.input_s3_key,
        "Downloading input from S3"
    );
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

    // Build ProcessConfig from request
    // If category/mode/strength are provided, use dynamic builder
    // Otherwise fall back to chain preset (legacy) or defaults
    let mut config = if req.category.is_some() || req.mode.is_some() || req.strength.is_some() {
        ProcessConfig::from_dynamic(
            req.category.as_deref(),
            req.mode.as_deref(),
            req.strength,
        )
    } else if let Some(ref chain_name) = req.chain {
        // Legacy chain preset support
        if !state.config.chains_dir.join(format!("{}.toml", chain_name)).exists() {
            return Err(ApiError::ChainNotFound(chain_name.clone()));
        }
        ProcessConfig {
            chain: req.chain,
            ..ProcessConfig::default()
        }
    } else {
        // Default to voice/natural/3
        ProcessConfig::from_dynamic(None, None, None)
    };

    // Apply output format settings
    config.output_format = match req.output_format.as_deref() {
        Some("wav") => OutputFormat::Wav,
        _ => OutputFormat::Mp3,
    };
    config.mp3_bitrate = req.mp3_bitrate;

    // Allow explicit override of ai_denoise
    if let Some(ai_clean) = req.ai_clean {
        config.ai_denoise = ai_clean;
    }

    // Validate webhook URL if provided (SSRF prevention)
    if let Some(ref webhook_url) = req.webhook_url {
        if let Err(reason) = validate_webhook_url(webhook_url) {
            return Err(ApiError::InvalidRequest(format!(
                "Invalid webhook URL: {}",
                reason
            )));
        }
    }

    // Create job with webhook info
    let job_id = Uuid::new_v4();
    let job = Job::new(job_id, config.clone(), filename.clone(), audio_bytes.len())
        .with_webhook(req.user_id, req.phoenix_job_id, req.webhook_url.clone(), req.webhook_secret.clone());

    // Get cancellation flag before inserting job (Arc is cloned)
    let cancelled = job.cancelled.clone();
    let user_id_for_upload = req.user_id;
    state.insert_job(job);

    info!(
        job_id = %job_id,
        user_id = user_id_for_upload.unwrap_or(-1),
        input_key = %req.input_s3_key,
        size_bytes = audio_bytes.len(),
        "Job created from S3 input"
    );

    // Spawn processing task
    let state_clone = state.clone();
    let chains_dir = state.config.chains_dir.clone();
    let job_timeout = state.config.job_timeout_seconds;
    let filename_for_upload = filename.clone();
    let webhook_client = state.webhook.clone();

    task::spawn(async move {
        let start_time = Instant::now();

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

                    debug!("Job {} progress: {} ({}/17)", job_id, stage, index);
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
                // Upload to S3 with retry if storage is configured and user_id is present
                let s3_key = if let (Some(ref storage), Some(user_id)) = (&state_clone.storage, user_id_for_upload) {
                    let extension = if content_type == "audio/mpeg" { ".mp3" } else { ".wav" };

                    upload_with_retry(
                        storage,
                        user_id,
                        &output_bytes,
                        &content_type,
                        &filename_for_upload,
                        extension,
                        job_id,
                    )
                    .await
                } else if state_clone.storage.is_some() && user_id_for_upload.is_none() {
                    error!("Job {} has no user_id, cannot upload to S3", job_id);
                    None
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

                let duration_ms = start_time.elapsed().as_millis();
                info!(
                    job_id = %job_id,
                    user_id = user_id_for_upload.unwrap_or(-1),
                    duration_ms = duration_ms,
                    "Job completed successfully"
                );

                // Send webhook for completion
                if let Some(job) = state_clone.get_job(&job_id) {
                    webhook_client.notify(&job, download_url).await;
                }
            }
            Ok(Ok(Err(e))) => {
                let duration_ms = start_time.elapsed().as_millis();
                error!(
                    job_id = %job_id,
                    user_id = user_id_for_upload.unwrap_or(-1),
                    duration_ms = duration_ms,
                    error = %e,
                    "Job failed"
                );
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
                let duration_ms = start_time.elapsed().as_millis();
                error!(
                    job_id = %job_id,
                    user_id = user_id_for_upload.unwrap_or(-1),
                    duration_ms = duration_ms,
                    error = %e,
                    "Job panicked"
                );
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
                let duration_ms = start_time.elapsed().as_millis();
                error!(
                    job_id = %job_id,
                    user_id = user_id_for_upload.unwrap_or(-1),
                    duration_ms = duration_ms,
                    timeout_seconds = job_timeout,
                    "Job timed out"
                );
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

/// Validate webhook URL to prevent SSRF attacks.
///
/// Security model:
/// - Internal URLs (localhost, private IPs): Allow HTTP (trusted internal network)
/// - External URLs: Require HTTPS (untrusted)
/// - Always block dangerous endpoints (cloud metadata, etc.)
fn validate_webhook_url(url_str: &str) -> Result<(), &'static str> {
    let parsed = Url::parse(url_str).map_err(|_| "Invalid webhook URL")?;

    // Only allow http or https schemes
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err("Webhook URL must use HTTP or HTTPS");
    }

    // Check host
    let host = parsed.host_str().ok_or("Webhook URL must have a host")?;
    let host_lower = host.to_lowercase();

    // Always block cloud metadata endpoints (SSRF to steal credentials)
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_metadata_ip(&ip) {
            return Err("Webhook URL cannot point to cloud metadata endpoints");
        }
    }

    // Check if this is an internal/trusted URL
    let is_internal = host_lower == "localhost"
        || host_lower == "127.0.0.1"
        || host_lower == "::1"
        || host_lower.ends_with(".local")
        || host_lower.ends_with(".localhost")
        || host.parse::<IpAddr>().map(|ip| is_private_ip(&ip)).unwrap_or(false);

    // External URLs must use HTTPS
    if !is_internal && parsed.scheme() != "https" {
        return Err("External webhook URLs must use HTTPS");
    }

    Ok(())
}

/// Check if an IP address is in a private/internal range (trusted for HTTP)
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()          // 127.0.0.0/8
                || ipv4.is_private()    // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                || ipv4.is_link_local() // 169.254.0.0/16 (except metadata)
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
        }
    }
}

/// Check if an IP is a cloud metadata endpoint (always block - credential theft risk)
fn is_metadata_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            // AWS/GCP/Azure metadata endpoint
            ipv4.octets() == [169, 254, 169, 254]
        }
        IpAddr::V6(_) => false,
    }
}

/// Upload to S3 with exponential backoff retry
async fn upload_with_retry(
    storage: &Storage,
    user_id: i64,
    data: &[u8],
    content_type: &str,
    filename: &str,
    extension: &str,
    job_id: Uuid,
) -> Option<String> {
    let mut delay_ms = S3_UPLOAD_INITIAL_DELAY_MS;

    for attempt in 1..=S3_UPLOAD_MAX_RETRIES {
        match storage
            .upload_result(user_id, data, content_type, filename, extension)
            .await
        {
            Ok(key) => {
                info!(
                    job_id = %job_id,
                    user_id = user_id,
                    s3_key = %key,
                    attempt = attempt,
                    "Job result uploaded to S3"
                );
                return Some(key);
            }
            Err(e) => {
                if attempt < S3_UPLOAD_MAX_RETRIES {
                    warn!(
                        job_id = %job_id,
                        user_id = user_id,
                        error = %e,
                        attempt = attempt,
                        retry_in_ms = delay_ms,
                        "S3 upload failed, retrying"
                    );
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    delay_ms *= 2;
                } else {
                    error!(
                        job_id = %job_id,
                        user_id = user_id,
                        error = %e,
                        attempts = S3_UPLOAD_MAX_RETRIES,
                        "S3 upload failed after all retries, keeping in memory"
                    );
                }
            }
        }
    }
    None
}
