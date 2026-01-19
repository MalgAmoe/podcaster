# Audio Processing Chains

## Overview

Chains are TOML configuration files that define which processors to apply and at what intensity. They live in the `chains/` directory.

## Quick Answer: Can I Add Processors in Any Order?

**No.** The processing order is **fixed and immutable**. Chains only control which processors are enabled and their intensity (preset 1-5). The engine always applies processors in the same sequence regardless of the order in your TOML file.

This is intentional - audio processing order matters significantly for quality.

## Fixed Processing Order

```
┌─────────────────────────────────────────────────────────────┐
│  1. INPUT STAGE                                             │
│     └─ Filters (HP 80Hz + LP 15.5kHz)                       │
│     └─ Input gain normalization (-18 LUFS target)           │
│     └─ Declick (offline only)                               │
├─────────────────────────────────────────────────────────────┤
│  2. SPECTRAL (STFT-based, mono processing)                  │
│     └─ DeReverb (optional)                                  │
│     └─ Denoiser (always on)                                 │
│     └─ Spectral Gate (optional)                             │
│     └─ Peak Attenuator (optional)                           │
├─────────────────────────────────────────────────────────────┤
│  3. DYNAMICS                                                │
│     └─ Expander / noise gate (optional)                     │
│     └─ Compressor - Peak OR FET (optional)                  │
├─────────────────────────────────────────────────────────────┤
│  4. EQ & TONE                                               │
│     └─ FixEQ - mud removal + correction (optional)          │
│     └─ De-Esser (optional)                                  │
│     └─ Saturation / Channel9 (optional)                     │
│     └─ ButterComp (optional)                                │
├─────────────────────────────────────────────────────────────┤
│  5. ENHANCEMENT                                             │
│     └─ EnhanceEQ OR RadioVoice (mutually exclusive)         │
│     └─ TapeGlue (optional)                                  │
├─────────────────────────────────────────────────────────────┤
│  6. OUTPUT STAGE                                            │
│     └─ LUFS normalization (optional)                        │
│     └─ Limiter - true peak -1dB (paired with LUFS)          │
└─────────────────────────────────────────────────────────────┘
```

## Why This Order?

| Stage | Reason |
|-------|--------|
| Filters first | Remove rumble/hiss before analysis |
| Denoiser before gate | Gate needs clean signal to detect speech vs noise |
| Expander before compressor | Gate reduces noise, then compressor works on clean signal |
| FixEQ before de-esser | Remove mud first, then target remaining sibilance |
| Enhancement last | Boost presence after all corrective processing |
| Limiter always last | Catch any peaks from cumulative gain |

## Chain File Format

```toml
name = "My Chain"
description = "Optional description"

# Spectral processors (1-5 intensity, false to disable)
denoiser = 3        # Always enabled, controls intensity
dereverb = 2        # Optional reverb removal
spectral_gate = 0   # 0 or false = disabled
depeak = false      # Tonal noise removal

# Dynamics
expander = 2                              # Noise gate
compressor = { type = "peak", preset = 3 } # or type = "fet"

# EQ & Tone
fixeq = true        # true = default preset
deesser = true
saturation = 2      # Channel9 warmth
buttercomp = 3      # Airwindows compression

# Enhancement
enhanceeq = 3       # Presence/air boost
tape = 2            # TapeGlue saturation
radio = false       # true = use RadioVoice instead of EnhanceEQ

# Output
output = -16        # LUFS target (-14, -16, -18, -24)
                    # false = disable normalization + limiter
```

## Preset Levels

All preset-based processors use 1-5 scale:

| Level | Name | Use Case |
|-------|------|----------|
| 1 | Gentle | Clean recordings, minimal intervention |
| 2 | Light | Good recordings with minor issues |
| 3 | Moderate | Average recordings (default) |
| 4 | Strong | Problematic recordings |
| 5 | Aggressive | Very noisy/problematic recordings |

## Built-in Chains

### gentle.toml
```toml
name = "Gentle"
description = "Minimal processing, preserve dynamics"

denoiser = 1
expander = 1
compressor = { type = "peak", preset = 1 }
fixeq = true
deesser = true
output = -18
```

### podcast.toml
```toml
name = "Podcast"
description = "Balanced processing for conversational podcasts"

denoiser = 3
expander = 2
compressor = { type = "peak", preset = 3 }
fixeq = true
deesser = true
saturation = 2
buttercomp = 3
enhanceeq = 3
tape = 2
output = -16
```

### broadcast.toml
```toml
name = "Broadcast"
description = "Radio-ready, polished sound"

denoiser = 4
expander = 4
compressor = { type = "fet", preset = 4 }
fixeq = true
deesser = true
saturation = 3
buttercomp = 4
enhanceeq = 5
tape = 3
output = -14
```

## Creating Custom Chains

1. Create a new `.toml` file in `chains/`
2. Set `name` (required) and `description` (optional)
3. Configure processors - omit or set `false` to disable
4. Set output LUFS target

**Minimal chain (just denoising):**
```toml
name = "Denoise Only"
denoiser = 3
output = false  # No normalization
```

**Full custom chain:**
```toml
name = "Interview Cleanup"
description = "For noisy interview recordings"

# Heavy noise reduction
denoiser = 4
dereverb = 2
spectral_gate = 3

# Gentle dynamics
expander = 2
compressor = { type = "peak", preset = 2 }

# Standard EQ
fixeq = true
deesser = true

# Light enhancement
enhanceeq = 2

# Podcast-standard output
output = -16
```

## CLI Usage

```bash
# Use a chain
poddyclip input.wav --chain podcast

# Override chain settings
poddyclip input.wav --chain podcast --preset 5  # Override denoiser to level 5

# List available chains
poddyclip --list-chains

# Process without chain (uses defaults)
poddyclip input.wav --preset 3
```

## Mutually Exclusive Options

| Option A | Option B | Notes |
|----------|----------|-------|
| EnhanceEQ | RadioVoice | Set `radio = true` for RadioVoice |
| Peak compressor | FET compressor | Set via `compressor.type` |

## Processor Categories

### STFT-Based (Spectral)
- Denoiser, DeReverb, Spectral Gate, Peak Attenuator
- Process L/R channels independently (no stereo linking)
- May cause minor stereo artifacts on very different channels

### Sample-Based (Time Domain)
- Expander, Compressors, EQ, Saturation, Limiter
- Can have stereo linking (shared detection)
- Preserves stereo image better

## Best Practices

1. **Start with a built-in chain** and adjust
2. **Don't over-process** - if source is clean, use gentle/light settings
3. **Match output LUFS to platform** (-14 YouTube, -16 podcasts, -24 film)
4. **Test with headphones** - artifacts more audible
5. **A/B compare** - toggle original vs processed frequently
