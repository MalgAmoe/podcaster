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

## Architecture

### Processing Traits (`traits.rs`)

- `AudioProcessor` - Buffer-based mono processing: `process_buffer(&mut [f32])`
- `StereoProcessor` - Stereo with optional linking: `process_stereo(&mut [f32], &mut [f32])`
- `FrameProcessor` - FFT frame-based (denoisers): `process_frame(&[f32]) -> Vec<f32>`
- `Stereo<P>` - Generic wrapper that creates dual-mono from any mono processor

### FFT Processors

All FFT-based processors (denoiser, dereverb) use:
- `WINDOW_SIZE = 2048`, `HOP_SIZE = 1024` (defined in `denoiser/common.rs`)
- `rustfft` for FFT operations
- Each processor has its own FFT planner and handles overlap-add internally
- Pattern: `processor.process(&samples) -> Vec<f32>` for batch, `process_frame()` for streaming

### Adding New Processors

1. Put processing logic in the library crate (`crates/poddyclip/src/`)
2. Implement appropriate trait (`AudioProcessor` for time-domain, custom for FFT)
3. FFT processors should include their own `process()` method that handles windowing/overlap-add
4. Export from `lib.rs`
5. CLI just calls the library - no FFT code in CLI

### Module Organization

- `denoiser/` - Spectral subtraction noise reduction (core.rs, common.rs, analysis.rs)
- `dereverb/` - Spectral de-reverb
- `dynamics/` - Compressors (buttercomp, fetcomp, peakcomp), limiter, autogain
- `eq/` - Filters, de-esser, fixeq, enhanceeq, radio voice processor
- `saturation/` - Channel9, TapeGlue
- `repair/` - Declicker (offline only)
- `analysis/` - LUFS, spectral, cepstral, reverb analysis

### CLI Processing Chain

The CLI applies processors in this order:
1. Filters (HP 80Hz, LP 15.5kHz)
2. Input gain normalization
3. Declick (optional, offline)
4. DeReverb (optional)
5. Denoise
6. Dynamics (PeakComp or FET)
7. FixEQ
8. DeEsser
9. Saturation
10. Compression
11. EnhanceEQ or RadioVoice
12. LUFS normalization
13. Limiter

### Plugin Architecture

Uses nih-plug with egui GUI. Plugin wraps the same `RealtimeDenoiser` from the library. Parameters in `params.rs`, visualizations use `egui_plot`.
