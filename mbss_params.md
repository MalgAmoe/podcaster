## Spectral Subtraction Parameters — Complete Guide

---

### The Core Equation

Everything flows from this:

```
G(k) = √[ max( |Y|² - α·N̂ , β·|Y|² ) / |Y|² ]
```

Where:

- `|Y|²` = power of current frame at bin k
- `N̂` = estimated noise power at bin k
- `α` = how much noise to subtract (oversubtraction factor)
- `β` = minimum floor (never go below this fraction of input)
- `G` = gain to apply (0 to 1)

---

## 1. Subtraction Parameters

### α_base (alpha base)

**What it is:** Starting point for noise subtraction multiplier.

**Range:** 0 – 20 (practical: 1 – 6)

**Effect:**

- `α = 1.0` → subtract exactly the estimated noise (theoretical ideal)
- `α > 1.0` → subtract MORE than estimated noise (oversubtraction)
- `α < 1.0` → subtract less than estimated noise (conservative)

**Why oversubtract?** Noise estimation is never perfect. Subtracting 1× leaves residual noise. Subtracting 2-4× removes more but risks eating into speech.

**Audible effect:**

|Value|Result|
|---|---|
|1.0|Noticeable residual noise, safe|
|2.0|Cleaner, minimal artifacts|
|3.0|Clean, slight artifacts possible|
|5.0+|Very clean, audible "musical noise" or speech damage|

---

### α_min / α_max (alpha clamp)

**What they are:** Hard limits on α after SNR-based adaptation.

**The adaptive formula:**

```
α(k) = clamp( α_base - SNR(k)/δ(k) , α_min , α_max )
```

**Logic:**

- High SNR (clean) → subtract less → α approaches α_min
- Low SNR (noisy) → subtract more → α approaches α_max

**α_min effect:**

- Sets minimum aggression even in clean regions
- Too high → unnecessary processing of clean audio
- Too low → might not do anything in clean regions (fine)

**α_max effect:**

- Caps maximum aggression in noisy regions
- Too high → destroys signal in noisy parts
- Too low → leaves noise in bad sections

**Interaction with α_base:**

- α_base is the starting point
- SNR shifts it up/down
- α_min/α_max are the guardrails

---

### β (beta / spectral floor)

**What it is:** Minimum gain floor as fraction of input power.

**Range:** 0 – 1

**The floor equation:**

```
output_power = max( subtracted_power, β × input_power )
```

**Effect:**

|Value|Result|
|---|---|
|0.0|Can go completely silent (vacuum effect)|
|0.02|Very quiet floor, aggressive|
|0.05|Slight residual, natural|
|0.15|Noticeable room tone preserved|
|1.0|No reduction at all|

**Why not 0?** Complete silence between words sounds unnatural. A small floor maintains "air" and masks musical noise artifacts.

**Interaction with α:**

- High α + low β = maximum removal, maximum artifacts
- High α + high β = α works hard but floor saves you
- Low α + low β = gentle overall

**Key insight:** β is your safety net. When in doubt, raise β before lowering α.

---

### δ (delta / per-band subtraction sensitivity)

**What it is:** Controls how much SNR affects α in each frequency band.

**Range:** 0.1 – 5 (per band)

**The formula:**

```
α(k) = α_base - SNR(k) / δ(k)
```

**Effect:**

- **Low δ (0.5):** Small SNR changes → big α changes → aggressive adaptation
- **High δ (3.0):** SNR changes have less effect → α stays near α_base → conservative

**Think of it as:** "How sensitive is this band to SNR?"

**Per-band logic:**

|Band|Suggested δ|Why|
|---|---|---|
|0-80 Hz|0.8|Rumble can be speech-correlated, be careful|
|80-250 Hz|1.0|Fundamental — protect at all costs|
|250-500 Hz|1.5|Some removal OK|
|500-2k Hz|2.0-2.5|Formants — standard processing|
|2-4k Hz|2.5|Intelligibility — don't destroy|
|4-8k Hz|2.0|Sibilance — can remove more|
|8-12k Hz|1.5|Air — more removal OK|
|12-24k Hz|1.0|Mostly hiss — hit it hard|

