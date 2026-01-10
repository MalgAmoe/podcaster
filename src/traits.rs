//! Buffer-based audio processor traits
//!
//! Primary abstraction uses `process_buffer(&mut [f32])` for better cache locality,
//! prefetcher optimization, and SIMD potential (285% performance improvement).

// Traits define API contract - not all methods used by all features
#![allow(dead_code)]

// =============================================================================
// Core Traits
// =============================================================================

/// Primary trait for buffer-based audio processing.
/// All processors should implement this as their main interface.
pub trait AudioProcessor: Send {
    /// Process a mono buffer in-place
    fn process_buffer(&mut self, buffer: &mut [f32]);

    /// Reset all internal state (filters, envelopes, delay lines)
    fn reset(&mut self);

    /// Report latency in samples (for lookahead processors)
    fn latency_samples(&self) -> usize {
        0
    }
}

/// Constructor trait for processors with simple `new(sample_rate: f32)` signature.
/// Use this with `Stereo<P>::new(sample_rate)`.
pub trait Processor: AudioProcessor + Sized {
    fn new(sample_rate: f32) -> Self;
}

/// Constructor trait for processors using f64 sample rate (tape processors).
/// Use this with `Stereo<P>::new_f64(sample_rate)`.
pub trait ProcessorF64: AudioProcessor + Sized {
    fn new(sample_rate: f64) -> Self;
}

/// Marker trait for processors that can be used with `Stereo<P>` wrapper.
/// Processors implementing this have independent per-channel processing
/// (no stereo linking, no shared state between channels).
pub trait MonoProcessor: AudioProcessor {}

/// Trait for stereo processors with linked behavior.
/// Use this for processors that need shared gain reduction, linked peak detection, etc.
/// Also used for dual-mono processors wrapped with `Stereo<P>`.
pub trait StereoProcessor: Send {
    /// Process stereo buffers in-place
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]);

    /// Process mono buffer (default: duplicate to both channels, process, return left)
    /// Override for linked processors that need special mono handling.
    fn process_mono(&mut self, buffer: &mut [f32]) {
        // Default implementation: process as stereo with identical L/R
        // This works for both linked and dual-mono processors
        let mut right = buffer.to_vec();
        self.process_stereo(buffer, &mut right);
        // Result is in buffer (left channel)
    }

    /// Reset all internal state
    fn reset(&mut self);

    /// Report latency in samples
    fn latency_samples(&self) -> usize {
        0
    }
}

/// Trait for FFT frame-based processors (denoisers).
/// These operate on fixed-size frames, not arbitrary buffers.
pub trait FrameProcessor: Send {
    /// Process a complete frame of samples.
    /// Input must be `window_size()` samples.
    /// Returns `hop_size()` output samples.
    fn process_frame(&mut self, frame: &[f32]) -> Vec<f32>;

    /// Reset all internal state
    fn reset(&mut self);

    /// FFT window size in samples
    fn window_size(&self) -> usize;

    /// Hop size (overlap) in samples
    fn hop_size(&self) -> usize;

    /// Total latency in samples
    fn latency_samples(&self) -> usize;
}

// =============================================================================
// Generic Stereo Wrapper
// =============================================================================

/// Generic stereo wrapper for mono processors.
/// Works with any processor implementing `AudioProcessor`.
#[derive(Clone, Debug)]
pub struct Stereo<P> {
    pub left: P,
    pub right: P,
}

// Constructor for f32 sample rate processors
impl<P: Processor + Clone> Stereo<P> {
    pub fn new(sample_rate: f32) -> Self {
        let p = P::new(sample_rate);
        Self {
            left: p.clone(),
            right: p,
        }
    }
}

// Constructor for f64 sample rate processors (TapeHysteresis, TapeGlue)
impl<P: ProcessorF64 + Clone> Stereo<P> {
    pub fn new_f64(sample_rate: f64) -> Self {
        let p = P::new(sample_rate);
        Self {
            left: p.clone(),
            right: p,
        }
    }
}

// Factory methods that work with any P
impl<P: Clone> Stereo<P> {
    /// Create stereo from a single mono processor (clones it)
    #[allow(dead_code)]
    pub fn from_mono(processor: P) -> Self {
        Self {
            left: processor.clone(),
            right: processor,
        }
    }
}

impl<P> Stereo<P> {
    /// Create stereo from two separate processor instances
    pub fn from_pair(left: P, right: P) -> Self {
        Self { left, right }
    }

    /// Apply a function to both channels (for parameter updates)
    pub fn set_both<F: FnMut(&mut P)>(&mut self, mut f: F) {
        f(&mut self.left);
        f(&mut self.right);
    }
}

// Buffer-based processing for AudioProcessor
impl<P: AudioProcessor> Stereo<P> {
    /// Process stereo buffers independently (no linking)
    pub fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.left.process_buffer(left);
        self.right.process_buffer(right);
    }

    /// Process mono buffer (left channel only)
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        self.left.process_buffer(buffer);
    }

    /// Reset both channels
    pub fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }

    /// Get latency (assumes both channels have same latency)
    pub fn latency_samples(&self) -> usize {
        self.left.latency_samples()
    }
}

// Implement StereoProcessor for Stereo<P> where P is a mono processor
impl<P: MonoProcessor> StereoProcessor for Stereo<P> {
    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.left.process_buffer(left);
        self.right.process_buffer(right);
    }

    /// Efficient mono processing: just use left processor directly
    fn process_mono(&mut self, buffer: &mut [f32]) {
        self.left.process_buffer(buffer);
    }

    fn reset(&mut self) {
        self.left.reset();
        self.right.reset();
    }

    fn latency_samples(&self) -> usize {
        self.left.latency_samples()
    }
}

