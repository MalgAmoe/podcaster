## Spectral Subtraction Algorithm — Visual Walkthrough

---

## 1. Big Picture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         AUDIO STREAM                                │
│  ════════════════════════════════════════════════════════════►     │
│  samples continuously flowing in                                    │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      FRAME EXTRACTION                               │
│                                                                     │
│   ┌──────────────────────────────────────────┐                     │
│   │  Frame m-1   │▓▓▓▓▓▓▓▓▓▓▓▓▓▓│            │                     │
│   │              │  Frame m     │▓▓▓▓▓▓▓▓▓▓▓▓│                     │
│   │              │              │  Frame m+1  │                     │
│   └──────────────────────────────────────────┘                     │
│                  ◄── 50% overlap ──►                                │
│                                                                     │
│   Window: 2048 samples                                              │
│   Hop:    1024 samples                                              │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
                    ┌──────────────────────────┐
                    │   PROCESS ONE FRAME      │
                    │   (see detailed flow)    │
                    └──────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      OVERLAP-ADD OUTPUT                             │
│                                                                     │
│   Output buffer:                                                    │
│   ┌──────────────────────────────────────────┐                     │
│   │▓▓▓▓▓▓▓▓▓▓▓▓▓▓│+++++++++++++++│           │                     │
│   │  ready to    │ accumulating  │  zeroes   │                     │
│   │  output      │               │           │                     │
│   └──────────────────────────────────────────┘                     │
│        ▼                                                            │
│   emit 1024 samples, shift left, add next frame                    │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Frame Processing Pipeline

```
┌─────────────┐
│ Input Frame │  2048 samples of audio
│   x[n]      │
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 1: WINDOW                              │
│                                             │
│   x_win[n] = x[n] · w[n]                    │
│                                             │
│   w[n] = sin(π·n/N)   ← root-Hann          │
│                                             │
│   ╭─────────╮                               │
│  ╱           ╲    Tapers edges to zero      │
│ ╱             ╲   Prevents spectral leakage │
│╱               ╲                            │
└─────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 2: FFT                                 │
│                                             │
│   Y[k] = FFT( x_win )                       │
│                                             │
│   Time domain → Frequency domain            │
│   2048 samples → 1025 complex bins          │
│                                             │
│   Each bin = one frequency component        │
│   Bin spacing = fs/N = 48000/2048 ≈ 23 Hz  │
└─────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 3: POWER SPECTRUM                      │
│                                             │
│   P[k] = |Y[k]|² = real² + imag²           │
│                                             │
│   Magnitude squared at each frequency       │
│                                             │
│   dB            │                           │
│    ▲            │ ┌─┐                       │
│    │    ┌─┐     │ │ │                       │
│    │    │ │ ┌─┐ │ │ │                       │
│    │  ┌─┤ │ │ │ │ │ │ ┌─┐                   │
│    └──┴─┴─┴─┴─┴─┴─┴─┴─┴─┴───► freq         │
└─────────────────────────────────────────────┘
       │
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 4: UPDATE NOISE ESTIMATE               │
│         (see detailed diagram below)        │
│                                             │
│   Input: P[k], current noise estimate N̂[k] │
│   Output: updated N̂[k]                     │
│                                             │
│   Decision: is this frame noise or speech?  │
└─────────────────────────────────────────────┘
       │
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 5: COMPUTE GAIN                        │
│         (see detailed diagram below)        │
│                                             │
│   Input: P[k], N̂[k], parameters            │
│   Output: G[k] gain curve (0 to 1)          │
│                                             │
│   Core spectral subtraction happens here    │
└─────────────────────────────────────────────┘
       │
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 6: SMOOTH GAIN                         │
│                                             │
│   Ĝ[k] = γ[k]·Ĝ_prev[k] + (1-γ[k])·G[k]   │
│                                             │
│   Reduces frame-to-frame gain fluctuations  │
│   Prevents "musical noise" artifacts        │
│                                             │
│   Before:  ┌┐┌┐  ┌┐ ┌┐┌┐                    │
│            ││││  ││ ││││   (jumpy)          │
│   After:   ╭──╮  ╭─╮╭──╮   (smooth)         │
└─────────────────────────────────────────────┘
       │
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 7: APPLY GAIN                          │
│                                             │
│   Y_out[k] = Y[k] · Ĝ[k]                   │
│                                             │
│   Multiply each frequency bin by its gain   │
│   Gain < 1 → that frequency gets quieter    │
│   Noise bins get small gain                 │
│   Speech bins get gain ≈ 1                  │
└─────────────────────────────────────────────┘
       │
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 8: IFFT + WINDOW                       │
│                                             │
│   x_out[n] = IFFT( Y_out ) · w[n]          │
│                                             │
│   Back to time domain                       │
│   Apply window again (WOLA requirement)     │
│                                             │
│   w[n]·w[n] at 50% overlap = 1.0 (COLA)    │
└─────────────────────────────────────────────┘
       │
       │
       ▼
┌─────────────────────────────────────────────┐
│ STEP 9: OVERLAP-ADD                         │
│                                             │
│   buffer[0:N] += x_out[0:N]                │
│   output = buffer[0:hop]                    │
│   buffer = roll_left(buffer, hop)           │
│                                             │
│   Reconstructs continuous audio stream      │
└─────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────┐
│ Output: 1024        │
│ denoised samples    │
└─────────────────────┘
```

