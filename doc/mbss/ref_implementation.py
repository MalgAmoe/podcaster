"""
Spectral Subtraction Noise Reduction - Python Validation
Minimal dependencies to mirror future Rust implementation.

Dependencies: numpy, scipy (for wav I/O only)
"""

import numpy as np
from scipy.io import wavfile
import argparse
from pathlib import Path


# =============================================================================
# Constants (from spec)
# =============================================================================

WINDOW_SIZE = 2048
HOP_SIZE = 1024
SAMPLE_RATE = 48000

# Noise estimation
LAMBDA = 0.95  # forget factor
SPIKE_THRESHOLD = 10.0  # 10 dB power spike
WARMUP_FRAMES = 20

# SFM thresholds (empirically tuned for finite FFT windows)
SFM_SPEECH = 0.1  # below = tonal (freeze)
SFM_NOISE = 0.4   # above = flat (update)

# 9-Band configuration for adaptive alpha
# (start_hz, end_hz, delta) — lower δ = more aggressive subtraction
BANDS = [
    (0, 80, 0.8),        # Band 0: Rumble, pops, HVAC
    (80, 250, 1.0),      # Band 1: Voice fundamental
    (250, 500, 1.5),     # Band 2: Warmth
    (500, 1000, 2.0),    # Band 3: Body
    (1000, 2000, 2.5),   # Band 4: Presence
    (2000, 4000, 2.5),   # Band 5: Intelligibility
    (4000, 8000, 2.0),   # Band 6: Sibilance
    (8000, 12000, 1.5),  # Band 7: Air
    (12000, 24000, 1.0), # Band 8: Hiss region
]

TRANSITION_BINS = 10  # Narrower transitions for more bands
EPSILON = 1e-10


# =============================================================================
# Preset Modes (1-5: Gentle to Aggressive)
# =============================================================================

# Gamma values per band (9 bands: rumble, fundamental, warmth, body, presence, intelligibility, sibilance, air, hiss)
PRESETS = {
    1: {  # Gentle - minimal processing, preserve everything
        "name": "Gentle",
        "alpha_base": 1.5,
        "alpha_min": 0.5,
        "alpha_max": 2.5,
        "beta": 0.15,
        "gamma": [0.40, 0.45, 0.50, 0.55, 0.60, 0.65, 0.70, 0.75, 0.80],
    },
    2: {  # Light - subtle noise reduction
        "name": "Light",
        "alpha_base": 2.0,
        "alpha_min": 0.8,
        "alpha_max": 3.5,
        "beta": 0.08,
        "gamma": [0.45, 0.50, 0.58, 0.65, 0.72, 0.78, 0.82, 0.86, 0.88],
    },
    3: {  # Moderate - balanced (default)
        "name": "Moderate",
        "alpha_base": 3.0,
        "alpha_min": 1.0,
        "alpha_max": 5.0,
        "beta": 0.05,
        "gamma": [0.50, 0.55, 0.65, 0.72, 0.78, 0.82, 0.86, 0.90, 0.92],
    },
    4: {  # Strong - noticeable noise reduction
        "name": "Strong",
        "alpha_base": 4.5,
        "alpha_min": 1.5,
        "alpha_max": 7.0,
        "beta": 0.02,
        "gamma": [0.55, 0.60, 0.70, 0.78, 0.84, 0.88, 0.91, 0.94, 0.96],
    },
    5: {  # Aggressive - maximum removal, may affect speech
        "name": "Aggressive",
        "alpha_base": 6.0,
        "alpha_min": 2.0,
        "alpha_max": 10.0,
        "beta": 0.008,
        "gamma": [0.60, 0.65, 0.75, 0.82, 0.88, 0.92, 0.94, 0.96, 0.98],
    },
}

# Default preset
DEFAULT_PRESET = 3


# =============================================================================
# Window Functions
# =============================================================================

def root_hann_window(n: int) -> np.ndarray:
    """Root-Hann (sine) window for WOLA."""
    return np.sin(np.pi * np.arange(n) / n)


# =============================================================================
# Spectral Flatness Measure
# =============================================================================

def compute_sfm(power_spectrum: np.ndarray) -> float:
    """
    Compute Spectral Flatness Measure.
    SFM ≈ 1.0 -> flat (noise)
    SFM ≈ 0.0 -> tonal (speech)
    """
    p = power_spectrum + EPSILON
    geo_mean = np.exp(np.mean(np.log(p)))
    arith_mean = np.mean(p)
    return geo_mean / (arith_mean + EPSILON)


# =============================================================================
# Multi-band Alpha Calculation
# =============================================================================

def hz_to_bin(hz: float, fft_size: int, sample_rate: int) -> int:
    """Convert frequency in Hz to FFT bin index."""
    return int(hz * fft_size / sample_rate)


