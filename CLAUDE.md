# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build CLI (release)
cargo build --release -p poddyclip-cli

# Build plugin (VST3/CLAP)
cargo xtask bundle poddyclip-plugin --release

# Run tests
cargo test

# Run single test
cargo test test_name

# Run CLI
./target/release/poddyclip input.wav --preset 3
```

## Project Structure

Rust workspace with three crates:

- **`crates/poddyclip`** - Core audio processing library (no audio I/O)
- **`crates/poddyclip-cli`** - Command-line tool using symphonia for input, hound for WAV output
- **`crates/poddyclip-plugin`** - VST3/CLAP plugin using nih-plug framework with egui GUI

## STFT Infrastructure

All FFT-based processors share the `stft/` module:

```
crates/poddyclip/src/stft/
├── mod.rs          # Re-exports
├── types.rs        # Constants: RT_WINDOW_SIZE=2048, RT_HOP_SIZE=1024, EPSILON=1e-10
├── window.rs       # hann_window(), sqrt_hann_window(), create_window()
└── processor.rs    # StftProcessor struct
```

**Key APIs:**
- `StftProcessor::new_realtime(sample_rate)` - 2048/1024 for real-time processing
- `StftProcessor::new_analysis(sample_rate)` - 4096/2048 for offline analysis
- `forward_fft(frame) -> Vec<Complex>` - Apply window and FFT
- `inverse_fft(spectrum) -> Vec<f32>` - IFFT and apply synthesis window
- `overlap_add(synthesized) -> &[f32]` - Get output hop samples
- `compute_power(spectrum) -> Vec<f32>` - Get power spectrum
- `ensure_symmetry(spectrum)` - For real-valued output

**Pattern:** All STFT processors (denoiser, dereverb, spectral gate, peak attenuator) use StftProcessor internally. Uses sqrt-Hann windows for overlap-add.

## Two Processor Categories

There are two fundamentally different processor types with different APIs:

### 1. STFT-Based Processors (FFT)

**Examples:** Denoiser, SpectralGate, DeReverb, PeakAttenuator

**Characteristics:**
- Returns new Vec: `fn process(&mut self, audio: &[f32]) -> Vec<f32>`
- NOT in-place processing (needs overlap-add buffer)
- Mono only - NO stereo linking possible
- Processes entire buffer at once (batch mode)

**API Pattern:**
```rust
pub struct MyStftProcessor {
    stft: StftProcessor,
    // ... state
}

impl MyStftProcessor {
    pub fn new(sample_rate: u32) -> Self;
    pub fn new_with_preset(sample_rate: u32, level: u8) -> Option<Self>;
    pub fn process(&mut self, audio: &[f32]) -> Vec<f32>;  // Returns NEW buffer
    pub fn reset(&mut self);
    pub fn get_max_gain_reduction_db(&self) -> f32;  // For diagnostics
}
```

**CLI Stereo Handling (separate instances):**
```rust
if is_stereo {
    let mut left_proc = Processor::new(sample_rate);
    let mut right_proc = Processor::new(sample_rate);
    samples[0] = left_proc.process(&samples[0]);
    samples[1] = right_proc.process(&samples[1]);
} else {
    samples[0] = proc.process(&samples[0]);
}
```

**Note:** STFT processors process channels independently. This can cause stereo artifacts if detection differs between channels. For stereo-critical processing, use sample-based processors with linking.

### 2. Sample-Based Processors (Time Domain)

**Examples:** Compressors, EQs, Filters, Limiters, Saturators

**Characteristics:**
- In-place: `fn process_buffer(&mut self, buffer: &mut [f32])`
- Can implement `StereoProcessor` for linked stereo operation
- Sample-by-sample or buffer-based

**Traits:**

```rust
// AudioProcessor - mono in-place processing
trait AudioProcessor: Send {
    fn process_buffer(&mut self, buffer: &mut [f32]);
    fn reset(&mut self);
}

// MonoProcessor - marker for Stereo<P> wrapper compatibility
trait MonoProcessor: AudioProcessor {}