---

## 3. Noise Estimation Detail

```
┌─────────────────────────────────────────────────────────────────────┐
│                     NOISE ESTIMATION LOGIC                          │
└─────────────────────────────────────────────────────────────────────┘

                         ┌─────────────┐
                         │  P[k]       │  Current frame power
                         │  (1025 bins)│
                         └──────┬──────┘
                                │
                ┌───────────────┴───────────────┐
                │                               │
                ▼                               ▼
┌───────────────────────────┐   ┌───────────────────────────┐
│ COMPUTE SFM               │   │ COMPUTE SPIKE RATIO       │
│                           │   │                           │
│ geo  = exp(mean(log(P)))  │   │ ratio = mean(P / N̂)      │
│ arith = mean(P)           │   │                           │
│ SFM = geo / arith         │   │ is_spike = ratio > θ      │
│                           │   │                           │
│ SFM → 0: tonal (speech)   │   │ θ = 10 means 10dB jump    │
│ SFM → 1: flat (noise)     │   │                           │
└─────────────┬─────────────┘   └─────────────┬─────────────┘
              │                               │
              ▼                               │
┌─────────────────────────────────────┐       │
│ SFM DECISION                        │       │
│                                     │       │
│ if SFM < 0.1:                       │       │
│     update = FALSE  (speech)        │       │
│ elif SFM > 0.4:                     │       │
│     update = TRUE   (noise)         │       │
│ else:                               │       │
│     update = previous_decision      │       │
│             (hysteresis)            │       │
└─────────────┬───────────────────────┘       │
              │                               │
              └───────────────┬───────────────┘
                              │
                              ▼
                    ┌───────────────────┐
                    │ FINAL DECISION    │
                    │                   │
                    │ if update AND     │
                    │    NOT is_spike:  │
                    │                   │
                    │   DO UPDATE       │
                    │ else:             │
                    │   FREEZE          │
                    └─────────┬─────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
              ▼                               ▼
┌─────────────────────────┐     ┌─────────────────────────┐
│ UPDATE                  │     │ FREEZE                  │
│                         │     │                         │
│ N̂ = λ·N̂ + (1-λ)·P      │     │ N̂ = N̂  (unchanged)     │
│                         │     │                         │
│ Slowly blend in new     │     │ Keep old estimate       │
│ power measurement       │     │ Speech would corrupt it │
└─────────────────────────┘     └─────────────────────────┘
```

**Pseudocode:**

```
function update_noise_estimate(P, N̂, frame_count):
    
    # Warmup: always update first 20 frames
    if frame_count < 20:
        N̂ = λ·N̂ + (1-λ)·P
        return N̂
    
    # Spectral Flatness Measure
    SFM = geometric_mean(P) / arithmetic_mean(P)
    
    # SFM decision with hysteresis
    if SFM < SFM_speech:
        update = false          # tonal → speech
    elif SFM > SFM_noise:
        update = true           # flat → noise  
    else:
        update = previous_decision
    
    # Spike detection
    ratio = mean(P / N̂)
    is_spike = ratio > θ
    
    # Final decision
    if update AND NOT is_spike:
        N̂ = λ·N̂ + (1-λ)·P      # blend in new measurement
    
    return N̂
```

---

## 4. Gain Computation Detail