**Interaction:**

- Low δ + high α_max = very aggressive in that band
- High δ + low α_base = almost no adaptation, constant subtraction

---

## 2. Noise Estimation Parameters

### λ (lambda / forget factor)

**What it is:** Smoothing coefficient for noise estimate updates.

**Range:** 0 – 0.999

**The update equation:**

```
N̂_new = λ · N̂_old + (1-λ) · current_power
```

**Effect:**

|Value|Time constant|Behavior|
|---|---|---|
|0.9|~10 frames|Fast tracking, follows noise changes quickly|
|0.95|~20 frames|Moderate (default)|
|0.99|~100 frames|Slow, very stable estimate|
|0.999|~1000 frames|Almost frozen|

**Time constant ≈** `1 / (1-λ)` frames

**Tradeoffs:**

- **High λ (slow):** Stable estimate, but can't track changing noise (e.g., AC turning on)
- **Low λ (fast):** Tracks changes, but speech can leak into noise estimate

**Interaction with SFM thresholds:** λ only matters when noise IS being updated. SFM decides WHEN to update.

---

### Spike θ (theta / spike threshold)

**What it is:** Power ratio threshold for detecting sudden transients.

**Range:** 1 – 100 (ratio, not dB)

**The check:**

```
mean(current_power / noise_power) > θ → spike detected → freeze update
```

**Effect:**

|Value|Result|
|---|---|
|3|Very sensitive, freezes often|
|10|Catches speech onsets (default)|
|50|Only catches very loud transients|
|100|Almost never triggers|

**Why it matters:** Prevents speech transients (plosives, loud syllables) from being interpreted as "noise changed" and corrupting the estimate.

**Interaction with λ:**

- Low θ + high λ = very stable estimate (spike blocks updates, λ slows them)
- High θ + low λ = fast-moving estimate (nothing blocks, updates happen often)

---

### SFM_speech / SFM_noise (Spectral Flatness thresholds)

**What SFM is:**

```
SFM = geometric_mean(power) / arithmetic_mean(power)
```

- SFM → 0: Tonal (peaks, speech harmonics)
- SFM → 1: Flat (white noise)

**SFM_speech (lower threshold):**

- Below this → definitely speech → **freeze** noise estimate
- Default: 0.1

**SFM_noise (upper threshold):**

- Above this → definitely noise → **update** noise estimate
- Default: 0.4

**Between thresholds:** Hysteresis — keeps previous decision.

**Effect:**

|SFM_speech|SFM_noise|Behavior|
|---|---|---|
|0.1|0.4|Default — conservative|
|0.2|0.5|More likely to update during speech|
|0.05|0.3|Very protective of speech|
|0.3|0.6|Aggressive updating|

**Failure modes:**

- **Thresholds too close:** Unstable, flips constantly
- **SFM_speech too high:** Noise estimate corrupted by speech
- **SFM_noise too low:** Updates during speech pauses that aren't pure noise

**Interaction with λ and θ:** SFM decides "should we update?" → θ checks "is it a spike?" → if both pass, λ controls update speed.

---

## 3. Gain Smoothing Parameters

### γ (gamma / per-band temporal smoothing)

**What it is:** IIR smoothing coefficient for the gain curve.

**Range:** 0 – 0.99 (per band)

**The smoothing:**

```
Ĝ_new = γ · Ĝ_old + (1-γ) · G_current
```

**Effect:**

|Value|Result|
|---|---|
|0.0|No smoothing — raw gain, maximum musical noise|
|0.5|Light smoothing — some flutter|
|0.8|Moderate — good balance|
|0.95|Heavy — very smooth, slow transient response|
|0.99|Almost frozen — gain barely moves|

