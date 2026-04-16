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

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse WAV header and return (channels, sample_rate, data_size_bytes)
    fn parse_wav_header(data: &[u8]) -> (u16, u32, u32) {
        let channels = u16::from_le_bytes([data[22], data[23]]);
        let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let data_size = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        (channels, sample_rate, data_size)
    }

    #[test]
    fn test_mono_wav_roundtrip() {
        // Create 1 second of mono audio at 48kHz
        let sample_rate = 48000u32;
        let num_samples = sample_rate as usize;
        let mono: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect();
        let samples = vec![mono];

        let wav = encode_wav(&samples, sample_rate).unwrap();
        let (channels, rate, data_size) = parse_wav_header(&wav);

        assert_eq!(channels, 1, "WAV header should say 1 channel");
        assert_eq!(rate, sample_rate, "WAV header sample rate mismatch");
        // data_size = num_samples * channels * bytes_per_sample
        let expected_data = (num_samples * 1 * 2) as u32;
        assert_eq!(data_size, expected_data, "WAV data size mismatch");

        // Total file = 44 header + data
        assert_eq!(wav.len(), 44 + expected_data as usize);

        // Now decode it back and verify
        let (decoded, metadata) = crate::audio::decode_audio(&wav, Some("test.wav")).unwrap();
        assert_eq!(metadata.channels, 1, "Decoded should be 1 channel");
        assert_eq!(metadata.sample_rate, sample_rate);
        assert_eq!(decoded.len(), 1, "Should have 1 channel vec");
        assert_eq!(decoded[0].len(), num_samples, "Should have same number of samples");
    }

    #[test]
    fn test_mono_full_pipeline() {
        // Simulate the full API path: decode → process → encode → verify
        let sample_rate = 48000u32;
        let duration_secs = 5;
        let num_samples = sample_rate as usize * duration_secs;

        // Create mono signal with some content
        let mono: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.3
                    + (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.1
            })
            .collect();

        // Encode to WAV (simulating an upload)
        let input_wav = encode_wav(&[mono], sample_rate).unwrap();
        let (in_ch, in_rate, _) = parse_wav_header(&input_wav);
        assert_eq!(in_ch, 1);
        assert_eq!(in_rate, sample_rate);

        // Decode (as the API would)
        let (mut samples, metadata) = crate::audio::decode_audio(&input_wav, Some("test.wav")).unwrap();
        assert_eq!(samples.len(), 1, "Should decode as mono");
        assert_eq!(metadata.channels, 1);
        let decoded_len = samples[0].len();

        // Process through engine
        let config = crate::models::ProcessConfig::new();
        crate::processing::process_audio(&mut samples, metadata.sample_rate, &config, None).unwrap();

        assert_eq!(samples.len(), 1, "Processing should not add channels");
        assert_eq!(samples[0].len(), decoded_len, "Processing should not change sample count");

        // Encode output
        let output_wav = encode_wav(&samples, metadata.sample_rate).unwrap();
        let (out_ch, out_rate, out_data) = parse_wav_header(&output_wav);

        assert_eq!(out_ch, 1, "Output WAV must be mono");
        assert_eq!(out_rate, sample_rate, "Output sample rate must match input");
        let expected_data = (decoded_len * 1 * 2) as u32;
        assert_eq!(out_data, expected_data, "Output data size mismatch — would cause speed issue");

        // Decode the output and verify
        let (final_decoded, final_meta) = crate::audio::decode_audio(&output_wav, Some("out.wav")).unwrap();
        assert_eq!(final_meta.channels, 1);
        assert_eq!(final_decoded.len(), 1);
        assert_eq!(final_decoded[0].len(), decoded_len, "Round-trip sample count must match");
    }

    #[test]
    fn test_stereo_wav_roundtrip() {
        let sample_rate = 48000u32;
        let num_samples = sample_rate as usize;
        let left: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5)
            .collect();
        let right: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * std::f32::consts::PI * 880.0 * i as f32 / sample_rate as f32).sin() * 0.3)
            .collect();
        let samples = vec![left, right];

        let wav = encode_wav(&samples, sample_rate).unwrap();
        let (channels, rate, data_size) = parse_wav_header(&wav);

        assert_eq!(channels, 2, "WAV header should say 2 channels");
        assert_eq!(rate, sample_rate);
        let expected_data = (num_samples * 2 * 2) as u32;
        assert_eq!(data_size, expected_data);

        let (decoded, metadata) = crate::audio::decode_audio(&wav, Some("test.wav")).unwrap();
        assert_eq!(metadata.channels, 2);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].len(), num_samples);
        assert_eq!(decoded[1].len(), num_samples);
    }
}