```
┌─────────────────────────────────────────────────────────────────────┐
│                       GAIN COMPUTATION                              │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────┐     ┌─────────────┐
│    P[k]     │     │    N̂[k]    │
│ input power │     │ noise est.  │
└──────┬──────┘     └──────┬──────┘
       │                   │
       └─────────┬─────────┘
                 │
                 ▼
┌─────────────────────────────────────────────┐
│ STEP A: Compute SNR per bin                 │
│                                             │
│   SNR[k] = 10 · log10( P[k] / N̂[k] )       │
│                                             │
│   Positive SNR = signal stronger than noise │
│   Negative SNR = noise dominates            │
│                                             │
│   bin 0   ███████████████  +15 dB (speech)  │
│   bin 50  ████████         +8 dB            │
│   bin 200 ███              +3 dB            │
│   bin 500 █                -2 dB (noise)    │
└─────────────────┬───────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ STEP B: Compute α per bin                   │
│                                             │
│   α[k] = α_base - SNR[k] / δ[k]            │
│   α[k] = clamp(α[k], α_min, α_max)         │
│                                             │
│   High SNR → low α (subtract less)          │
│   Low SNR  → high α (subtract more)         │
│                                             │
│   δ[k] controls sensitivity per band        │
│                                             │
│       α                                     │
│       ▲                                     │
│ α_max ┤ ────────╮                           │
│       │          ╲                          │
│ α_base┤           ╲                         │
│       │            ╲                        │
│ α_min ┤             ╲────────               │
│       └─────┴─────┴─────┴─────► SNR        │
│           -10    0    +10   +20            │
└─────────────────┬───────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ STEP C: Spectral Subtraction                │
│                                             │
│   subtracted[k] = P[k] - α[k]·N̂[k]         │
│                                             │
│   This can go NEGATIVE! (oversubtraction)   │
│                                             │
│         P[k]          │                     │
│   ┌─────────────┐     │   α·N̂[k]           │
│   │░░░░░░░░░░░░░│     │ ┌───────┐           │
│   │░░░░░░░░░░░░░│  -  │ │███████│           │
│   │░░░░░░░░░░░░░│     │ │███████│           │
│   └─────────────┘     │ └───────┘           │
│                                             │
│   = subtracted (might be negative)          │
└─────────────────┬───────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ STEP D: Apply Floor                         │
│                                             │
│   floor[k] = β · P[k]                       │
│                                             │
│   output_power[k] = max(subtracted[k],      │
│                         floor[k])           │
│                                             │
│   β = 0.05 means floor is 5% of input       │
│   Prevents complete silence                 │
│                                             │
│   If subtracted < 0:                        │
│       output = floor (β·P)                  │
│   If subtracted > floor:                    │
│       output = subtracted                   │
└─────────────────┬───────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ STEP E: Compute Gain                        │
│                                             │
│   G[k] = √( output_power[k] / P[k] )       │
│   G[k] = clamp(G[k], 0, 1)                 │
│                                             │
│   This is the multiplier for this bin       │
│                                             │
│   G = 1.0: no change (clean bin)            │
│   G = 0.5: reduce by 6 dB                   │
│   G = 0.1: reduce by 20 dB                  │
│   G ≈ √β: maximum reduction (floor)         │
└─────────────────────────────────────────────┘
```

**Pseudocode:**

```
function compute_gain(P, N̂, params):
    
    G = new array[n_bins]
    
    for each bin k:
        
        # Step A: SNR
        SNR = 10 * log10(P[k] / N̂[k])
        
        # Step B: Adaptive alpha
        δ = get_delta_for_bin(k)
        α = α_base - SNR / δ
        α = clamp(α, α_min, α_max)
        
        # Step C: Subtract
        subtracted = P[k] - α * N̂[k]
        
        # Step D: Floor
        floor = β * P[k]
        output_power = max(subtracted, floor)
        
        # Step E: Gain
        G[k] = sqrt(output_power / P[k])
        G[k] = clamp(G[k], 0, 1)
    
    return G
```

---

## 5. Overlap-Add Reconstruction

