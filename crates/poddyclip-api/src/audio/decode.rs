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

    let probed = match symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) {
        Ok(probed) => probed,
        Err(_) if filename.is_some() => {
            // Extension hint may be wrong (e.g. .mp3 file that's actually MP4/AAC).
            // Retry without the hint so Symphonia probes the actual content.
            let cursor = Cursor::new(data.to_vec());
            let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
            symphonia::default::get_probe()
                .format(
                    &Hint::new(),
                    mss,
                    &FormatOptions::default(),
                    &MetadataOptions::default(),
                )
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
    let mut consecutive_errors: usize = 0;
    let mut total_errors: usize = 0;
    let max_consecutive_errors: usize = 50;

    loop {
        match format.next_packet() {
            Ok(packet) => match decoder.decode(&packet) {
                Ok(decoded) => {
                    consecutive_errors = 0;

                    let spec = *decoded.spec();
                    let duration = decoded.capacity() as u64;

                    let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
                    sample_buf.copy_interleaved_ref(decoded);

                    let samples = sample_buf.samples();

                    for (i, sample) in samples.iter().enumerate() {
                        all_samples[i % channels].push(*sample);
                    }
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => {
                    consecutive_errors += 1;
                    total_errors += 1;
                    if consecutive_errors >= max_consecutive_errors {
                        bail!("File appears to be corrupt (too many consecutive decode errors)");
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Decoding error: {}", e)),
            },
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => return Err(anyhow::anyhow!("Decoding error: {}", e)),
        }
    }

    if total_errors > 0 {
        tracing::warn!(
            skipped = total_errors,
            "Skipped malformed audio frames during decode"
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mp3_path() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sounds/short.mp3")
    }

    #[test]
    fn test_decode_valid_mp3() {
        let data = std::fs::read(test_mp3_path()).expect("short.mp3 not found in sounds/");
        let (samples, metadata) = decode_audio(&data, Some("short.mp3")).unwrap();

        assert_eq!(metadata.sample_rate, 48000);
        assert_eq!(metadata.channels, 2);
        assert!(metadata.duration_samples > 0);
        assert_eq!(samples.len(), 2);
        assert!(
            samples[0].len() > 1000,
            "expected many samples, got {}",
            samples[0].len()
        );
    }

    #[test]
    fn test_decode_mp3_with_corrupted_frames_in_middle() {
        let mut data = std::fs::read(test_mp3_path()).expect("short.mp3 not found in sounds/");

        // Corrupt some bytes in the middle of the file to simulate bad frames
        let mid = data.len() / 2;
        for i in mid..mid + 200 {
            if i < data.len() {
                data[i] = 0xFF ^ data[i];
            }
        }

        // Should still decode successfully, skipping the bad frames
        let result = decode_audio(&data, Some("short.mp3"));
        assert!(
            result.is_ok(),
            "decode should succeed with a few corrupt frames: {:?}",
            result.err()
        );

        let (samples, metadata) = result.unwrap();
        assert!(
            samples[0].len() > 1000,
            "should have decoded most of the file"
        );
        assert_eq!(metadata.channels, 2);
    }

    #[test]
    fn test_decode_mp3_with_corrupted_start() {
        let mut data = std::fs::read(test_mp3_path()).expect("short.mp3 not found in sounds/");

        // Corrupt the first ~500 bytes after any ID3 tag
        // MP3 frames typically start after ID3v2 tags
        let start = if data.len() > 600 { 100 } else { 0 };
        for i in start..start + 500 {
            if i < data.len() {
                data[i] = 0x00;
            }
        }

        // Should still decode, skipping bad frames at the start
        let result = decode_audio(&data, Some("short.mp3"));
        assert!(
            result.is_ok(),
            "decode should handle corrupt frames at start: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_decode_totally_corrupt_data() {
        // Random garbage that isn't audio at all
        let data: Vec<u8> = (0..10000).map(|i| (i * 7 % 256) as u8).collect();

        let result = decode_audio(&data, Some("garbage.mp3"));
        assert!(result.is_err(), "should fail on completely garbage data");
    }

    #[test]
    fn test_decode_empty_data() {
        let result = decode_audio(&[], Some("empty.mp3"));
        assert!(result.is_err(), "should fail on empty data");
    }

    #[test]
    fn test_decode_wav() {
        // Check if we have a wav file to test
        let wav_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sounds/kebab1.wav");

        if wav_path.exists() {
            let data = std::fs::read(&wav_path).unwrap();
            let (samples, metadata) = decode_audio(&data, Some("kebab1.wav")).unwrap();

            assert!(metadata.sample_rate > 0);
            assert!(metadata.channels > 0);
            assert!(samples[0].len() > 0);
        }
    }

    #[test]
    fn test_decode_wrong_extension_hint() {
        // Give a .wav extension hint for an MP3 file
        // Should still work because of the probe retry without hint
        let data = std::fs::read(test_mp3_path()).expect("short.mp3 not found in sounds/");
        let result = decode_audio(&data, Some("wrong.wav"));
        assert!(
            result.is_ok(),
            "should handle wrong extension via probe retry: {:?}",
            result.err()
        );
    }
}
