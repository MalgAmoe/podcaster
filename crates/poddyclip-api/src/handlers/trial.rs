use axum::{
    body::Body,
    extract::State,
    http::{header, Response, StatusCode},
    Json,
};
use serde::Deserialize;
use tracing::info;

use crate::audio::{decode_audio, encode_opus, encode_wav};
use crate::error::ApiError;
use crate::models::{OutputFormat, ProcessConfig};
use crate::processing::process_audio;
use crate::state::AppState;

/// WAV header size (standard 44 bytes for PCM)
const WAV_HEADER_SIZE: u64 = 44;

#[derive(Debug, Deserialize)]
pub struct TrialRequest {
    /// S3 key of the original audio file (any format)
    pub s3_key: String,
    /// Start time in seconds
    pub start_time: f64,
    /// Duration in seconds (typically 10)
    #[serde(default = "default_duration")]
    pub duration: f64,
    /// Processing chain name
    pub chain: Option<String>,
}

fn default_duration() -> f64 {
    10.0
}

/// Derive WAV key from original key
/// e.g., "inputs/123/abc.mp3" -> "inputs/123/abc.wav"
fn wav_key_from_original(original_key: &str) -> String {
    if let Some(dot_pos) = original_key.rfind('.') {
        format!("{}.wav", &original_key[..dot_pos])
    } else {
        format!("{}.wav", original_key)
    }
}

