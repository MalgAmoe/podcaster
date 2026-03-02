use std::io::Cursor;

use anyhow::{bail, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Audio metadata extracted during decoding
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub sample_rate: u32,
    pub channels: usize,
    pub duration_samples: usize,
}

/// Decode audio from bytes into channel-separated f32 samples.
/// Supports WAV, MP3, FLAC, and AAC via Symphonia.
pub fn decode_audio(data: &[u8], filename: Option<&str>) -> Result<(Vec<Vec<f32>>, AudioMetadata)> {
    let cursor = Cursor::new(data.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    if let Some(name) = filename {
        if let Some(ext) = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
        {
            hint.with_extension(ext);
        }
    }

    let probed = match symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
    {
        Ok(probed) => probed,
        Err(_) if filename.is_some() => {
            // Extension hint may be wrong (e.g. .mp3 file that's actually MP4/AAC).
            // Retry without the hint so Symphonia probes the actual content.
            let cursor = Cursor::new(data.to_vec());
            let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
            symphonia::default::get_probe()
                .format(&Hint::new(), mss, &FormatOptions::default(), &MetadataOptions::default())
                .map_err(|e| anyhow::anyhow!("Failed to probe audio format: {}", e))?
        }
        Err(e) => return Err(anyhow::anyhow!("Failed to probe audio format: {}", e)),
    };

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| anyhow::anyhow!("No audio track found"))?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| anyhow::anyhow!("Failed to create decoder: {}", e))?;

    let mut all_samples: Vec<Vec<f32>> = vec![Vec::new(); channels];

    loop {
        match format.next_packet() {
            Ok(packet) => {
                let decoded = decoder.decode(&packet)?;
                let spec = *decoded.spec();
                let duration = decoded.capacity() as u64;

                let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
                sample_buf.copy_interleaved_ref(decoded);

                let samples = sample_buf.samples();

                for (i, sample) in samples.iter().enumerate() {
                    all_samples[i % channels].push(*sample);
                }
            }
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(anyhow::anyhow!("Decoding error: {}", e)),
        }
    }

    if all_samples.is_empty() || all_samples[0].is_empty() {
        bail!("No audio samples found in file");
    }

    let duration_samples = all_samples[0].len();

    let metadata = AudioMetadata {
        sample_rate,
        channels,
        duration_samples,
    };

    Ok((all_samples, metadata))
}

/// Get the file extension hint for content-type detection
pub fn extension_from_content_type(content_type: &str) -> Option<&'static str> {
    match content_type {
        "audio/wav" | "audio/wave" | "audio/x-wav" => Some("wav"),
        "audio/mpeg" | "audio/mp3" => Some("mp3"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/aac" | "audio/x-aac" | "audio/mp4" | "audio/x-m4a" => Some("aac"),
        _ => None,
    }
}

/// Check if a content type is supported
pub fn is_supported_content_type(content_type: &str) -> bool {
    matches!(
        content_type,
        "audio/wav"
            | "audio/wave"
            | "audio/x-wav"
            | "audio/mpeg"
            | "audio/mp3"
            | "audio/flac"
            | "audio/x-flac"
            | "audio/aac"
            | "audio/x-aac"
            | "audio/mp4"
            | "audio/x-m4a"
    )
}
