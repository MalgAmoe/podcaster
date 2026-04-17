# Audio Processing Pipeline

## Overview

The processing pipeline is configured via **strength** (1-3) and an optional **AI Clean** toggle. The strength maps to processor settings in `ProcessConfig::from_strength()`.

## Web UI Configuration

- **Strength**: 1-3 (Subtle, Balanced, Intense)
- **AI Clean**: Optional toggle - isolates voice using MossFormer2 speech enhancement

### API

```json
POST /api/jobs
{
  "s3_key": "inputs/123/audio.mp3",
  "filename": "episode.mp3",
  "strength": 2
}
```

---

## Processing Order

The processing order is **fixed and immutable**. Strength controls which processors are enabled and their intensity.

```
1. INPUT STAGE
   - Filters (HP 80Hz + LP 15.5kHz)
   - Input gain normalization (-18 LUFS target)
   - Declick (offline only)

2. SPECTRAL (STFT-based, mono processing)
   - DeReverb (optional)
   - Denoiser (always on)
   - AI Denoise (optional, MossFormer2)
   - Spectral Gate (optional)

3. DYNAMICS
   - Peak Compressor (look-ahead VCA)
   - FET Compressor (feedback 1176-style)

4. EQ & TONE
   - FixEQ - mud removal + correction (optional)
   - De-Esser (sibilance reduction)
   - Saturation / Channel9 (optional)
   - ButterComp (optional)

5. ENHANCEMENT
   - EnhanceEQ OR RadioVoice (mutually exclusive)
   - TapeGlue (optional)

6. OUTPUT STAGE
   - LUFS normalization (-16 LUFS target)
   - Limiter - true peak -1dB
```

## Why This Order?

| Stage | Reason |
|-------|--------|
| Filters first | Remove rumble/hiss before analysis |
| Denoiser before gate | Gate needs clean signal to detect speech vs noise |
| Compressor after gate | Compressor works on clean signal |
| FixEQ before de-esser | Remove mud first, then target remaining sibilance |
| Enhancement last | Boost presence after all corrective processing |
| Limiter always last | Catch any peaks from cumulative gain |

## Strength Levels

| Level | Name | Description |
|-------|------|-------------|
| 1 | Subtle | Light touch, preserves dynamics |
| 2 | Balanced | Good for most recordings (default) |
| 3 | Intense | Heavy processing, broadcast-ready |

## Processor Categories

### STFT-Based (Spectral)
- Denoiser, DeReverb, Spectral Gate, Peak Attenuator
- Process L/R channels independently (no stereo linking)

### Sample-Based (Time Domain)
- Compressors, EQ, Saturation, Limiter
- Can have stereo linking (shared detection)
- Preserves stereo image better

## Best Practices

1. **Start with Balanced** (strength 2) and adjust
2. **Don't over-process** - if source is clean, use Subtle
3. **Enable AI Clean** for noisy recordings with non-voice sounds
4. **A/B compare** - toggle original vs processed frequently
