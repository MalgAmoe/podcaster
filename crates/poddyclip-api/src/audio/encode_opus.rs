use anyhow::{Context, Result};
use ogg::writing::PacketWriter;
use opus::{Application, Channels, Encoder};
use std::io::Cursor;

/// Opus encoding bitrate (128 kbps for good quality A/B comparison)
pub const OPUS_BITRATE: i32 = 128_000;

/// Opus frame size in samples (20ms at 48kHz = 960 samples)
const FRAME_SIZE: usize = 960;

/// Encode channel-separated f32 samples to Opus/Ogg bytes.
/// Resamples to 48kHz if needed (Opus native rate).
pub fn encode_opus(samples: &[Vec<f32>], sample_rate: u32) -> Result<Vec<u8>> {
    let channels = samples.len();
    if channels == 0 || channels > 2 {
        anyhow::bail!("Opus encoding supports 1 or 2 channels, got {}", channels);
    }

    // Resample to 48kHz if needed (Opus native rate)
    let (resampled, opus_rate) = if sample_rate != 48000 {
        let resampled = resample_to_48k(samples, sample_rate);
        (resampled, 48000)
    } else {
        (samples.to_vec(), sample_rate)
    };

    let opus_channels = if channels == 1 {
        Channels::Mono
    } else {
        Channels::Stereo
    };

    let mut encoder =
        Encoder::new(opus_rate, opus_channels, Application::Audio).context("Failed to create Opus encoder")?;

    encoder.set_bitrate(opus::Bitrate::Bits(OPUS_BITRATE)).ok();

    // Get encoder lookahead for pre-skip
    let lookahead = encoder.get_lookahead().unwrap_or(312) as u16;

    // Prepare Ogg container
    let mut ogg_buffer = Cursor::new(Vec::new());
    let mut packet_writer = PacketWriter::new(&mut ogg_buffer);

    // Write Opus header packet
    let opus_head = create_opus_head(channels as u8, opus_rate, lookahead);
    packet_writer
        .write_packet(opus_head, 0, ogg::writing::PacketWriteEndInfo::EndPage, 0)
        .context("Failed to write Opus header")?;

    // Write Opus comment packet
    let opus_tags = create_opus_tags();
    packet_writer
        .write_packet(opus_tags, 0, ogg::writing::PacketWriteEndInfo::EndPage, 0)
        .context("Failed to write Opus tags")?;

    // Interleave samples for encoding
    let num_samples = resampled.iter().map(|c| c.len()).min().unwrap_or(0);
    let mut interleaved = Vec::with_capacity(num_samples * channels);
    for i in 0..num_samples {
        for ch in &resampled {
            interleaved.push(ch[i]);
        }
    }

    // Encode in frames
    let mut granule_pos: u64 = 0;
    let mut encoded_packet = vec![0u8; 4000]; // Max Opus packet size

    for frame_start in (0..interleaved.len()).step_by(FRAME_SIZE * channels) {
        let frame_end = (frame_start + FRAME_SIZE * channels).min(interleaved.len());
        let frame = &interleaved[frame_start..frame_end];

        // Pad last frame if needed
        let padded_frame: Vec<f32>;
        let encode_frame = if frame.len() < FRAME_SIZE * channels {
            padded_frame = {
                let mut f = frame.to_vec();
                f.resize(FRAME_SIZE * channels, 0.0);
                f
            };
            &padded_frame
        } else {
            frame
        };

        let encoded_len = encoder
            .encode_float(encode_frame, &mut encoded_packet)
            .context("Failed to encode Opus frame")?;

        granule_pos += FRAME_SIZE as u64;

        let is_last = frame_end >= interleaved.len();
        let end_info = if is_last {
            ogg::writing::PacketWriteEndInfo::EndStream
        } else {
            ogg::writing::PacketWriteEndInfo::NormalPacket
        };

        packet_writer
            .write_packet(
                encoded_packet[..encoded_len].to_vec(),
                0,
                end_info,
                granule_pos,
            )
            .context("Failed to write Opus packet")?;
    }

    Ok(ogg_buffer.into_inner())
}

/// Create Opus identification header (OpusHead)
fn create_opus_head(channels: u8, sample_rate: u32, pre_skip: u16) -> Vec<u8> {
    let mut head = Vec::with_capacity(19);
    head.extend_from_slice(b"OpusHead"); // Magic signature
    head.push(1); // Version
    head.push(channels); // Channel count
    head.extend_from_slice(&pre_skip.to_le_bytes()); // Pre-skip (encoder lookahead)
    head.extend_from_slice(&sample_rate.to_le_bytes()); // Input sample rate (informational)
    head.extend_from_slice(&0i16.to_le_bytes()); // Output gain
    head.push(0); // Channel mapping family (0 = mono/stereo)
    head
}

/// Create Opus comment header (OpusTags)
fn create_opus_tags() -> Vec<u8> {
    let vendor = "poddyclip";
    let mut tags = Vec::new();
    tags.extend_from_slice(b"OpusTags"); // Magic signature
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes()); // Vendor string length
    tags.extend_from_slice(vendor.as_bytes()); // Vendor string
    tags.extend_from_slice(&0u32.to_le_bytes()); // User comment list length (0 comments)
    tags
}

/// Simple linear resampling to 48kHz
fn resample_to_48k(samples: &[Vec<f32>], from_rate: u32) -> Vec<Vec<f32>> {
    let ratio = 48000.0 / from_rate as f64;

    samples
        .iter()
        .map(|channel| {
            let new_len = (channel.len() as f64 * ratio).ceil() as usize;
            let mut resampled = Vec::with_capacity(new_len);

            for i in 0..new_len {
                let src_pos = i as f64 / ratio;
                let src_idx = src_pos.floor() as usize;
                let frac = (src_pos - src_idx as f64) as f32;

                let sample = if src_idx + 1 < channel.len() {
                    channel[src_idx] * (1.0 - frac) + channel[src_idx + 1] * frac
                } else if src_idx < channel.len() {
                    channel[src_idx]
                } else {
                    0.0
                };

                resampled.push(sample);
            }

            resampled
        })
        .collect()
}
