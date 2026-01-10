# Poddyclip Architecture Refactoring Plan

## Philosophy

Keep it practical. Not everything needs perfect abstraction. The goal is:
1. Make the code clearer and more "Rusty"
2. Enable sample-by-sample chaining where it makes sense
3. Keep different analysis types separate (they're genuinely different)
4. Incremental improvements, not a rewrite

---

## Current State

### Project Structure (after removing aireq)
```
src/
├── analysis/mod.rs        # Shared spectral analysis
├── deesser/{mod,analysis} # Sibilance reduction
├── denoiser/{common,denoiser,denoiser_rt,mod} # FFT-based noise reduction
├── distortion/{channel9,tape,mod}  # Saturation effects
├── dynamic/{autogain,buttercomp,limiter,mod} # Dynamics
├── enhanceeq/mod.rs       # 3-band enhancement
├── filters/{common,dynamic,mod}    # SVF filters + dynamic bands
├── fixeq/{mod,analysis}   # Corrective EQ
├── output/lufs            # Loudness metering
├── peakcomp/{mod,analysis} # VCA peak compression
├── lib.rs                 # Plugin
├── main.rs                # CLI
└── visualizations.rs      # Plugin UI helpers
```

### Current Pipeline & Analysis Points

```
Input
  │
  ├─ [AutoGain Analysis] ─────────────── RMS + Peak calculation
  │   └─ apply_gain()
  │
  ├─ [Filters: HP 80Hz + LP 15.5kHz] ─── No analysis, sample-by-sample
  │
  ├─ [Denoiser Analysis] ─────────────── Noise floor estimation (unique, FFT-based)
  │   └─ SpectralSubtractionDenoiser
  │
  ├─ [PeakComp Analysis] ─────────────── Histogram for threshold (unique)
  │   └─ VcaPeakComp
  │
  ├─ [Spectral Analysis] ─────────────── Shared: SpectralAnalysis (one FFT)
  │   ├─ FixEq.configure_from_spectrum()
  │   ├─ DeEsser.configure_from_spectrum()
  │   └─ EnhanceEq.configure_from_spectrum()
  │
  ├─ [Processing Chain] ──────────────── All sample-by-sample after config
  │   ├─ FixEq
  │   ├─ DeEsser
  │   ├─ Channel9
  │   ├─ EnhanceEq
  │   ├─ ButterComp
  │   └─ Limiter
  │
  └─ Output
```

### Key Insight: Analysis Types Are Different

| Analysis | What It Computes | Used By | Can Share? |
|----------|-----------------|---------|------------|
| RMS/Peak | Loudness stats | AutoGain | Standalone |
| Noise Floor | Per-bin noise power | Denoiser | Unique (needs silence) |
| Peak Histogram | Threshold detection | PeakComp | Unique |
| Spectral Shape | Tilt, peaks, energy | FixEq, EnhanceEq | Yes - already shared |
| Sibilance | 4-10kHz peaks | DeEsser | Uses shared spectral |

**Conclusion:** Don't try to unify all analysis. Focus on:
1. Spectral analysis is already shared (good)
2. DeEsser uses the same SpectralAnalysis (same FFT, different calculation)
3. Other analysis types are genuinely different

---

## What We Can Improve Without Big Decisions

### Step 1: Basic Trait for Sample Processors

Add a simple trait to formalize what we already have:

```rust
// src/traits.rs (new file)

/// Any processor that can handle samples one at a time
pub trait SampleProcessor {
    fn process(&mut self, input: f32) -> f32;
    fn reset(&mut self);
}

/// Convenience for stereo
pub struct Stereo<P> {
    pub left: P,
    pub right: P,
}

impl<P: SampleProcessor + Clone> Stereo<P> {
    pub fn new(p: P) -> Self {
        Self { left: p.clone(), right: p }
    }

    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        (self.left.process(l), self.right.process(r))
    }
}
```

**Why:** Formalizes the pattern we already use. No breaking changes.

### Step 2: Implement Trait for Existing Processors

Add `impl SampleProcessor for X` to existing processors:
- `ButterComp2` - already has `process(f32) -> f32`
- `Channel9` - already has `process(f32) -> f32`
- `EnhanceEq` - already has `process(f32) -> f32`
- `FixEq` - has `process_sample_left/right`, needs small refactor
- `DeEsser` - has `process(f32) -> f32`
- `Limiter` - has lookahead, needs thought

**Why:** Each processor keeps working as-is, just gains trait.

### Step 3: Sample Chain Helper

```rust
// src/chain.rs (new file)

/// Chain processors for efficient sample-by-sample processing
pub struct SampleChain<'a> {
    processors: Vec<&'a mut dyn SampleProcessor>,
}

impl<'a> SampleChain<'a> {
    pub fn new() -> Self {
        Self { processors: vec![] }
    }

    pub fn add(&mut self, p: &'a mut dyn SampleProcessor) -> &mut Self {
        self.processors.push(p);
        self
    }

    #[inline]
    pub fn process(&mut self, mut sample: f32) -> f32 {
        for p in &mut self.processors {
            sample = p.process(sample);
        }
        sample
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer {
            *sample = self.process(*sample);
        }
    }
}
```

**Why:** Enables chaining without changing existing code.

---

## Design Decisions

### DeEsser Uses Shared Spectrum

DeEsser uses the same FFT/SpectralAnalysis as FixEq and EnhanceEq. They share the spectral data - only the calculation of what they extract is different. This avoids a second pass over the file.

```rust
// In deesser/mod.rs - add this method
pub fn configure_from_spectrum(&mut self, spectrum: &SpectralAnalysis) -> &SibilanceAnalysis {
    // Use spectrum.find_peak_deviation(4000.0, 10000.0, 6000.0) for sibilance
    // Same FFT data, different frequency range and interpretation
}
```

### Sample Chain: FixEq + DeEsser + EnhanceEq + ButterComp + Channel9

After all are configured from analysis, these processors can be chained sample-by-sample:

```rust
let mut chain = SampleChain::new();
chain.add(&mut fix_eq)
     .add(&mut deesser)
     .add(&mut enhance_eq)
     .add(&mut butter_comp)
     .add(&mut channel9);

chain.process_buffer(&mut samples);
```

This is the "enhance" portion of the pipeline - all sample-by-sample after configuration.

### Chunk-Based Processing

Use chunk-based processing for better cache locality:

```rust
const CHUNK: usize = 64;  // L1 cache friendly

for chunk in buffer.chunks_mut(CHUNK) {
    for sample in chunk {
        *sample = chain.process(*sample);
    }
}
```

This is better than processing the whole file sample-by-sample in one pass.

### Decision: Practical Dispatch

Use whatever dispatch method is more practical and useful for the situation:
- **Dynamic dispatch (trait objects)**: More flexible, easier to compose at runtime
- **Static dispatch**: When performance is critical and chain is known at compile time

```rust
// Use dynamic when building chains at runtime
let chain: Vec<Box<dyn SampleProcessor>> = vec![...];

// Use static for fixed, hot paths
#[inline]
fn process_fixed_chain(s: f32, a: &mut A, b: &mut B) -> f32 {
    b.process(a.process(s))
}
```

Don't be dogmatic - measure if it matters, choose what's cleaner.

---

## Incremental Steps (No Big Decisions Required)

### Phase 1: Add Traits (Safe, Non-Breaking) - DONE
1. Created `src/traits.rs` with `SampleProcessor` trait + `Stereo<P>` wrapper
2. Added `mod traits;` to lib.rs and main.rs
3. Implemented trait for ButterComp2, Channel9, EnhanceEq, FixEq, DeEsser
4. Existing API unchanged, trait is additive

### Phase 2: Sample Chain Helper (Optional Use)
1. Create `src/chain.rs`
2. Add `mod chain;`
3. Use in CLI as experiment
4. Compare performance

### Phase 3: Clean Up lib.rs (Plugin)
1. Extract params to `src/plugin/params.rs`
2. Extract UI to `src/plugin/ui.rs`
3. Keep lib.rs as thin wrapper
4. No functional changes

### Phase 4: Project Structure (After Core Changes)
First change the core, then evaluate project structure:
- Do we need separate crates?
- Is current feature flag approach sufficient?
- Does workspace add value or just complexity?

**Order:** Core improvements first, project restructuring second.

---

## What NOT To Do (Over-Engineering)

1. **Don't unify all analysis into one mega-struct**
   - Noise floor analysis is fundamentally different from spectral
   - Histogram analysis for peaks is unique

2. **Don't force everything into one Pipeline abstraction**
   - Some things are buffer-based (denoiser)
   - Some have lookahead (limiter, peakcomp)
   - Mixing them in one abstraction adds complexity

3. **Don't abstract parameters into a generic system**
   - Each processor has unique params
   - NIH-plug already handles plugin params

4. **Don't create workspace until we need it**
   - Current feature flags (cli/plugin) work fine
   - Workspace adds build complexity

---

## Files to Create/Modify

### New Files
| File | Purpose |
|------|---------|
| `src/traits.rs` | SampleProcessor trait, Stereo wrapper |
| `src/chain.rs` | SampleChain helper (optional) |
| `src/plugin/mod.rs` | Plugin-specific code (extracted from lib.rs) |
| `src/plugin/params.rs` | Parameter definitions |
| `src/plugin/ui.rs` | UI code |

### Files to Modify
| File | Change |
|------|--------|
| `src/lib.rs` | Slim down, use plugin module |
| `src/dynamic/buttercomp.rs` | Add `impl SampleProcessor` |
| `src/distortion/channel9.rs` | Add `impl SampleProcessor` |
| `src/enhanceeq/mod.rs` | Add `impl SampleProcessor` |
| `src/deesser/mod.rs` | Add `configure_from_spectrum()` |

---

## Testing Each Step

### After Phase 1 (Traits)
```bash
cargo test --features cli
cargo build --features plugin
# Run CLI on test file, compare output to before
```

### After Phase 2 (Chain)
```bash
cargo bench  # Add benchmarks first
# Compare chained vs separate processing
```

### After Phase 3 (lib.rs cleanup)
```bash
cargo build --features plugin
# Test plugin in DAW
# Verify all params still work
```

---

## Decisions Made

1. **DeEsser uses shared SpectralAnalysis** - Same FFT, different calculation
2. **Chunk-based processing** - 64 samples per chunk for cache locality
3. **Practical dispatch** - Use what's cleaner, measure if it matters
4. **Core first, structure later** - Improve processors before reorganizing project

---

## Summary

**Immediate tasks:**
1. Add SampleProcessor trait to formalize existing pattern
2. Implement trait for ButterComp, Channel9, EnhanceEq, FixEq, DeEsser
3. Add `configure_from_spectrum()` to DeEsser (use shared FFT)
4. Create SampleChain helper with chunk-based processing
5. Clean up lib.rs by extracting plugin modules

**After core changes:**
- Evaluate if project restructuring is needed
- Consider workspace only if complexity is justified

**Avoid:**
- Unified analysis system (they're genuinely different)
- Over-abstracted pipeline builder
- Generic parameter system
- Premature workspace restructuring

**Goal:** Clarity, Rustiness, practical performance gains. Not a rewrite.
