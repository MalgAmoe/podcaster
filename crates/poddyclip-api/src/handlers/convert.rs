use axum::{
    extract::{Multipart, State},
    Json,
};
use serde::Serialize;
use tracing::info;
use uuid::Uuid;

use crate::audio::{decode_audio, encode_wav};
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct ConvertResponse {
    pub s3_key: String,
    pub duration: f64,
    pub sample_rate: u32,
    pub channels: usize,
}

/// POST /convert - Convert any audio format to WAV and upload to S3
///
/// Accepts multipart form data with:
/// - file: The audio file (MP3, WAV, FLAC, AAC, etc.)
/// - user_id: User identifier for S3 path organization
///
/// Returns metadata about the converted file for trial processing.
pub async fn convert_audio(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ConvertResponse>, ApiError> {
    // Ensure S3 storage is configured
    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::Internal("S3 storage not configured".to_string()))?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut user_id: Option<String> = None;

    // Extract multipart fields
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read multipart: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read file: {}", e)))?
                        .to_vec(),
                );
            }
            "user_id" => {
                user_id = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::InvalidRequest(format!("Failed to read user_id: {}", e)))?,
                );
            }
            "filename" => {
                // Allow explicit filename override
                let explicit_name = field
                    .text()
                    .await
                    .map_err(|e| ApiError::InvalidRequest(format!("Failed to read filename: {}", e)))?;
                if !explicit_name.is_empty() {
                    filename = Some(explicit_name);
                }
            }
            _ => {}
        }
    }

    let file_data = file_data.ok_or_else(|| ApiError::InvalidRequest("No file provided".to_string()))?;
    let user_id = user_id.unwrap_or_else(|| "anonymous".to_string());
    let filename = filename.unwrap_or_else(|| "audio".to_string());

    // Check file size
    let size_mb = file_data.len() / (1024 * 1024);
    if size_mb > state.config.max_file_size_mb {
        return Err(ApiError::FileTooLarge(size_mb, state.config.max_file_size_mb));
    }

    info!(
        "Converting file '{}' ({} bytes) for user '{}'",
        filename,
        file_data.len(),
        user_id
    );

    // Decode audio (any format -> PCM samples)
    let (samples, metadata) = decode_audio(&file_data, Some(&filename))
        .map_err(|e| ApiError::InvalidRequest(format!("Failed to decode audio: {}", e)))?;

    // Encode to WAV (16-bit PCM)
    let wav_data = encode_wav(&samples, metadata.sample_rate)
        .map_err(|e| ApiError::Internal(format!("Failed to encode WAV: {}", e)))?;

    // Generate unique filename for S3
    let unique_id = Uuid::new_v4();
    let wav_filename = format!("{}.wav", unique_id);

    // Upload to S3
    let s3_key = storage
        .upload_input(&user_id, &wav_data, &wav_filename)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to upload to S3: {}", e)))?;

    // Calculate duration in seconds
    let duration = metadata.duration_samples as f64 / metadata.sample_rate as f64;

    info!(
        "Converted '{}' -> '{}' ({:.1}s, {}Hz, {} ch)",
        filename, s3_key, duration, metadata.sample_rate, metadata.channels
    );

    Ok(Json(ConvertResponse {
        s3_key,
        duration,
        sample_rate: metadata.sample_rate,
        channels: metadata.channels,
    }))
}