/// POST /trial - Process a short segment and return Opus audio
///
/// On first request for a file:
/// 1. Downloads original from S3
/// 2. Converts to WAV (16-bit PCM)
/// 3. Uploads WAV to S3
/// 4. Extracts segment via byte-range
/// 5. Processes and returns Opus
///
/// Subsequent requests use cached WAV directly.
pub async fn process_trial(
    State(state): State<AppState>,
    Json(req): Json<TrialRequest>,
) -> Result<Response<Body>, ApiError> {
    let storage = state
        .storage
        .as_ref()
        .ok_or_else(|| ApiError::Internal("S3 storage not configured".to_string()))?;

    // Validate chain preset exists if specified
    if let Some(ref chain_name) = req.chain {
        if !state.config.chains_dir.join(format!("{}.toml", chain_name)).exists() {
            return Err(ApiError::ChainNotFound(chain_name.clone()));
        }
    }

    let wav_key = wav_key_from_original(&req.s3_key);

    // Try to get WAV metadata - if it fails, we need to convert
    let (sample_rate, channels, total_duration) = match get_wav_metadata(storage, &wav_key).await {
        Ok(meta) => {
            info!("Using cached WAV: {}", wav_key);
            meta
        }
        Err(_) => {
            // WAV doesn't exist - download original, convert, upload
            info!("Converting {} to WAV...", req.s3_key);

            let original_bytes = storage
                .download(&req.s3_key)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to download original: {}", e)))?;

            // Decode original (any format)
            let filename = req.s3_key.split('/').last().unwrap_or("audio");
            let (samples, metadata) = decode_audio(&original_bytes, Some(filename))
                .map_err(|e| ApiError::Internal(format!("Failed to decode audio: {}", e)))?;

            // Encode to WAV
            let wav_bytes = encode_wav(&samples, metadata.sample_rate)
                .map_err(|e| ApiError::Internal(format!("Failed to encode WAV: {}", e)))?;

            // Upload WAV
            storage
                .upload_wav(&wav_key, &wav_bytes)
                .await
                .map_err(|e| ApiError::Internal(format!("Failed to upload WAV: {}", e)))?;

            let duration = metadata.duration_samples as f64 / metadata.sample_rate as f64;
            info!(
                "Converted and cached WAV: {} ({:.1}s, {}Hz, {} ch)",
                wav_key, duration, metadata.sample_rate, metadata.channels
            );

            (metadata.sample_rate, metadata.channels, duration)
        }
    };

    // Ensure start_time is valid
    let start_time = req.start_time.max(0.0).min(total_duration - req.duration);
    let duration = req.duration.min(total_duration - start_time);

    // Calculate byte range for the WAV data section
    let bytes_per_sample: u64 = 2; // 16-bit PCM
    let bytes_per_frame: u64 = channels as u64 * bytes_per_sample;

    let start_sample = (start_time * sample_rate as f64) as u64;
    let duration_samples = (duration * sample_rate as f64) as u64;

    let start_byte = WAV_HEADER_SIZE + (start_sample * bytes_per_frame);
    let end_byte = WAV_HEADER_SIZE + ((start_sample + duration_samples) * bytes_per_frame);

    info!(
        "Trial: {:.1}s-{:.1}s, bytes {}-{}",
        start_time, start_time + duration, start_byte, end_byte
    );

    // Download the segment
    let data_bytes = storage
        .download_range(&wav_key, start_byte, end_byte - 1)
        .await
        .map_err(|e| ApiError::Internal(format!("Failed to download segment: {}", e)))?;

    // Construct WAV from raw PCM
    info!(
        "WAV metadata: {}Hz, {} channels, {} bytes PCM",
        sample_rate, channels, data_bytes.len()
    );
    let wav_bytes = construct_wav_segment(&data_bytes, sample_rate, channels as u8);

    // Decode the constructed WAV
    let (mut samples, meta) = decode_audio(&wav_bytes, Some("segment.wav"))
        .map_err(|e| ApiError::Internal(format!("Failed to decode segment: {}", e)))?;

    // Check decoded sample range
    let (min_dec, max_dec) = samples.iter().fold((f32::MAX, f32::MIN), |(min, max), ch| {
        ch.iter().fold((min, max), |(min, max), &s| (min.min(s), max.max(s)))
    });

    info!(
        "Decoded: {} channels x {} samples, {}Hz (meta: {}Hz, {} ch), range [{:.3}, {:.3}]",
        samples.len(),
        samples.first().map(|c| c.len()).unwrap_or(0),
        sample_rate,
        meta.sample_rate,
        meta.channels,
        min_dec,
        max_dec
    );

    // Process
    let config = ProcessConfig {
        chain: req.chain.clone(),
        output_format: OutputFormat::Wav,
        ..ProcessConfig::default()
    };

    info!("Processing with chain: {:?}", req.chain);

    process_audio(
        &mut samples,
        sample_rate,
        &config,
        &state.config.chains_dir,
        None,
    )
    .map_err(|e| ApiError::Internal(format!("Processing failed: {}", e)))?;

    // Check sample ranges
    let (min_sample, max_sample) = samples.iter().fold((f32::MAX, f32::MIN), |(min, max), ch| {
        ch.iter().fold((min, max), |(min, max), &s| (min.min(s), max.max(s)))
    });

    info!(
        "After processing: {} channels x {} samples, range [{:.3}, {:.3}]",
        samples.len(),
        samples.first().map(|c| c.len()).unwrap_or(0),
        min_sample,
        max_sample
    );

    // Encode to Opus
    let opus_bytes = encode_opus(&samples, sample_rate)
        .map_err(|e| ApiError::Internal(format!("Failed to encode Opus: {}", e)))?;

    info!(
        "Trial complete: {:.1}s -> {} bytes Opus",
        duration,
        opus_bytes.len()
    );

    // Return Opus with metadata headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/ogg")
        .header(header::CONTENT_LENGTH, opus_bytes.len())
        .header("X-Wav-Key", &wav_key)
        .header("X-Sample-Rate", sample_rate.to_string())
        .header("X-Channels", channels.to_string())
        .header("X-Duration", total_duration.to_string())
        .body(Body::from(opus_bytes))
        .map_err(|e| ApiError::Internal(format!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// Try to get WAV metadata by reading header
async fn get_wav_metadata(
    storage: &crate::storage::Storage,
    wav_key: &str,
) -> Result<(u32, usize, f64), ApiError> {
    // Download just the header
    let header = storage
        .download_range(wav_key, 0, WAV_HEADER_SIZE - 1)
        .await
        .map_err(|e| ApiError::Internal(format!("WAV not found: {}", e)))?;

    if header.len() < 44 {
        return Err(ApiError::Internal("Invalid WAV header".to_string()));
    }

    // Parse WAV header
    // Bytes 22-23: channels (u16 LE)
    let channels = u16::from_le_bytes([header[22], header[23]]) as usize;
    // Bytes 24-27: sample rate (u32 LE)
    let sample_rate = u32::from_le_bytes([header[24], header[25], header[26], header[27]]);
    // Bytes 40-43: data size (u32 LE)
    let data_size = u32::from_le_bytes([header[40], header[41], header[42], header[43]]);

    let bytes_per_sample = 2u32; // 16-bit
    let total_samples = data_size / (channels as u32 * bytes_per_sample);
    let duration = total_samples as f64 / sample_rate as f64;

    Ok((sample_rate, channels, duration))
}

/// Construct a valid WAV file from raw PCM data
fn construct_wav_segment(data: &[u8], sample_rate: u32, channels: u8) -> Vec<u8> {
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = bits_per_sample / 8;
    let block_align = channels as u16 * bytes_per_sample;
    let byte_rate = sample_rate * block_align as u32;
    let data_size = data.len() as u32;
    let file_size = 36 + data_size;

    let mut buffer = Vec::with_capacity(44 + data.len());

    // RIFF header
    buffer.extend_from_slice(b"RIFF");
    buffer.extend_from_slice(&file_size.to_le_bytes());
    buffer.extend_from_slice(b"WAVE");

    // fmt subchunk
    buffer.extend_from_slice(b"fmt ");
    buffer.extend_from_slice(&16u32.to_le_bytes());
    buffer.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buffer.extend_from_slice(&(channels as u16).to_le_bytes());
    buffer.extend_from_slice(&sample_rate.to_le_bytes());
    buffer.extend_from_slice(&byte_rate.to_le_bytes());
    buffer.extend_from_slice(&block_align.to_le_bytes());
    buffer.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    buffer.extend_from_slice(b"data");
    buffer.extend_from_slice(&data_size.to_le_bytes());
    buffer.extend_from_slice(data);

    buffer
}