```
┌─────────────────────────────────────────────────────────────────────┐
│                    OVERLAP-ADD (WOLA)                               │
└─────────────────────────────────────────────────────────────────────┘

Why it works:
─────────────
Window w[n] = sin(π·n/N)

At 50% overlap, consecutive frames sum to unity:

    Frame m:      w²[n]           for n in [0, N)
    Frame m+1:    w²[n - hop]     for n in [hop, N+hop)
    
    In overlap region: w²[n] + w²[n-hop] = 1.0  ✓


Visual:
───────

Frame m:   ╭───────────────╮
           │               │
          ╱                 ╲
         ╱                   ╲
────────╱─────────────────────╲────────────────────────
        0                     N

Frame m+1:         ╭───────────────╮
                   │               │
                  ╱                 ╲
                 ╱                   ╲
────────────────╱─────────────────────╲────────────────
              hop                   N+hop

Sum:       ════════════════════════════
           Perfect reconstruction = 1.0


Buffer mechanics:
─────────────────

Time →

overlap_buffer (size N = 2048):
┌─────────────────────────────────────────────────────┐
│  ready to output  │  accumulating   │    zeros      │
│    (hop = 1024)   │    (1024)       │               │
└─────────────────────────────────────────────────────┘
         ▼
    output these
         │
         ▼
    then shift left by hop
         │
         ▼
┌─────────────────────────────────────────────────────┐
│   accumulating    │     zeros       │    zeros      │
└─────────────────────────────────────────────────────┘
         │
         │  add next processed frame
         ▼
┌─────────────────────────────────────────────────────┐
│ accum + new_start │   new_middle    │   new_end     │
└─────────────────────────────────────────────────────┘
```

**Pseudocode:**

```
# Initialization
overlap_buffer = zeros[window_size]

function overlap_add(processed_frame):
    
    # Add processed frame to buffer
    overlap_buffer += processed_frame
    
    # Extract output (first hop samples are ready)
    output = overlap_buffer[0 : hop_size]
    
    # Shift buffer left
    overlap_buffer = roll_left(overlap_buffer, hop_size)
    
    # Zero the end (where next frame will go)
    overlap_buffer[hop_size : end] = 0
    
    return output   # 1024 samples
```

---

## 6. Complete Algorithm Pseudocode

```
====================================================================
SPECTRAL SUBTRACTION DENOISER - COMPLETE ALGORITHM
====================================================================

INITIALIZATION:
    window_size = 2048
    hop_size = 1024
    n_bins = 1025
    
    window = sin(π · [0, 1, 2, ... N-1] / N)
    
    noise_estimate = zeros[n_bins]
    prev_gain = ones[n_bins]
    overlap_buffer = zeros[window_size]
    frame_count = 0
    prev_sfm_decision = true

--------------------------------------------------------------------

PROCESS_STREAM(input_samples):
    
    # Accumulate input until we have a full frame
    input_buffer.append(input_samples)
    
    while input_buffer.length >= window_size:
        
        # Extract frame
        frame = input_buffer[0 : window_size]
        input_buffer.remove_first(hop_size)
        
        # Process frame
        output_chunk = PROCESS_FRAME(frame)
        
        # Send to output
        emit(output_chunk)

--------------------------------------------------------------------

PROCESS_FRAME(frame):
    
    # === ANALYSIS ===
    
    # Window
    windowed = frame × window
    
    # FFT
    spectrum = FFT(windowed)
    power = |spectrum|²
    
    
    # === NOISE ESTIMATION ===
    
    if frame_count < 20:
        # Warmup: assume noise only
        noise_estimate = λ·noise_estimate + (1-λ)·power
        gain = ones[n_bins]
    
    else:
        # SFM calculation
        log_power = log(power + ε)
        geometric_mean = exp(mean(log_power))
        arithmetic_mean = mean(power)
        SFM = geometric_mean / arithmetic_mean
        
        # SFM decision
        if SFM < SFM_speech_threshold:
            should_update = false       # speech detected
        elif SFM > SFM_noise_threshold:
            should_update = true        # noise detected
        else:
            should_update = prev_sfm_decision   # hysteresis
        
        prev_sfm_decision = should_update
        
        # Spike detection
        power_ratio = mean(power / noise_estimate)
        is_spike = power_ratio > spike_threshold
        
        # Update noise estimate
        if should_update AND NOT is_spike:
            noise_estimate = λ·noise_estimate + (1-λ)·power
        
        
        # === GAIN CALCULATION ===
        
        for each bin k:
            # SNR
            SNR = 10 · log10(power[k] / noise_estimate[k])
            
            # Adaptive alpha
            α = α_base - SNR / δ[k]
            α = clamp(α, α_min, α_max)
            
            # Spectral subtraction
            subtracted = power[k] - α · noise_estimate[k]
            floor = β · power[k]
            clean_power = max(subtracted, floor)
            
            # Gain
            gain[k] = sqrt(clean_power / power[k])
            gain[k] = clamp(gain[k], 0, 1)
        
        
        # === GAIN SMOOTHING ===
        
        for each bin k:
            gain[k] = γ[k]·prev_gain[k] + (1-γ[k])·gain[k]
        
        prev_gain = gain
    
    frame_count += 1
    
    
    # === SYNTHESIS ===
    
    # Apply gain
    spectrum_out = spectrum × gain
    
    # IFFT + window
    frame_out = IFFT(spectrum_out) × window
    
    
    # === OVERLAP-ADD ===
    
    overlap_buffer += frame_out
    output = overlap_buffer[0 : hop_size]
    overlap_buffer = roll_left(overlap_buffer, hop_size)
    overlap_buffer[hop_size : end] = 0
    
    return output

====================================================================
```

