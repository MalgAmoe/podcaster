use std::mem::MaybeUninit;

use anyhow::Result;
use mp3lame_encoder::{Builder, DualPcm, FlushNoGap, MonoPcm};

/// Default MP3 bitrate in kbps
pub const DEFAULT_MP3_BITRATE: u32 = 192;

/// Encode channel-separated f32 samples to MP3 bytes.
pub fn encode_mp3(samples: &[Vec<f32>], sample_rate: u32, bitrate_kbps: u32) -> Result<Vec<u8>> {
    let channels = samples.len();
    let num_samples = samples.iter().map(|c| c.len()).min().unwrap_or(0);

    if num_samples == 0 {
        anyhow::bail!("No samples to encode");
    }

    let mut builder = Builder::new().ok_or_else(|| anyhow::anyhow!("Failed to create LAME encoder"))?;

    builder
        .set_sample_rate(sample_rate)
        .map_err(|e| anyhow::anyhow!("Failed to set sample rate: {:?}", e))?;

    builder
        .set_num_channels(channels as u8)
        .map_err(|e| anyhow::anyhow!("Failed to set channels: {:?}", e))?;

    // Set bitrate based on requested value
    let bitrate = match bitrate_kbps {
        128 => mp3lame_encoder::Birtate::Kbps128,
        160 => mp3lame_encoder::Birtate::Kbps160,
        192 => mp3lame_encoder::Birtate::Kbps192,
        224 => mp3lame_encoder::Birtate::Kbps224,
        256 => mp3lame_encoder::Birtate::Kbps256,
        320 => mp3lame_encoder::Birtate::Kbps320,
        _ => mp3lame_encoder::Birtate::Kbps192, // Default to 192
    };

    builder
        .set_brate(bitrate)
        .map_err(|e| anyhow::anyhow!("Failed to set bitrate: {:?}", e))?;

    builder
        .set_quality(mp3lame_encoder::Quality::Best)
        .map_err(|e| anyhow::anyhow!("Failed to set quality: {:?}", e))?;

    let mut encoder = builder
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build encoder: {:?}", e))?;

    // Convert f32 to i16
    let convert = |ch: &[f32]| -> Vec<i16> {
        ch.iter()
            .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
            .collect()
    };

    // Allocate output buffer (worst case: 1.25 * num_samples + 7200)
    let max_output_size = (num_samples * 5 / 4) + 7200;
    let mut output: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); max_output_size];

    // Use MonoPcm for mono, DualPcm for stereo.
    // InterleavedPcm always divides by 2 (assumes stereo), which halves mono audio.
    let encoded_size = if channels == 1 {
        let pcm = convert(&samples[0]);
        encoder
            .encode(MonoPcm(&pcm), &mut output)
            .map_err(|e| anyhow::anyhow!("Encoding error: {:?}", e))?
    } else {
        let left = convert(&samples[0]);
        let right = convert(&samples[1]);
        encoder
            .encode(DualPcm { left: &left, right: &right }, &mut output)
            .map_err(|e| anyhow::anyhow!("Encoding error: {:?}", e))?
    };

    // Flush remaining data
    let flush_size = encoder
        .flush::<FlushNoGap>(&mut output[encoded_size..])
        .map_err(|e| anyhow::anyhow!("Flush error: {:?}", e))?;

    // Convert MaybeUninit<u8> to u8
    let total_size = encoded_size + flush_size;
    let result: Vec<u8> = output
        .into_iter()
        .take(total_size)
        .map(|m| unsafe { m.assume_init() })
        .collect();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mono_mp3_duration() {
        // Create 5 seconds of mono audio at 48kHz
        let sample_rate = 48000u32;
        let duration_secs = 5.0;
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        let mono: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect();

        let mp3 = encode_mp3(&[mono], sample_rate, 192).unwrap();

        // Decode the MP3 back and check duration
        let (decoded, metadata) = crate::audio::decode_audio(&mp3, Some("test.mp3")).unwrap();
        assert_eq!(decoded.len(), 1, "Should decode as mono");

        let decoded_duration = decoded[0].len() as f32 / metadata.sample_rate as f32;
        // MP3 has codec delay, so allow some tolerance, but it must not be half duration
        assert!(
            decoded_duration > duration_secs * 0.9,
            "Mono MP3 duration {:.2}s is too short (expected ~{:.1}s) — likely encoding only half the samples",
            decoded_duration,
            duration_secs
        );
        assert!(
            decoded_duration < duration_secs * 1.1,
            "Mono MP3 duration {:.2}s is too long (expected ~{:.1}s)",
            decoded_duration,
            duration_secs
        );
    }

    #[test]
    fn test_stereo_mp3_duration() {
        let sample_rate = 48000u32;
        let duration_secs = 5.0;
        let num_samples = (sample_rate as f32 * duration_secs) as usize;
        let left: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect();
        let right: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 880.0 * i as f32 / sample_rate as f32).sin() * 0.3)
            .collect();

        let mp3 = encode_mp3(&[left, right], sample_rate, 192).unwrap();

        let (decoded, metadata) = crate::audio::decode_audio(&mp3, Some("test.mp3")).unwrap();
        assert_eq!(decoded.len(), 2, "Should decode as stereo");

        let decoded_duration = decoded[0].len() as f32 / metadata.sample_rate as f32;
        assert!(
            decoded_duration > duration_secs * 0.9,
            "Stereo MP3 duration {:.2}s is too short (expected ~{:.1}s)",
            decoded_duration,
            duration_secs
        );
        assert!(
            decoded_duration < duration_secs * 1.1,
            "Stereo MP3 duration {:.2}s is too long (expected ~{:.1}s)",
            decoded_duration,
            duration_secs
        );
    }
}