def compute_alpha_curve(fft_size: int, sample_rate: int, snr_per_bin: np.ndarray,
                        alpha_base: float, alpha_min: float, alpha_max: float) -> np.ndarray:
    """
    Compute frequency-dependent alpha with cross-band smoothing.
    """
    n_bins = fft_size // 2 + 1
    alpha = np.zeros(n_bins)
    delta = np.zeros(n_bins)
    
    # Assign delta per band
    for start_hz, end_hz, d in BANDS:
        start_bin = hz_to_bin(start_hz, fft_size, sample_rate)
        end_bin = min(hz_to_bin(end_hz, fft_size, sample_rate), n_bins)
        delta[start_bin:end_bin] = d
    
    # Fill any gaps with nearest value
    delta[delta == 0] = BANDS[-1][2]
    
    # Compute alpha per bin: α = clamp(α₀ - SNR/δ, min, max)
    alpha = np.clip(alpha_base - snr_per_bin / (delta + EPSILON), alpha_min, alpha_max)
    
    # Apply raised-cosine smoothing at band transitions
    for i, (start_hz, end_hz, _) in enumerate(BANDS[:-1]):
        transition_bin = hz_to_bin(end_hz, fft_size, sample_rate)
        half_width = TRANSITION_BINS // 2
        
        for k in range(max(0, transition_bin - half_width), 
                       min(n_bins, transition_bin + half_width)):
            t = (k - (transition_bin - half_width)) / TRANSITION_BINS
            w = 0.5 * (1 - np.cos(np.pi * t))
            # Blend between current and next band's alpha
            if k > 0 and k < n_bins - 1:
                alpha[k] = (1 - w) * alpha[k-1] + w * alpha[k+1] if k > transition_bin else alpha[k]
    
    return alpha


def compute_gamma_curve(fft_size: int, sample_rate: int, gamma_per_band: list) -> np.ndarray:
    """Compute frequency-dependent gain smoothing factor using 9 bands."""
    n_bins = fft_size // 2 + 1
    gamma = np.zeros(n_bins)
    
    # Assign gamma per band
    for i, (start_hz, end_hz, _) in enumerate(BANDS):
        start_bin = hz_to_bin(start_hz, fft_size, sample_rate)
        end_bin = min(hz_to_bin(end_hz, fft_size, sample_rate), n_bins)
        gamma[start_bin:end_bin] = gamma_per_band[i]
    
    # Fill any gaps with last band's value
    gamma[gamma == 0] = gamma_per_band[-1]
    
    # Smooth transitions between bands
    for i in range(len(BANDS) - 1):
        transition_bin = hz_to_bin(BANDS[i][1], fft_size, sample_rate)
        half_width = TRANSITION_BINS // 2
        
        for k in range(max(0, transition_bin - half_width),
                       min(n_bins, transition_bin + half_width)):
            t = (k - (transition_bin - half_width)) / TRANSITION_BINS
            w = 0.5 * (1 - np.cos(np.pi * t))
            gamma[k] = (1 - w) * gamma_per_band[i] + w * gamma_per_band[i + 1]
    
    return gamma


# =============================================================================
# Core Denoiser State
# =============================================================================

