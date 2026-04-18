# CLAUDE.md

<!-- !!!! IMPORTANT

YOU MUST USE memo TO SAVE INFO BECAUSE COMPACTING MAKE YOU LOOSE IMPORTANT CONTEXT.

When we see test failing we need to understand why, and fix the situation!

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Memory

  Start sessions with `memo context`. Before answering questions about this project, try `memo similar "topic" --here`.

  Save important discoveries:
  - `memo remember learned "..."` - gotchas, patterns, how things work
  - `memo remember preference "..."` - user choices, workflow preferences
  - `memo remember fact "..."` - config locations, API details, decisions made

  Remember to use it before compacting if you want to save important info. -->


## Build Commands

```bash
# Rust
cargo build --release -p poddyclip-cli      # CLI
cargo build --release -p poddyclip-api      # API server
cargo test                                  # all tests
cargo test -p poddyclip-api                 # API tests only

# Phoenix (Elixir)
cd backend && mix compile
cd backend && mix test

# SolidJS frontend
cd backend/assets && node build.mjs         # one-shot build
cd backend/assets && node build.mjs --watch # watch mode
cd backend/assets && npm test               # Playwright guest-preview checks

# Full dev stack (Postgres + MinIO, then API, then Phoenix)
./dev.sh infra    # terminal 1
./dev.sh api      # terminal 2  (Rust API on :3000)
./dev.sh web      # terminal 3  (Phoenix on :4000)
```

**macOS gotcha:** `dev.sh` exports `SSL_CERT_FILE=/etc/ssl/cert.pem` on Darwin to work around `rustls-native-certs 0.6.3` choking on macOS keychain certs (produces `InvalidCertificate(BadEncoding)` when connecting to MinIO).

## Project Structure

Rust workspace + Elixir/Phoenix app.

**Rust crates (`crates/`):**
- `poddyclip` — Core audio processing library (no audio I/O)
- `poddyclip-cli` — CLI tool (symphonia in, hound out)
- `poddyclip-api` — HTTP API (Axum) that Phoenix calls for processing

**Elixir app:**
- `backend/` — Phoenix 1.8 + LiveView; serves SolidJS frontend at `/app/*`, handles auth/billing/job orchestration. Talks to `poddyclip-api` via HTTP.

**Frontend:**
- `backend/assets/js/solid/` — SolidJS SPA, bundled into Phoenix static assets.

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
5. Denoise (spectral subtraction, optional)
6. AI Clean (MossFormer2, optional)
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

## Denoising

The production AI-clean path is now MossFormer2 behind the generic `ai_clean` interface.

- Feature-gated in Rust by the `mossformer2` feature
- `AiCleanProcessor` expects 48kHz input; CLI and API resample in and back out around that step
- The CLI can still use the normal spectral denoiser, spectral gate, dereverb, etc.
- The API uses MossFormer2 in the fixed production chain and does not expose a user toggle for it

## Module Organization

