# Poddyclip

Podcast audio enhancement with a Rust DSP engine, a production Rust API, and a Phoenix + SolidJS web app.

Only pay for what you eat.

## Overview

Poddyclip is a Rust workspace with three components:

- **poddyclip** - Core audio processing library
- **poddyclip-cli** - Internal command-line tool for testing ideas, checking settings on real files, and benchmarking processors
- **poddyclip-api** - HTTP service used by the Phoenix app for production processing

## Installation

```bash
# Build the internal CLI tool
cargo build --release -p poddyclip-cli
```

## CLI Usage

The CLI is primarily for internal testing and benchmarking.

**Supported input formats:** WAV, MP3, FLAC (via symphonia)
**Output format:** WAV

```bash
# Basic processing
poddyclip input.wav -o output.wav

# Enable the spectral denoiser with a stronger preset
poddyclip input.wav --denoise --preset 3 -o output.wav

# AI cleaning with MossFormer2
poddyclip input.wav --ai-denoise -o output.wav

# FET compression (1176-style) instead of peak compression
poddyclip input.wav --fet -o output.wav

# Remove tonal noise (hum, whine)
poddyclip input.wav --depeak -o output.wav
```

### Key Options

| Option | Description |
|--------|-------------|
| `--denoise` | Enable the normal spectral denoiser |
| `--preset 1-3` | Spectral denoiser strength |
| `--ai-denoise` | Run MossFormer2 voice cleaning |
| `--dereverb 1-3` | De-reverb strength (0 = off) |
| `--spectral-gate 1-3` | Non-stationary noise gate (0 = off) |
| `--depeak` | Remove tonal peaks (hum, whine) |
| `--fet` | Use FET compressor instead of peak compressor |
| `--radio` | Broadcast-style automatic EQ |
| `--declick` | Remove clicks and pops (offline) |

### Disable Flags

Skip individual processors: `--disable-expander`, `--disable-comp`, `--disable-fixeq`, `--disable-deesser`, `--disable-saturation`, `--disable-buttercomp`, `--disable-enhanceeq`, `--disable-tape`, `--disable-limiter`

### Per-Processor Presets

Fine-tune individual stages: `--expander-preset`, `--peakcomp-preset`, `--fetcomp-preset`, `--saturation-preset`, `--eq-preset`, `--buttercomp-preset`

## Processing Chain

1. **Filters** - HP 80Hz (12/24dB slope), LP 15.5kHz
2. **Input Gain** - Normalize to -18dB RMS
3. **Declick** - Click/pop removal (optional, offline)
4. **De-reverb** - Spectral reverb reduction (optional)
5. **Denoise** - Spectral subtraction with minimum statistics noise floor (optional)
6. **AI Clean** - MossFormer2 voice cleaning at a 48k internal model rate (optional)
7. **Spectral Gate** - Non-stationary noise handling (optional)
8. **Peak Attenuator** - Tonal noise removal (optional)
9. **Expander** - Downward expansion for quiet passages
10. **Compressor** - Peak VCA (lookahead) or FET (1176-style)
11. **FixEQ** - Dynamic 3-band correction
12. **De-esser** - Sibilance reduction
13. **Channel9** - Neve-style transformer saturation
14. **ButterComp** - Smooth bipolar compression
15. **EnhanceEQ** - Presence and air boost
16. **TapeGlue** - Tape hysteresis saturation
17. **LUFS Normalization** - Target loudness
18. **Limiter** - True peak limiting (-1dBTP)

## Technical Details

### Spectral Processing

- **STFT Infrastructure** - Centralized FFT with sqrt-Hann windowing
  - Real-time: 2048 window / 1024 hop
  - Analysis: 4096 window / 2048 hop
- **Noise Floor Estimation** - Minimum statistics tracking
- **Spectral Flatness Measure** - Speech vs. noise classification

### Dynamics

- **StereoVcaPeakComp** - Lookahead VCA with histogram-based threshold detection
- **StereoFetCompressor** - 1176-style feedback topology
  - 2x oversampling to reduce aliasing
  - Newton-Raphson solver for artifact-free gain computation
  - Program-dependent release
- **ButterComp2** - Airwindows bipolar algorithm with 4 interleaved control paths

### Saturation

- **TapeGlue** - Jiles-Atherton magnetic hysteresis model
  - Physics-based tape emulation (DAFx 2019 research)
  - f64 internal precision
- **Channel9** - Neve transformer emulation (Airwindows port)
  - Golden ratio filter Q values
  - Multi-stage: biquad, IIR highpass, spiral saturation, slew limiting

### Analysis

- **LUFS Metering** - ITU-R BS.1770 integrated loudness
- **Cepstral Analysis** - F0 detection, voiced ratio, spectral envelope
- **Reverb Analysis** - RT60 per-band, Direct-to-Reverb Ratio (DRR)

### Stereo Handling

- **STFT processors** (denoiser, dereverb, spectral gate) - Independent L/R processing
- **Time-domain processors** - Linked stereo detection, same gain applied to both channels

## Project Structure

```
crates/poddyclip/src/
├── ai_clean/      # MossFormer2 model adapter
├── analysis/      # LUFS, spectral, cepstral, reverb analysis
├── denoiser/      # Spectral subtraction, gates, peak attenuation
├── dereverb/      # Spectral de-reverb
├── dynamics/      # Compressors, expander, limiter, autogain
├── eq/            # Filters, deesser, enhancement, radio EQ
├── repair/        # Declicker
├── sample_rate.rs # Shared resampling helper
├── saturation/    # TapeGlue, Channel9
├── stft/          # FFT infrastructure
└── traits.rs      # AudioProcessor, StereoProcessor traits

crates/poddyclip-cli/    # Internal command-line tool
crates/poddyclip-api/    # HTTP processing service
```

## Preset Levels

| Level | Name | Description |
|-------|------|-------------|
| 1 | Gentle | Minimal processing |
| 2 | Light | Subtle effect |
| 3 | Moderate | Balanced (default) |

## License

All rights reserved.