**Why per-band?**

|Band|Suggested γ|Reason|
|---|---|---|
|Low (< 250 Hz)|0.5-0.6|Preserve punch, transients|
|Mid (250-2k Hz)|0.7-0.85|Balance|
|High (> 2k Hz)|0.9+|Smooth out hiss artifacts|

**Interaction with β:**

- Low γ + low β = maximum artifact risk (jumpy gain, can hit zero)
- High γ + high β = safest but least removal
- High γ smooths the gain, β catches what slips through

---

## 4. Parameter Interaction Map

```
                    ┌─────────────┐
                    │ Input Frame │
                    └──────┬──────┘
                           │
                           ▼
┌──────────────────────────────────────────┐
│           NOISE ESTIMATION               │
│  ┌─────┐   ┌─────┐   ┌─────┐            │
│  │ SFM │ → │Spike│ → │  λ  │ → N̂        │
│  │thrsh│   │  θ  │   │     │            │
│  └─────┘   └─────┘   └─────┘            │
│                                          │
│  SFM decides IF, θ gates spikes, λ HOW  │
└──────────────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────┐
│           GAIN CALCULATION               │
│                                          │
│  SNR = 10·log(|Y|²/N̂)                   │
│            │                             │
│            ▼                             │
│  ┌────┐  ┌────┐  ┌────┐                 │
│  │α_b │─→│ δ  │─→│α_mm│ → α(k)          │
│  └────┘  └────┘  └────┘                 │
│            │                             │
│            ▼                             │
│  G = √max(|Y|²-α·N̂, β·|Y|²)/|Y|²       │
│                      ↑                   │
│                      β (safety floor)    │
└──────────────────────────────────────────┘
                           │
                           ▼
┌──────────────────────────────────────────┐
│           GAIN SMOOTHING                 │
│  ┌─────┐                                 │
│  │  γ  │ → Ĝ = γ·Ĝ_prev + (1-γ)·G       │
│  └─────┘                                 │
└──────────────────────────────────────────┘
                           │
                           ▼
                    ┌─────────────┐
                    │ Output = Y·Ĝ│
                    └─────────────┘
```

---

## 5. Tuning Recipes

**"Too much musical noise / birdies"** → Increase γ (more smoothing) → Increase β (higher floor)

**"Not removing enough noise"** → Increase α_base → Decrease β → Decrease δ in problem bands

**"Speech sounds watery/phasey"** → Decrease α_base → Increase β → Increase δ in speech bands (80-4k Hz)

**"Pumping/breathing effect"** → Increase β (raise the floor) → Increase γ_low (smooth low-freq gain)

**"Noise estimate corrupted by speech"** → Lower SFM_speech threshold → Lower spike θ → Increase λ (slower updates)

**"Noise floor changes aren't tracked"** → Decrease λ → Raise SFM_noise threshold

**"Hiss not removed enough"** → Lower δ in 8-24k bands → Increase α_max → Decrease β

**"Losing vocal presence"** → Increase δ in 1-4k Hz bands → Lower γ in those bands → Raise β

---

## 6. Quick Reference Card

|Parameter|Default|Turn UP to...|Turn DOWN to...|
|---|---|---|---|
|α_base|3.0|Remove more noise|Preserve signal|
|α_min|1.0|Minimum processing floor|Let clean parts through|
|α_max|5.0|Allow aggressive removal|Cap maximum damage|
|β|0.05|Keep room tone, safety|Deeper noise removal|
|λ|0.95|Stable estimate|Track changing noise|
|θ|10|Ignore transients|Protect speech onsets|
|SFM_sp|0.1|Update more during speech|Protect speech|
|SFM_ns|0.4|Require flatter spectrum|Update sooner|
|δ[band]|varies|Less adaptation (stable α)|More aggressive adaptation|
|γ[band]|varies|Smoother gain (less artifacts)|Faster response|
