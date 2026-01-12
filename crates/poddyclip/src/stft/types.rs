//! Shared STFT constants and types
//!
//! Two window sizes are used intentionally:
//! - Real-time (2048): Lower latency for processors
//! - Analysis (4096): Better frequency resolution for analysis

/// Real-time window size (2048 samples at 48kHz = 42.7ms)
/// - Frequency resolution: 23.4 Hz/bin at 48kHz
/// - Latency: 1024 samples lookahead
/// - Used by: denoiser, dereverb, spectral_gate, peak_attenuator
pub const RT_WINDOW_SIZE: usize = 2048;

/// Real-time hop size (50% overlap)
pub const RT_HOP_SIZE: usize = 1024;

/// Number of FFT bins for real-time processing
pub const RT_N_BINS: usize = RT_WINDOW_SIZE / 2 + 1; // 1025

/// Analysis window size (4096 samples at 48kHz = 85.3ms)
/// - Frequency resolution: 11.7 Hz/bin at 48kHz
/// - Better for detecting narrow peaks, spectral features
/// - Used by: spectral analysis, sibilance detection, EQ fitting
pub const ANALYSIS_WINDOW_SIZE: usize = 4096;

/// Analysis hop size (50% overlap)
pub const ANALYSIS_HOP_SIZE: usize = 2048;

/// Number of FFT bins for analysis
pub const ANALYSIS_N_BINS: usize = ANALYSIS_WINDOW_SIZE / 2 + 1; // 2049

/// Small value to prevent division by zero
pub const EPSILON: f32 = 1e-10;

/// Configuration for STFT processor
#[derive(Clone, Debug)]
pub struct StftConfig {
    pub window_size: usize,
    pub hop_size: usize,
    pub window_type: WindowType,
}

impl StftConfig {
    /// Real-time processing config (2048/1024)
    pub fn realtime() -> Self {
        Self {
            window_size: RT_WINDOW_SIZE,
            hop_size: RT_HOP_SIZE,
            window_type: WindowType::SqrtHann,
        }
    }

    /// Analysis config (4096/2048)
    pub fn analysis() -> Self {
        Self {
            window_size: ANALYSIS_WINDOW_SIZE,
            hop_size: ANALYSIS_HOP_SIZE,
            window_type: WindowType::Hann,
        }
    }

    /// Number of FFT bins
    pub fn n_bins(&self) -> usize {
        self.window_size / 2 + 1
    }
}

/// Window function type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowType {
    /// Regular Hann window: 0.5 * (1 - cos(2*PI*i/(n-1)))
    /// Use for analysis where overlap-add reconstruction is not needed
    Hann,
    /// Square root of Hann window: sin(PI*i/n)
    /// Use for 50% overlap-add reconstruction (synthesis)
    SqrtHann,
}
