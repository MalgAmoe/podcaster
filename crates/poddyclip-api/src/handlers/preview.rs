use axum::{
    body::Bytes,
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
};
use tokio::task;
use tracing::{info, warn};

use crate::audio::{decode_audio, encode_wav};
use crate::error::ApiError;
use crate::models::ProcessConfig;
use crate::processing::process_audio;
use crate::state::AppState;

/// POST /preview - Process a short preview clip synchronously and return WAV bytes.
pub async fn preview(
    State(state): State<AppState>,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let _preview_permit = state
        .preview_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::PreviewBusy)?;

    let _processing_permit = state
        .processing_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| ApiError::PreviewBusy)?;

    let audio_bytes = body.to_vec();
    let bytes_in = audio_bytes.len();
    let job_timeout = state.config.job_timeout_seconds;
    let preview_limit = state.config.preview_max_seconds;
    let preview_tolerance = state.config.preview_tolerance_seconds;

    let wav_bytes = tokio::time::timeout(
        std::time::Duration::from_secs(job_timeout),
        task::spawn_blocking(move || {
            let (mut samples, metadata) = decode_audio(&audio_bytes, None)
                .map_err(|e| ApiError::DecodeFailed(e.to_string()))?;

            let decoded_duration =
                metadata.duration_samples as f64 / metadata.sample_rate as f64;
            let allowed_duration = preview_limit as f64 + preview_tolerance;

            if decoded_duration > allowed_duration {
                warn!(
                    decoded_duration,
                    allowed_duration,
                    "Preview rejected for exceeding duration limit"
                );
                return Err(ApiError::PreviewTooLong {
                    max_seconds: preview_limit,
                    tolerance_seconds: preview_tolerance,
                });
            }

            process_audio(&mut samples, metadata.sample_rate, &ProcessConfig::new(), None)
                .map_err(|e| ApiError::ProcessingError(e.to_string()))?;

            encode_wav(&samples, metadata.sample_rate)
                .map_err(|e| ApiError::ProcessingError(e.to_string()))
        }),
    )
    .await
    .map_err(|_| ApiError::ProcessingError("Preview processing timed out".to_string()))?
    .map_err(|e| ApiError::ProcessingError(e.to_string()))??;

    info!(
        bytes_in,
        bytes_out = wav_bytes.len(),
        "Preview processed successfully"
    );

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "audio/wav")],
        wav_bytes,
    ))
}
