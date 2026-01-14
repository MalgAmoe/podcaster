use std::sync::Arc;

use axum::{
    extract::{Multipart, State},
    Json,
};
use tokio::task;
use tracing::{error, info};
use uuid::Uuid;

use crate::audio::{decode_audio, encode_mp3, encode_wav};
use crate::error::ApiError;
use crate::models::{Job, JobStatus, OutputFormat, ProcessConfig, ProcessResponse};
use crate::processing::process_audio;
use crate::state::AppState;

/// POST /process - Upload audio file and start processing
pub async fn process_audio_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ProcessResponse>, ApiError> {
    let mut audio_data: Option<Vec<u8>> = None;
    let mut audio_filename: Option<String> = None;
    let mut config = ProcessConfig::default();

    // Parse multipart form
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read multipart: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "audio" | "file" => {
                audio_filename = field.file_name().map(|s| s.to_string());
                audio_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read audio: {}", e)))?
                        .to_vec(),
                );
            }
            "config" => {
                let config_str = field
                    .text()
                    .await
                    .map_err(|e| ApiError::InvalidRequest(format!("Failed to read config: {}", e)))?;
                config = serde_json::from_str(&config_str)
                    .map_err(|e| ApiError::InvalidRequest(format!("Invalid config JSON: {}", e)))?;
            }
            _ => {
                // Ignore unknown fields
            }
        }
    }

    // Validate audio was provided
    let audio_bytes = audio_data.ok_or_else(|| ApiError::InvalidRequest("No audio file provided".to_string()))?;
    let filename = audio_filename.unwrap_or_else(|| "audio.wav".to_string());

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
    if let Some(ref chain_name) = config.chain {
        if !state.config.chains_dir.join(format!("{}.toml", chain_name)).exists() {
            return Err(ApiError::ChainNotFound(chain_name.clone()));
        }
    }

    // Create job
    let job_id = Uuid::new_v4();
    let job = Job::new(job_id, config.clone(), filename.clone(), audio_bytes.len());
    state.insert_job(job);

    info!("Created job {} for file '{}' ({} bytes)", job_id, filename, audio_bytes.len());

    // Spawn processing task
    let state_clone = state.clone();
    let chains_dir = state.config.chains_dir.clone();
    let job_timeout = state.config.job_timeout_seconds;
    let filename_for_upload = filename.clone();

    task::spawn(async move {
        // Update status to processing
        state_clone.update_job(&job_id, |j| {
            j.status = JobStatus::Processing;
            j.progress.update("decoding", 0);
            j.updated_at = now();
        });

        // Clone state for use inside blocking task
        let progress_state = state_clone.clone();

        // Run processing in blocking task (CPU-bound) with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(job_timeout),
            task::spawn_blocking(move || {
                // Decode audio
                let (mut samples, metadata) = decode_audio(&audio_bytes, Some(&filename))?;

                // Progress callback that updates job state
                let progress_callback = Box::new(move |stage: &str, index: u8| {
                    tracing::debug!("Job {} progress: {} ({}/17)", job_id, stage, index);
                    progress_state.update_job(&job_id, |j| {
                        j.progress.update(stage, index);
                        j.updated_at = now();
                    });
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
                    let output_filename = format!("{}_processed{}",
                        std::path::Path::new(&filename_for_upload).file_stem().unwrap_or_default().to_string_lossy(),
                        extension
                    );

                    match storage.upload_result(job_id, &output_bytes, &content_type, &output_filename).await {
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

                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Completed;
                    // Only keep in memory if S3 upload failed
                    if s3_key.is_none() {
                        j.result = Some(Arc::new(output_bytes));
                    }
                    j.result_content_type = Some(content_type);
                    j.result_s3_key = s3_key;
                    j.progress.update("completed", 17);
                    j.updated_at = now();
                });
                info!("Job {} completed successfully", job_id);
            }
            Ok(Ok(Err(e))) => {
                error!("Job {} failed: {}", job_id, e);
                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some(e.to_string());
                    j.updated_at = now();
                });
            }
            Ok(Err(e)) => {
                error!("Job {} panicked: {}", job_id, e);
                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some("Internal processing error".to_string());
                    j.updated_at = now();
                });
            }
            Err(_) => {
                error!("Job {} timed out", job_id);
                state_clone.update_job(&job_id, |j| {
                    j.status = JobStatus::Failed;
                    j.error = Some("Processing timed out".to_string());
                    j.updated_at = now();
                });
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
