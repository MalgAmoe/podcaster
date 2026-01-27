# CLAUDE.md

!!!! IMPORTANT

YOU MUST USE memo TO SAVE INFO BECAUSE COMPACTING MAKE YOU LOOSE IMPORTANT CONTEXT.

When we see test failing we need to understand why, and fix the situation!

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Memory

  Start sessions with `memo context`. Before answering questions about this project, try `memo similar "topic" --here`.

  Save important discoveries:
  - `memo remember learned "..."` - gotchas, patterns, how things work
  - `memo remember preference "..."` - user choices, workflow preferences
  - `memo remember fact "..."` - config locations, API details, decisions made

  Remember to use it before compacting if you want to save important info.


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
./target/release/poddyclip input.wav --preset 2
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

## Preset-Based Processors (1-3 levels)

Used by: Denoiser, SpectralGate, DeReverb
```rust
Processor::new_with_preset(sample_rate, level)  // level: 1-3
PRESET_NAMES: [&str; 3] = ["Subtle", "Balanced", "Intense"]
get_preset_name(level) -> &str  // Returns "Unknown" for 0 or >3
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
6. AI Clean (optional, isolates voice)
7. Spectral Gate (non-stationary noise)
8. Peak Attenuator (tonal noise removal)
9. Dynamics:
   - PeakComp (look-ahead compression)
   - OR FetComp (alternative compressor)
10. FixEQ (demud + 2-band correction)
11. DeEsser (sibilance reduction)
12. Saturation:
    - TapeGlue (Jiles-Atherton hysteresis, f64 internal)
    - Channel9 (Neve-style transformer)
13. Compression (ButterComp)
14. Enhancement:
    - EnhanceEQ OR RadioVoice (broadcast EQ)
15. LUFS normalization (-16 LUFS target)
16. Limiter (true peak -1dB)
```

## AI Clean

Deep learning noise removal that isolates voice and removes everything else. More thorough than spectral denoising but requires additional processing time.

**API:**
- `ProcessConfig.ai_denoise: bool` - enable AI cleaning
- `CreateS3JobRequest.ai_clean: Option<bool>` - override default
- Default: OFF (user must opt-in)
- Runs after spectral denoising, before spectral gate

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
├── deepfilter/      # AI denoiser (optional, feature-gated)
│   ├── core.rs      # DeepFilterDenoiser wrapper
│   └── analysis.rs  # SNR analysis for auto-tuning
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

Uses nih-plug with egui GUI. Plugin wraps processors from the core library.

**Key Files:**
- `params.rs` - Parameter definitions (nih-plug `#[derive(Params)]`)
- `lib.rs` - Plugin struct, processing logic, parameter sync
- `editor.rs` - egui GUI with controls and meters

### Adding a Processor to the Plugin

#### 1. Add Parameters (`params.rs`)

```rust
// Define parameter struct
#[derive(Params)]
pub struct MyProcessorParams {
    #[id = "myproc_enable"]
    pub enable: BoolParam,

    #[id = "myproc_amount"]
    pub amount: FloatParam,
}

// Add to main params struct
#[derive(Params)]
pub struct PoddyclipParams {
    // ... existing params ...

    #[nested(group = "My Processor")]
    pub myproc: MyProcessorParams,
}

// Add defaults in impl Default for PoddyclipParams
myproc: MyProcessorParams {
    enable: BoolParam::new("Enable My Processor", false),
    amount: FloatParam::new(
        "Amount",
        0.5,
        FloatRange::Linear { min: 0.0, max: 1.0 },
    )
    .with_step_size(0.01)
    .with_value_to_string(formatters::v2s_f32_percentage(0)),
},
```

#### 2. Add Processor to Plugin (`lib.rs`)

```rust
// Add import
use poddyclip::dynamics::MyProcessor;  // or StereoMyProcessor

// Add to Poddyclip struct
pub struct Poddyclip {
    // ... existing fields ...
    myproc: StereoMyProcessor,
    myproc_gain_db: Arc<Mutex<f32>>,  // For GR meter
}

// Add to Default impl
impl Default for Poddyclip {
    fn default() -> Self {
        Self {
            // ... existing ...
            myproc: StereoMyProcessor::new(48000.0),
            myproc_gain_db: Arc::new(Mutex::new(0.0)),
        }
    }
}

// Add to initialize() if processor needs sample rate
self.myproc = StereoMyProcessor::new(buffer_config.sample_rate);

// Add to reset()
self.myproc.reset();
```