```
crates/poddyclip/src/
├── lib.rs           # Public exports
├── traits.rs        # AudioProcessor, StereoProcessor, etc.
├── ai_clean/        # MossFormer2 model adapter
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
├── sample_rate.rs   # Shared resampling helper
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

## API Processing (`poddyclip-api`)

HTTP server (Axum) that processes audio with a single fixed pipeline. Not configurable per-request — there's no strength selector, no toggles, no presets exposed to the client.

**Features:**
- `mossformer2` (default) — MossFormer2 AI clean path

**`ProcessConfig` is built via `ProcessConfig::new()`** — a static preset. There is no `from_strength()` and no runtime branching. Fixed settings:
- Filters: HP 85Hz @ 24dB/oct
- Declick: ON
- Denoise: MossFormer2 AI clean in the `denoise` stage
- PeakComp: preset 1
- FixEQ: OFF (reserved — flag exists but default off)
- DeEsser: ON
- EnhanceEQ: preset 1
- FetComp: OFF (reserved)
- LUFS target: −16

**Removed from the API chain** (still in the library — CLI may use them): dereverb, spectral gate, saturation (Channel9), tape, ButterComp, radio voice, mono/center-audio summing, spectral-subtraction denoiser.

**Request body — `CreateS3JobRequest`:**
- `input_s3_key` (required), `user_id`, `filename`
- `phoenix_job_id`, `webhook_url`, `webhook_secret`
- `output_format` ("mp3"/"wav"), `mp3_bitrate` (default 192)
- No strength, no ai_clean, no mono.

**Preview path — `POST /preview`:**
- raw audio bytes in
- synchronous processing
- hard duration limit enforcement for the guest demo path
- WAV bytes out
- no S3, no webhook, no background job state

**Pipeline stages (13 total):**
```
0-decoding, 1-filters, 2-input_gain, 3-denoise, 4-peakcomp,
5-analyzing_eq, 6-fixeq, 7-deesser, 8-analyzing_enhance,
9-enhanceeq, 10-fetcomp, 11-analyzing_levels, 12-output
```
Stages skip when their processor is disabled; progress indices stay stable.

## Phoenix Backend (`backend/`)

Elixir/Phoenix app that handles auth, billing, job orchestration, and serves the SolidJS frontend. Talks to `poddyclip-api` via HTTP (`PoddyclipBackend.Processing.Client`).

**Key modules:**
- `PoddyclipBackend.Accounts` — users, sessions, magic-link auth, guest users, guest→user merge
- `PoddyclipBackend.Billing` — plans, minute packs, Polar integration
- `PoddyclipBackend.Processing` — job queue (Oban), submit/cancel, lifecycle
- `PoddyclipBackend.Storage` — S3 (Cloudflare R2) via `ex_aws_s3`, presigned URLs, filter_existing_keys
- `PoddyclipBackend.Feedback` — form-submitted feedback from `/feedback`

**Routes of note** (`lib/poddyclip_backend_web/router.ex`):
- `/app` and `/app/*path` → SolidJS app (via `PageController.app`, catch-all)
- `/api/preview` (POST) → synchronous guest preview path
- `/api/jobs` (POST) → create job; minimal payload: `s3_key`, `filename`, `duration_seconds`
- `/api/jobs/current`, `/api/jobs/history`, `/api/jobs/:id/download_url`, etc.
- `/api/internal/jobs/:id/status` — Rust API webhook callback
- `/api/webhooks/polar` — Polar billing webhook
- `/feedback` — form page (Phoenix-rendered), NOT a JSON API

**Workers (Oban):** `ProcessingWorker`, `FreePlanResetWorker`, `SubscriptionExpiryWorker`, `ExpiryNotificationWorker`, `CleanupJobs`, `CleanupOrphanedFiles`, `CleanupExpiredPacks`.

**Admin endpoint** on port 4001 (separate `AdminEndpoint`, only compiled when `ADMIN_ENABLED=true`). Access via SSH tunnel — not exposed publicly.

## Frontend (`backend/assets/js/solid/`)

**Stack:** SolidJS 1.9 + `@solidjs/router` + Tailwind CSS + daisyUI, bundled with esbuild (`build.mjs`). Mounts into `<div id="solid-process-app">` on `/app/*`.

**Routes (client-side):** `/` → `ProcessPage`, `/past-munchings` → `PastMunchingsPage`. Base path is `/app`.

**State:** two contexts — `ProcessContext` (upload/job state, Phoenix Channel subscription for live progress) and `NotificationContext` (toast queue). No i18n.

**Upload / preview flow:**
- signed-in users: `/api/presign-upload` → direct upload to S3 via XHR → `POST /api/jobs`
- guests: browser trims to a 30-second WAV preview → `POST /api/preview` → synchronous processed WAV result
- guest preview does not use S3, Oban, or persisted jobs

**Theme:** custom daisyUI theme `poddyclip` (dark purple, `#9333ea`) in `assets/css/app.css`. DM Sans font.

## Recent cleanup — what was removed

Context for future edits (don't re-add without explicit ask):
- **Blog** (`nimble_publisher`/`earmark`, markdown posts, `BlogController`, sitemap entries)
- **Translations** (`gettext`, `Gettext`, `LocaleHelpers`, `SetLocale` plug, `LocaleHook`, all `.po`/`.pot` files, `@solid-primitives/i18n`, `locales/en.js`, `translate.js`)
- **OpenObserve / LogShipper** (custom Logger `:gen_event` backend, shipper GenServer)
- **Strength selector** (`ProcessConfig::from_strength`, strength UI, `StrengthKnob.jsx`, strength param across the whole stack)
- **AI Clean toggle** (now always on)
- **Center audio / mono summing** (toggle, config field, engine block)
- **Spectral-subtraction denoiser** (normal `RealtimeDenoiser` path in API; library still has it)
- **Dereverb, spectral gate, saturation, tape, ButterComp, radio voice** from the API chain (library keeps them)
- **QualityRating / PmfSurvey** post-job components
- **JSON feedback API** (`/api/feedback`, `FeedbackController`, `submitFeedback` JS helper) — only the form-based `/feedback` page remains
- **VST3/CLAP plugin** (`poddyclip-plugin` crate + `xtask` build runner + `nih-plug`/`bundler.toml` tooling) — web-only product now
- **Standalone registration page** (`/users/register`) — removed; public auth is unified magic-link sign-in

## Deactivated — kept in code, re-enable later

These are **not dead code** — they're temporarily disabled with routes commented out. Templates, controllers, and schemas are intact.
- **Legal pages** (`/terms`, `/privacy`, `/legal`) — actions + HEEx templates still exist. Router comments explain: re-enable when the layout/footer has visible links.