class SpectralSubtractionDenoiser:
    def __init__(self, sample_rate: int = SAMPLE_RATE, window_size: int = WINDOW_SIZE,
                 preset: int = DEFAULT_PRESET):
        self.sample_rate = sample_rate
        self.window_size = window_size
        self.hop_size = window_size // 2
        self.n_bins = window_size // 2 + 1
        
        # Load preset parameters
        if preset not in PRESETS:
            raise ValueError(f"Invalid preset {preset}. Must be 1-5.")
        p = PRESETS[preset]
        self.preset_name = p["name"]
        self.alpha_base = p["alpha_base"]
        self.alpha_min = p["alpha_min"]
        self.alpha_max = p["alpha_max"]
        self.beta = p["beta"]
        
        # Windows
        self.window = root_hann_window(window_size)
        
        # State buffers
        self.noise_pow = np.zeros(self.n_bins)
        self.prev_gain = np.ones(self.n_bins)  # Initialize to unity
        self.prev_sfm_decision = True  # True = update noise
        self.frame_count = 0
        
        # Precompute gamma curve with preset values
        self.gamma = compute_gamma_curve(window_size, sample_rate, p["gamma"])
        
        # Output overlap buffer
        self.overlap_buffer = np.zeros(window_size)
        
        # Input buffer for accumulating samples
        self.input_buffer = np.zeros(0)
    
    def _compute_snr_per_bin(self, power: np.ndarray) -> np.ndarray:
        """Compute SNR in dB per frequency bin."""
        snr_linear = power / (self.noise_pow + EPSILON)
        return 10 * np.log10(snr_linear + EPSILON)
    
    def _update_noise_estimate(self, power: np.ndarray, force_update: bool = False):
        """Update noise estimate with spike protection."""
        for k in range(self.n_bins):
            # Spike protection: skip if signal is 10 dB above noise
            if not force_update and power[k] > SPIKE_THRESHOLD * self.noise_pow[k]:
                continue
            # Recursive update
            self.noise_pow[k] = LAMBDA * self.noise_pow[k] + (1 - LAMBDA) * power[k]
    
    def _compute_gain(self, power: np.ndarray, alpha: np.ndarray) -> np.ndarray:
        """Compute Berouti spectral subtraction gain."""
        # G(k) = sqrt(max(|Y|² - α·N̂, β·|Y|²) / |Y|²)
        subtracted = power - alpha * self.noise_pow
        floored = self.beta * power
        numerator = np.maximum(subtracted, floored)
        gain = np.sqrt(numerator / (power + EPSILON))
        return np.clip(gain, 0.0, 1.0)
    
    def _smooth_gain(self, gain: np.ndarray) -> np.ndarray:
        """Apply frequency-dependent temporal smoothing."""
        smoothed = self.gamma * self.prev_gain + (1 - self.gamma) * gain
        self.prev_gain = smoothed.copy()
        return smoothed
    
    def process_frame(self, frame: np.ndarray) -> np.ndarray:
        """Process a single frame of audio."""
        assert len(frame) == self.window_size
        
        # Analysis window
        windowed = frame * self.window
        
        # FFT
        spectrum = np.fft.rfft(windowed)
        power = np.abs(spectrum) ** 2
        
        # Warmup: collect noise estimate but pass signal through
        if self.frame_count < WARMUP_FRAMES:
            self._update_noise_estimate(power, force_update=True)
            self.frame_count += 1
            # Pass-through: apply synthesis window only
            enhanced = np.fft.irfft(spectrum)
        else:
            # SFM-based VAD
            sfm = compute_sfm(power)
            
            if sfm > SFM_NOISE:
                # Flat spectrum = noise, update estimate
                self._update_noise_estimate(power)
                self.prev_sfm_decision = True
            elif sfm < SFM_SPEECH:
                # Tonal = speech, freeze estimate
                self.prev_sfm_decision = False
            # else: hysteresis, keep previous decision
            elif self.prev_sfm_decision:
                self._update_noise_estimate(power)
            
            # Compute adaptive alpha
            snr = self._compute_snr_per_bin(power)
            alpha = compute_alpha_curve(self.window_size, self.sample_rate, snr,
                                        self.alpha_base, self.alpha_min, self.alpha_max)
            
            # Compute and smooth gain
            gain = self._compute_gain(power, alpha)
            gain = self._smooth_gain(gain)
            
            # Apply gain
            enhanced_spectrum = gain * spectrum
            
            # IFFT
            enhanced = np.fft.irfft(enhanced_spectrum)
        
        # Synthesis window
        enhanced = enhanced * self.window
        
        # Overlap-add: add current frame to buffer, output first hop_size samples
        self.overlap_buffer += enhanced
        output = self.overlap_buffer[:self.hop_size].copy()
        
        # Shift buffer
        self.overlap_buffer = np.roll(self.overlap_buffer, -self.hop_size)
        self.overlap_buffer[-self.hop_size:] = 0
        
        self.frame_count += 1
        return output
    
    def process(self, audio: np.ndarray) -> np.ndarray:
        """Process entire audio signal."""
        original_len = len(audio)
        
        # Pad to multiple of hop size
        pad_len = (self.hop_size - len(audio) % self.hop_size) % self.hop_size
        audio = np.pad(audio, (0, pad_len))
        
        # Pre-pad for first window (so first real sample aligns correctly)
        pre_pad = self.window_size - self.hop_size
        audio = np.pad(audio, (pre_pad, 0))
        
        output = []
        
        # Process frame by frame
        for i in range(0, len(audio) - self.window_size + 1, self.hop_size):
            frame = audio[i:i + self.window_size]
            out_frame = self.process_frame(frame)
            output.append(out_frame)
        
        result = np.concatenate(output)
        
        # Remove pre-padding from output and trim to original length
        result = result[pre_pad:]
        result = result[:original_len]
        
        return result
    
    def reset(self):
        """Reset state for new audio."""
        self.noise_pow = np.zeros(self.n_bins)
        self.prev_gain = np.ones(self.n_bins)
        self.prev_sfm_decision = True
        self.frame_count = 0
        self.overlap_buffer = np.zeros(self.window_size)
        self.input_buffer = np.zeros(0)


# =============================================================================
# Stereo Processing (M/S)
# =============================================================================

