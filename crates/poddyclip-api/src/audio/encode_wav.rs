use anyhow::Result;

/// Encode channel-separated f32 samples to WAV bytes (16-bit PCM).
/// We manually construct the WAV since hound doesn't expose the inner buffer easily.
pub fn encode_wav(samples: &[Vec<f32>], sample_rate: u32) -> Result<Vec<u8>> {
    let channels = samples.len() as u16;
    let num_samples = samples.iter().map(|c| c.len()).min().unwrap_or(0);
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = bits_per_sample / 8;
    let block_align = channels * bytes_per_sample;
    let byte_rate = sample_rate * block_align as u32;
    let data_size = (num_samples * channels as usize * bytes_per_sample as usize) as u32;
    let file_size = 36 + data_size; // RIFF header (8) + fmt chunk (24) + data header (8) + data

    let mut buffer = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    buffer.extend_from_slice(b"RIFF");
    buffer.extend_from_slice(&file_size.to_le_bytes());
    buffer.extend_from_slice(b"WAVE");

    // fmt subchunk
    buffer.extend_from_slice(b"fmt ");
    buffer.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size for PCM
    buffer.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat: 1 = PCM
    buffer.extend_from_slice(&channels.to_le_bytes());
    buffer.extend_from_slice(&sample_rate.to_le_bytes());
    buffer.extend_from_slice(&byte_rate.to_le_bytes());
    buffer.extend_from_slice(&block_align.to_le_bytes());
    buffer.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data subchunk
    buffer.extend_from_slice(b"data");
    buffer.extend_from_slice(&data_size.to_le_bytes());

    // Write interleaved samples
    for i in 0..num_samples {
        for channel in samples {
            let sample = channel[i].clamp(-1.0, 1.0);
            let sample_i16 = (sample * 32767.0) as i16;
            buffer.extend_from_slice(&sample_i16.to_le_bytes());
        }
    }

    Ok(buffer)
}