// StereoProcessor - linked stereo (shared detection/gain)
trait StereoProcessor: Send {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]);
    fn process_mono(&mut self, buffer: &mut [f32]);
    fn reset(&mut self);
}
```

**Stereo<P> Wrapper** - For dual-mono (no linking):
```rust
let stereo = Stereo::from_mono(mono_processor);  // Clones for L/R
stereo.set_both(|p| p.set_param(value));  // Apply to both
```

**Linked Stereo Processors** - Implement StereoProcessor directly:
```rust
// Example: StereoFetCompressor detects on max(L,R), applies same gain
pub struct StereoFetCompressor {
    envelope_db: f32,  // Shared state
    // ...
}

impl StereoProcessor for StereoFetCompressor {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        // Linked detection: max of both channels
        let peak = left_sample.abs().max(right_sample.abs());
        // Apply SAME gain to both
    }
}
```

**CLI Pattern for StereoProcessor (split_at_mut):**
```rust
if is_stereo {
    let (left, right) = samples.split_at_mut(1);
    processor.process_stereo(&mut left[0], &mut right[0]);
} else {
    processor.process_mono(&mut samples[0]);
}
```

## Preset-Based Processors (1-5 levels)

Used by: Denoiser, SpectralGate, DeReverb
```rust
Processor::new_with_preset(sample_rate, level)  // level: 1-5
PRESET_NAMES: [&str; 5] = ["Gentle", "Light", "Moderate", "Strong", "Aggressive"]
get_preset_name(level) -> &str  // Returns "Unknown" for 0
```

## STFT Processing Pattern

```rust
// 1. Create processor
let mut proc = Processor::new(sample_rate);

// 2. Initialize from analysis (optional but recommended)
proc.init_with_analysis(&analysis_result);
// or
proc.init_noise_floor(&noise_floor);

// 3. Process (returns new Vec)
let output = proc.process(&input);
```

## Dynamics Processors

- `FetCompressor` (mono) / `StereoFetCompressor` (linked) - Feedback FET
- `StereoVcaPeakComp` - Look-ahead VCA with linked detection
- `ButterComp2` - Airwindows bipolar algorithm
- `Limiter` - Soft-knee true peak with lookahead

## Analysis Cascade

Analysis flows through stages, reusing FFT data:
```
SpectralAnalysis (4096 FFT)
    ↓ power_spectrum, avg_power
CepstralAnalysis
    ↓ f0, voiced_ratio, spectral_envelope
ReverbAnalysis
    → rt60_ms, drr_db, decay_per_band
```

Key structs:
- `SpectralAnalysis` - avg power spectrum, band energies
- `CepstralAnalysis` - f0 detection, envelope, echo detection
- `ReverbAnalysis` - RT60 estimation, per-band decay rates

## Two-Phase Processing

Most CLI processing uses two passes:

**Pass 1 (Analysis):**
- Read entire file into memory
- Run SpectralAnalysis → CepstralAnalysis → ReverbAnalysis
- Detect tonal peaks
- Compute noise floor (minimum statistics)

**Pass 2 (Processing):**
- Initialize processors with analysis results
- Process audio through chain
- Write output

## CLI Processing Chain

Complete chain with all stages:

```
1. Filters (HP 80Hz, LP 15.5kHz)
2. Input gain normalization (-18 LUFS target)
3. Declick (optional, offline only)
4. DeReverb (optional, init with ReverbAnalysis)
5. Denoise (spectral subtraction)
6. Spectral Gate (non-stationary noise)
7. Peak Attenuator (tonal noise removal)
8. Dynamics:
   - PeakComp (look-ahead compression)
   - OR FetComp (alternative compressor)
9. FixEQ (demud + 2-band correction)
10. DeEsser (sibilance reduction)
11. Saturation:
    - TapeGlue (Jiles-Atherton hysteresis, f64 internal)
    - Channel9 (Neve-style transformer)
12. Compression (ButterComp)
13. Enhancement:
    - EnhanceEQ OR RadioVoice (broadcast EQ)