def process_stereo(left: np.ndarray, right: np.ndarray, sample_rate: int,
                   preset: int = DEFAULT_PRESET) -> tuple:
    """Process stereo audio using M/S matrix."""
    # Convert to M/S
    mid = (left + right) / 2
    side = (left - right) / 2
    
    # Process each channel independently
    denoiser_mid = SpectralSubtractionDenoiser(sample_rate, preset=preset)
    denoiser_side = SpectralSubtractionDenoiser(sample_rate, preset=preset)
    
    mid_processed = denoiser_mid.process(mid)
    side_processed = denoiser_side.process(side)
    
    # Ensure same length
    min_len = min(len(mid_processed), len(side_processed))
    mid_processed = mid_processed[:min_len]
    side_processed = side_processed[:min_len]
    
    # Convert back to L/R
    left_out = mid_processed + side_processed
    right_out = mid_processed - side_processed
    
    return left_out, right_out


# =============================================================================
# Level Matching
# =============================================================================

def match_rms(input_audio: np.ndarray, output_audio: np.ndarray) -> np.ndarray:
    """Match output RMS to input RMS."""
    rms_in = np.sqrt(np.mean(input_audio ** 2) + EPSILON)
    rms_out = np.sqrt(np.mean(output_audio ** 2) + EPSILON)
    gain = rms_in / rms_out
    return output_audio * gain


# =============================================================================
# File I/O
# =============================================================================

def load_wav(path: str) -> tuple:
    """Load WAV file, return (audio, sample_rate, is_stereo)."""
    sr, audio = wavfile.read(path)
    
    # Convert to float32 [-1, 1]
    if audio.dtype == np.int16:
        audio = audio.astype(np.float32) / 32768.0
    elif audio.dtype == np.int32:
        audio = audio.astype(np.float32) / 2147483648.0
    elif audio.dtype == np.float32:
        pass
    else:
        audio = audio.astype(np.float32)
    
    is_stereo = len(audio.shape) > 1 and audio.shape[1] == 2
    return audio, sr, is_stereo


def save_wav(path: str, audio: np.ndarray, sample_rate: int):
    """Save audio to WAV file."""
    # Clip and convert to int16
    audio = np.clip(audio, -1.0, 1.0)
    audio_int = (audio * 32767).astype(np.int16)
    wavfile.write(path, sample_rate, audio_int)


# =============================================================================
# Main
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Spectral Subtraction Denoiser",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Preset Modes:
  1 = Gentle     - Minimal processing, preserves everything
  2 = Light      - Subtle noise reduction  
  3 = Moderate   - Balanced (default)
  4 = Strong     - Noticeable noise reduction
  5 = Aggressive - Maximum removal, may affect speech quality
        """
    )
    parser.add_argument("input", help="Input WAV file")
    parser.add_argument("-o", "--output", help="Output WAV file", default=None)
    parser.add_argument("-p", "--preset", type=int, choices=[1, 2, 3, 4, 5], default=3,
                        help="Denoising strength 1-5 (default: 3)")
    parser.add_argument("--no-level-match", action="store_true", help="Disable RMS matching")
    parser.add_argument("--all-presets", action="store_true", 
                        help="Generate output for all 5 presets")
    args = parser.parse_args()
    
    input_path = Path(args.input)
    
    print(f"Loading: {input_path}")
    audio, sr, is_stereo = load_wav(str(input_path))
    
    print(f"Sample rate: {sr} Hz")
    print(f"Channels: {'stereo' if is_stereo else 'mono'}")
    print(f"Duration: {len(audio) / sr:.2f}s")
    
    # Resample warning
    if sr != SAMPLE_RATE:
        print(f"WARNING: Sample rate {sr} != {SAMPLE_RATE}. Results may vary.")
    
    if args.all_presets:
        # Generate all 5 versions
        presets_to_run = [1, 2, 3, 4, 5]
    else:
        presets_to_run = [args.preset]
    
    for preset in presets_to_run:
        preset_name = PRESETS[preset]["name"].lower()
        
        if args.output and not args.all_presets:
            output_path = args.output
        else:
            output_path = f"{input_path.stem}_denoised_{preset}_{preset_name}.wav"
        
        print(f"\nProcessing with preset {preset} ({PRESETS[preset]['name']})...")
        
        if is_stereo:
            left, right = audio[:, 0].copy(), audio[:, 1].copy()
            left_out, right_out = process_stereo(left, right, sr, preset=preset)
            
            if not args.no_level_match:
                left_out = match_rms(left[:len(left_out)], left_out)
                right_out = match_rms(right[:len(right_out)], right_out)
            
            output = np.column_stack([left_out, right_out])
        else:
            denoiser = SpectralSubtractionDenoiser(sr, preset=preset)
            output = denoiser.process(audio.copy())
            
            if not args.no_level_match:
                output = match_rms(audio[:len(output)], output)
        
        print(f"Saving: {output_path}")
        save_wav(output_path, output, sr)
    
    print("\nDone.")


if __name__ == "__main__":
    main()