#### 3. Add Processing Logic (`lib.rs`)

In `process_mono()` and `process_stereo()`:

```rust
// Read enable flag at buffer start
let myproc_enabled = self.params.myproc.enable.value();

// In the sample loop, add processing:
if myproc_enabled {
    (left_out, right_out) = self.myproc.process_sample(left_out, right_out);
}
```

In `sync_processor_params()`:

```rust
// Sync parameters from UI to processor
if self.params.myproc.enable.value() {
    self.myproc.set_amount(self.params.myproc.amount.value());
}
```

In `update_visualization()`:

```rust
if let Ok(mut gain) = self.myproc_gain_db.try_lock() {
    *gain = self.myproc.get_gain_reduction_db();
}
```

#### 4. Pass GR Meter to Editor (`lib.rs`)

Update `editor()` to pass the new meter:

```rust
fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
    editor::create_plugin_editor(
        // ... existing args ...
        self.myproc_gain_db.clone(),
    )
}
```

#### 5. Add UI Controls (`editor.rs`)

Update function signature:

```rust
pub fn create_plugin_editor(
    // ... existing args ...
    myproc_gain: Arc<Mutex<f32>>,
) -> Option<Box<dyn Editor>> {
```

Add control section in `draw_params_column()`:

```rust
draw_section(ui, "My Processor", |ui| {
    param_row(ui, "Enable:", &params.myproc.enable, setter);
    labeled_slider(ui, "Amount:", &params.myproc.amount, setter);
});
```

Add meter color (if showing GR meter):

```rust
enum MeterColor {
    // ... existing ...
    MyColor,  // Add new color
}

impl MeterColor {
    fn color_for_ratio(self, ratio: f32) -> egui::Color32 {
        // ... existing ...
        MeterColor::MyColor => {
            if ratio > 0.8 { egui::Color32::from_rgb(R, G, B) }
            else if ratio > 0.5 { egui::Color32::from_rgb(R, G, B) }
            else { egui::Color32::from_rgb(R, G, B) }
        }
    }
}
```

Add meter in `draw_viz_column()`:

```rust
fn draw_viz_column(
    // ... add myproc_gain parameter ...
) {
    // ... existing meters ...

    ui.heading("My Processor Gain Reduction");
    ui.add_space(5.0);
    let gain = myproc_gain.lock().map(|g| *g).unwrap_or(0.0);
    draw_gr_meter(ui, gain, 12.0, MeterColor::MyColor);
}
```

#### 6. Build and Test

```bash
cargo xtask bundle poddyclip-plugin --release
# Plugin at: target/bundled/Poddyclip.clap and .vst3
```

### Processing Chain Order (Plugin)

```
1. Filter (HP + LP)
2. Denoiser
3. Expander
4. FET Compressor
5. Peak Compressor
6. FixEq
7. De-Esser
8. Channel9 (Neve)
9. Enhance EQ
10. ButterComp
11. TapeGlue
12. Limiter
```

## API Processing

The `poddyclip-api` crate provides HTTP endpoints for audio processing.

**Features:**
- `deepfilter` (default) - Enables AI denoiser

**ProcessConfig fields (key ones):**
- `ai_denoise: bool` - Enable DeepFilterNet (default: false, optional for voice category)
- `denoiser_preset: u8` - Spectral subtraction level 1-3
- `dereverb: u8` - DeReverb level 0-3 (0 = off)
- `spectral_gate: u8` - Gate level 0-3 (0 = off)

**Processing modes (via category/mode/strength):**
| Mode | AI Clean | Saturation |
|------|----------|------------|
| Natural | Optional (voice) | Light |
| Studio | Optional (voice) | Full |

**API stages (25 total):**
```
0-decoding, 1-filters, 2-input_gain, 3-analyzing_reverb, 4-dereverb,
5-analyzing_noise, 6-denoise, 7-ai_denoise, 8-spectral_gate,
9-analyzing_peaks, 10-peak_attenuation, 11-expander, 12-compressor,
13-analyzing_eq, 14-fixeq, 15-deesser, 16-saturation, 17-buttercomp,
18-analyzing_enhance, 19-enhanceeq, 20-tape, 21-radio,
22-analyzing_levels, 23-output, 24-encoding
```
