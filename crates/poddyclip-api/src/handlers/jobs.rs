use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::models::{JobStatus, JobStatusResponse};
use crate::state::AppState;

#[derive(Serialize)]
pub struct DeleteResponse {
    pub id: Uuid,
    pub deleted: bool,
    pub message: String,
}

/// GET /jobs/{id} - Get job status
pub async fn get_job_status(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobStatusResponse>, ApiError> {
    let job = state.get_job(&job_id).ok_or(ApiError::JobNotFound(job_id))?;

    // Generate presigned URL if result is in S3
    let download_url = if let (Some(ref s3_key), Some(ref storage)) = (&job.result_s3_key, &state.storage) {
        match storage.presign_get(s3_key).await {
            Ok(url) => Some(url),
            Err(e) => {
                tracing::warn!("Failed to generate presigned URL: {}", e);
                None
            }
        }
    } else {
        None
    };

    let result_ready = job.result.is_some() || job.result_s3_key.is_some();

    Ok(Json(JobStatusResponse {
        id: job.id,
        status: format!("{:?}", job.status).to_lowercase(),
        progress: job.progress,
        created_at: job.created_at,
        updated_at: job.updated_at,
        error: job.error,
        input_filename: job.input_filename,
        input_size_bytes: job.input_size_bytes,
        result_ready,
        download_url,
    }))
}

/// GET /jobs/{id}/result - Get processed audio result (redirect to S3 or stream)
pub async fn get_job_result(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let job = state.get_job(&job_id).ok_or(ApiError::JobNotFound(job_id))?;

    match job.status {
        JobStatus::Completed => {
            // If result is in S3, redirect to presigned URL
            if let (Some(ref s3_key), Some(ref storage)) = (&job.result_s3_key, &state.storage) {
                let presigned_url = storage.presign_get(s3_key).await
                    .map_err(|e| ApiError::Internal(format!("Failed to generate download URL: {}", e)))?;

                return Ok(Response::builder()
                    .status(StatusCode::TEMPORARY_REDIRECT)
                    .header(header::LOCATION, presigned_url)
                    .body(Body::empty())
                    .unwrap()
                    .into_response());
            }

            // Fallback: serve from memory
            let result = job.result.ok_or(ApiError::Internal("Result missing".to_string()))?;
            let content_type = job
                .result_content_type
                .unwrap_or_else(|| "audio/wav".to_string());

            // Determine filename extension based on content type
            let ext = if content_type.contains("mpeg") || content_type.contains("mp3") {
                "mp3"
            } else {
                "wav"
            };

            // Build filename from original input
            let stem = std::path::Path::new(&job.input_filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("audio");
            let filename = format!("{}_processed.{}", stem, ext);

            let body = Body::from((*result).clone());

            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, content_type)
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .header(header::CONTENT_LENGTH, result.len())
                .body(body)
                .unwrap()
                .into_response())
        }
        JobStatus::Failed => Err(ApiError::JobFailed(
            job.error.unwrap_or_else(|| "Unknown error".to_string()),
        )),
        _ => Err(ApiError::JobNotCompleted),
    }
}

/// DELETE /jobs/{id} - Delete a job and its result
pub async fn delete_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ApiError> {
    // Check if job exists
    let job = state.get_job(&job_id).ok_or(ApiError::JobNotFound(job_id))?;

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