---

## 7. Data Flow Summary

```
┌────────────────────────────────────────────────────────────────┐
│                     ONE FRAME JOURNEY                          │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│   2048 time samples                                            │
│        │                                                       │
│        ▼ window                                                │
│   2048 windowed samples                                        │
│        │                                                       │
│        ▼ FFT                                                   │
│   1025 complex bins ──────┬──────────────────────┐            │
│        │                  │                      │            │
│        ▼ |·|²             ▼                      │            │
│   1025 power values   1025 phase values          │            │
│        │                  │ (preserved)          │            │
│        │                  │                      │            │
│        ├──────► noise estimation                 │            │
│        │              │                          │            │
│        │              ▼                          │            │
│        │        1025 noise power                 │            │
│        │              │                          │            │
│        ├──────────────┤                          │            │
│        │              │                          │            │
│        ▼              ▼                          │            │
│   ┌─────────────────────────┐                    │            │
│   │    GAIN COMPUTATION     │                    │            │
│   │   SNR → α → subtract    │                    │            │
│   │   → floor → gain        │                    │            │
│   └───────────┬─────────────┘                    │            │
│               │                                  │            │
│               ▼                                  │            │
│        1025 gain values                          │            │
│               │                                  │            │
│               ▼ smooth                           │            │
│        1025 smoothed gains                       │            │
│               │                                  │            │
│               │◄─────────────────────────────────┘            │
│               │                                               │
│               ▼ multiply complex spectrum by gain             │
│        1025 filtered complex bins                             │
│               │                                               │
│               ▼ IFFT                                          │
│        2048 time samples                                      │
│               │                                               │
│               ▼ window                                        │
│        2048 windowed output                                   │
│               │                                               │
│               ▼ overlap-add                                   │
│        1024 output samples ──────► to output stream           │
│                                                               │
└────────────────────────────────────────────────────────────────┘
```

---

## 8. State Machine View

```
┌─────────────────────────────────────────────────────────────────────┐
│                        DENOISER STATES                              │
└─────────────────────────────────────────────────────────────────────┘


                        ┌──────────────┐
                        │    START     │
                        └───────┬──────┘
                                │
                                ▼
                        ┌──────────────┐
                ┌──────►│   WARMUP     │◄─────── frame_count < 20
                │       │              │
                │       │ • Always update noise
                │       │ • Gain = 1.0 (passthrough)
                │       └───────┬──────┘
                │               │
                │               │ frame_count >= 20
                │               ▼
         reset  │       ┌──────────────┐
                │       │   RUNNING    │
                │       │              │
                │       └───────┬──────┘
                │               │
                │               ▼
                │       ┌ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┐
                │         Per-frame decisions:
                │       │                               │
                │        ┌─────────────────────────────┐
                │       ││      SFM CLASSIFIER         ││
                │        │                             │
                │       ││  SFM < 0.1    SFM > 0.4    ││
                │        │      │             │        │
                │       ││      ▼             ▼       ││
                │        │  ┌───────┐   ┌─────────┐   │
                │       ││  │SPEECH │   │  NOISE  │  ││
                │        │  │freeze │   │ update? │   │
                │       ││  └───────┘   └────┬────┘  ││
                │        │                   │        │
                │       ││                   ▼       ││
                │        │              ┌─────────┐   │
                │       ││              │ SPIKE?  │  ││
                │        │              └────┬────┘   │
                │       ││          no ╱     │ yes  ││
                │        │            ╱      │        │
                │       ││           ▼       ▼      ││
                │        │     ┌────────┐ ┌────────┐ │
                │       ││     │ UPDATE │ │ FREEZE │││
                │        │     │   N̂   │ │   N̂   │ │
                │       ││     └────────┘ └────────┘││
                │        └─────────────────────────────┘
                │       └ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┘
                │
                └─────────────── on seek / reset
```