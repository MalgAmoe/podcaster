use std::mem::MaybeUninit;

use anyhow::Result;
use mp3lame_encoder::{Builder, FlushNoGap, InterleavedPcm};

/// Default MP3 bitrate in kbps
pub const DEFAULT_MP3_BITRATE: u32 = 192;

/// Encode channel-separated f32 samples to MP3 bytes.
pub fn encode_mp3(samples: &[Vec<f32>], sample_rate: u32, bitrate_kbps: u32) -> Result<Vec<u8>> {
    let channels = samples.len();
    let num_samples = samples.iter().map(|c| c.len()).min().unwrap_or(0);

    if num_samples == 0 {
        anyhow::bail!("No samples to encode");
    }

    // Convert f32 to i16 and interleave
    let mut interleaved: Vec<i16> = Vec::with_capacity(num_samples * channels);
    for i in 0..num_samples {
        for channel in samples {
            let sample = channel[i].clamp(-1.0, 1.0);
            let sample_i16 = (sample * 32767.0) as i16;
            interleaved.push(sample_i16);
        }
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

    // Encode in chunks
    let input = InterleavedPcm(&interleaved);

    // Allocate output buffer (worst case: 1.25 * input + 7200)
    let max_output_size = (interleaved.len() * 5 / 4) + 7200;
    let mut output: Vec<MaybeUninit<u8>> = vec![MaybeUninit::uninit(); max_output_size];

    let encoded_size = encoder
        .encode(input, &mut output)
        .map_err(|e| anyhow::anyhow!("Encoding error: {:?}", e))?;

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