14. LUFS normalization (-16 LUFS target)
15. Limiter (true peak -1dB)
```

## Module Organization

```
crates/poddyclip/src/
├── lib.rs           # Public exports
├── traits.rs        # AudioProcessor, StereoProcessor, etc.
├── stft/            # Shared STFT infrastructure
├── denoiser/        # Spectral subtraction
│   ├── core.rs      # RealtimeDenoiser, StreamingDenoiser
│   ├── common.rs    # Presets, constants, SFM thresholds
│   ├── analysis.rs  # One-pass FFT analysis
│   ├── spectral_gate.rs    # Non-stationary noise gate
│   └── peak_attenuator.rs  # Tonal peak removal
├── dereverb/        # Spectral de-reverb
│   ├── core.rs      # DeReverbProcessor
│   └── common.rs    # Presets, params
├── dynamics/        # Compressors & limiters
│   ├── fetcomp.rs   # Feedback FET compressor
│   ├── peakcomp.rs  # Look-ahead VCA
│   ├── buttercomp.rs # Airwindows algorithm
│   ├── limiter.rs   # True peak limiter
│   └── autogain.rs  # Automatic gain staging
├── eq/              # Filters & EQ
│   ├── filters.rs   # SVF biquad implementations
│   ├── fixeq.rs     # Dynamic 3-band EQ
│   ├── deesser.rs   # Sibilance reduction
│   ├── enhanceeq.rs # Presence/air enhancement
│   └── radio_voice.rs # Broadcast-style processing
├── saturation/      # Harmonic enhancement
│   ├── tape.rs      # TapeGlue (Jiles-Atherton)
│   └── channel9.rs  # Neve transformer model
├── repair/          # Audio repair
│   └── declicker.rs # Click/pop removal (offline)
└── analysis/        # Audio analysis
    ├── lufs.rs      # LUFS metering
    ├── spectral.rs  # 4096-FFT spectral analysis
    ├── cepstral.rs  # f0 detection, envelope
    └── reverb.rs    # RT60, DRR estimation
```

## Key Constants

```rust
// STFT (real-time)
RT_WINDOW_SIZE = 2048
RT_HOP_SIZE = 1024
RT_N_BINS = 1025

// STFT (analysis)
ANALYSIS_WINDOW_SIZE = 4096
ANALYSIS_HOP_SIZE = 2048

// Common
EPSILON = 1e-10  // Avoid log(0)

// Noise detection (SFM = Spectral Flatness Measure)
DEFAULT_SFM_SPEECH = 0.4  // Below = speech
DEFAULT_SFM_NOISE = 0.6   // Above = noise

// LUFS targets
INPUT_LUFS_TARGET = -18.0
OUTPUT_LUFS_TARGET = -16.0
```

## Stereo Handling Summary

**STFT Processors (Denoiser, SpectralGate, etc.):**
- Create separate instances for L/R
- Process independently: `samples[0] = proc.process(&samples[0])`
- No stereo linking - may cause artifacts if detection differs between channels

**Sample-Based Processors:**
- Use `Stereo<P>` wrapper for dual-mono (no linking)
- Use dedicated `StereoXxx` types for linked operation
- CLI uses `split_at_mut(1)` to get mutable L/R refs:
```rust
let (left, right) = samples.split_at_mut(1);
processor.process_stereo(&mut left[0], &mut right[0]);
```

## Adding New Processors

### STFT-Based Processor
1. Create in `denoiser/` or new module under `crates/poddyclip/src/`
2. Use `StftProcessor` from `stft/` module
3. Implement: `new(sample_rate)`, `process(&[f32]) -> Vec<f32>`, `reset()`
4. Add preset support if needed: `new_with_preset()`, `PRESETS` array
5. Export from module's `mod.rs` and `lib.rs`
6. In CLI: create separate L/R instances, process independently

### Sample-Based Processor
1. Create in appropriate module (`dynamics/`, `eq/`, etc.)
2. Implement `AudioProcessor` trait for mono
3. For linked stereo: create `StereoXxx` struct implementing `StereoProcessor`
4. Export from `lib.rs`
5. In CLI: use `split_at_mut(1)` pattern for stereo

## Plugin Architecture

Uses nih-plug with egui GUI. Plugin wraps the same `RealtimeDenoiser` from the library. Parameters in `params.rs`, visualizations use `egui_plot`.
