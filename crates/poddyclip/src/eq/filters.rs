//! Common filter implementations shared between CLI and plugin
//! Uses State Variable Filters (SVF) for numerical stability

use std::f32::consts::PI;

#[cfg(feature = "plugin")]
use nih_plug::prelude::Enum;

// =============================================================================
// Constants
// =============================================================================

/// Fixed high-pass frequency (removes rumble and plosives)
pub const HP_FREQUENCY: f32 = 80.0;

/// Fixed low-pass frequency (removes hiss and aliasing)
pub const LP_FREQUENCY: f32 = 15500.0;

/// Quality factor for filters (0.707 = Butterworth response)
const Q_BUTTERWORTH: f32 = 0.70710678;

/// Small epsilon to prevent denormal numbers
const DENORMAL_OFFSET: f32 = 1.0e-25;

// =============================================================================
// 2nd Order State Variable Filter (SVF)
// =============================================================================

/// Single 2nd order SVF biquad section
/// Provides simultaneous LP, BP, HP outputs with numerical stability
#[derive(Clone, Debug)]
pub struct SvfBiquad {
    // State variables (integrated state for trapezoidal SVF)
    ic1eq: f32, // Integrator 1 state
    ic2eq: f32, // Integrator 2 state

    // Optimized Coefficients (Andrew Simper form)
    a1: f32,
    a2: f32,
    a3: f32,

    // Standard params stored for updates
    g: f32,
    k: f32,
}

impl SvfBiquad {
    /// Create a new SVF biquad
    pub fn new(cutoff_hz: f32, sample_rate: f32, q_factor: f32) -> Self {
        let mut filter = Self {
            ic1eq: 0.0,
            ic2eq: 0.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
            g: 0.0,
            k: 0.0,
        };
        filter.update(cutoff_hz, sample_rate, q_factor);
        filter
    }

    /// Update coefficients based on new parameters
    pub fn update(&mut self, cutoff_hz: f32, sample_rate: f32, q_factor: f32) {
        // Pre-warp frequency
        self.g = (PI * cutoff_hz / sample_rate).tan();

        // Damping: k = 1/Q
        self.k = 1.0 / q_factor;

        // Calculate optimized coefficients
        // Reference: Andrew Simper, "Linear Trapezoidal Integrated SVF"
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    /// Process one sample through the filter
    /// Returns (low_pass, band_pass, high_pass)
    #[inline]
    pub fn process(&mut self, input: f32) -> (f32, f32, f32) {
        // 1. Anti-Denormal: Add tiny DC offset to input to keep FP unit active
        // Alternatively, use proper CPU flags (DAZ/FTZ) in the audio thread setup.
        let v0 = input + DENORMAL_OFFSET;

        // 2. Solve the linear system
        let v3 = v0 - self.ic2eq;
        let v1 = self.a1 * self.ic1eq + self.a2 * v3;
        let v2 = self.ic2eq + self.a2 * self.ic1eq + self.a3 * v3;

        // 3. Update state variables
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;

        // 4. Calculate outputs
        let lp = v2;
        let bp = v1;
        let hp = v0 - self.k * v1 - v2;

        (lp, bp, hp)
    }

    /// Reset filter state to zero (e.g., on playback stop/seek)
    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }
}

// =============================================================================
// High Pass Filter (12 dB/oct or 24 dB/oct)
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "plugin", derive(Enum))]
pub enum HighPassSlope {
    /// 12 dB/octave (1 biquad)
    #[cfg_attr(feature = "plugin", name = "12 dB/oct")]
    Slope12dB,
    /// 24 dB/octave (2 biquads in series)
    #[cfg_attr(feature = "plugin", name = "24 dB/oct")]
    Slope24dB,
}

#[derive(Clone, Debug)]
pub struct HighPassFilter {
    stage1: SvfBiquad,
    stage2: Option<SvfBiquad>,
    sample_rate: f32,
    cutoff: f32,
}

impl HighPassFilter {
    pub fn new(sample_rate: f32, slope: HighPassSlope) -> Self {
        Self::new_with_cutoff(sample_rate, slope, HP_FREQUENCY)
    }

    pub fn new_with_cutoff(sample_rate: f32, slope: HighPassSlope, cutoff: f32) -> Self {
        let stage1 = SvfBiquad::new(cutoff, sample_rate, Q_BUTTERWORTH);
        let stage2 = match slope {
            HighPassSlope::Slope12dB => None,
            HighPassSlope::Slope24dB => {
                Some(SvfBiquad::new(cutoff, sample_rate, Q_BUTTERWORTH))
            }
        };

        Self { stage1, stage2, sample_rate, cutoff }
    }

    /// Must be called if the host changes sample rate
    pub fn set_sample_rate(&mut self, new_rate: f32) {
        if (self.sample_rate - new_rate).abs() > 0.1 {
            self.sample_rate = new_rate;
            self.stage1.update(self.cutoff, new_rate, Q_BUTTERWORTH);
            if let Some(s2) = &mut self.stage2 {
                s2.update(self.cutoff, new_rate, Q_BUTTERWORTH);
            }
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let (_, _, hp1) = self.stage1.process(input);

        if let Some(stage2) = &mut self.stage2 {
            let (_, _, hp2) = stage2.process(hp1);
            hp2
        } else {
            hp1
        }
    }

    pub fn reset(&mut self) {
        self.stage1.reset();
        if let Some(stage2) = &mut self.stage2 {
            stage2.reset();
        }
    }
}

// =============================================================================
// Low Pass Filter (12 dB/oct)
// =============================================================================

#[derive(Clone, Debug)]
pub struct LowPassFilter {
    biquad: SvfBiquad,
    sample_rate: f32,
}

impl LowPassFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            biquad: SvfBiquad::new(LP_FREQUENCY, sample_rate, Q_BUTTERWORTH),
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, new_rate: f32) {
        if (self.sample_rate - new_rate).abs() > 0.1 {
            self.sample_rate = new_rate;
            self.biquad.update(LP_FREQUENCY, new_rate, Q_BUTTERWORTH);
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let (lp, _, _) = self.biquad.process(input);
        lp
    }

    pub fn reset(&mut self) {
        self.biquad.reset();
    }
}

// =============================================================================
// Filter Chain (HP + LP)
// =============================================================================

#[derive(Clone, Debug)]
pub struct FilterChain {
    hp: HighPassFilter,
    lp: LowPassFilter,
}

impl FilterChain {
    pub fn new(sample_rate: f32, hp_slope: HighPassSlope) -> Self {
        Self::new_with_cutoff(sample_rate, hp_slope, HP_FREQUENCY)
    }

    pub fn new_with_cutoff(sample_rate: f32, hp_slope: HighPassSlope, hp_cutoff: f32) -> Self {
        Self {
            hp: HighPassFilter::new_with_cutoff(sample_rate, hp_slope, hp_cutoff),
            lp: LowPassFilter::new(sample_rate),
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.hp.set_sample_rate(sample_rate);
        self.lp.set_sample_rate(sample_rate);
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let after_hp = self.hp.process(input);
        self.lp.process(after_hp)
    }

    pub fn reset(&mut self) {
        self.hp.reset();
        self.lp.reset();
    }
}

impl crate::traits::AudioProcessor for FilterChain {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for FilterChain {}

impl crate::traits::Processor for FilterChain {
    fn new(sample_rate: f32) -> Self {
        Self::new(sample_rate, HighPassSlope::Slope24dB)
    }
}

// =============================================================================
// 1st Order High Shelf using SVF Topology (6 dB/oct, gentle slope)
// =============================================================================

/// 1st order high shelf filter using SVF topology
/// More stable and consistent with the rest of the processor.
/// Ideal for "Air EQ" - gentle, natural-sounding lift.
#[derive(Clone, Debug)]
pub struct HighShelfSvf {
    // State variable
    ic1eq: f32,

    // Coefficients
    g: f32,

    // Parameters
    sample_rate: f32,
    freq: f32,
    gain_db: f32,
}

impl HighShelfSvf {
    pub fn new(freq: f32, gain_db: f32, sample_rate: f32) -> Self {
        let mut shelf = Self {
            ic1eq: 0.0,
            g: 0.0,
            sample_rate,
            freq,
            gain_db,
        };
        shelf.update_coefficients();
        shelf
    }

    fn update_coefficients(&mut self) {
        // Standard tan-warped frequency
        self.g = (PI * self.freq / self.sample_rate).tan();
    }

    pub fn set_params(&mut self, freq: f32, gain_db: f32) {
        if (self.freq - freq).abs() > 0.1 || (self.gain_db - gain_db).abs() > 0.01 {
            self.freq = freq;
            self.gain_db = gain_db;
            self.update_coefficients();
        }
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        // 1st order SVF (simplified Trapezoidal)
        let v1 = (input - self.ic1eq) * (self.g / (1.0 + self.g));
        let v2 = v1 + self.ic1eq;
        self.ic1eq = v2 + v1;

        // Output logic for High Shelf:
        // Result = Input + (A - 1) * HighPass
        let a = 10.0f32.powf(self.gain_db / 20.0); // 1st order gain factor
        let hp = input - v2;

        input + (a - 1.0) * hp
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
    }
}

// =============================================================================
// 1st Order Low Shelf using SVF Topology (6 dB/oct, gentle slope)
// =============================================================================

/// 1st order low shelf filter using SVF topology
/// Mirror of HighShelfSvf but boosts/cuts low frequencies.
#[derive(Clone, Debug)]
pub struct LowShelfSvf {
    ic1eq: f32,
    g: f32,
    sample_rate: f32,
    freq: f32,
    gain_db: f32,
}

impl LowShelfSvf {
    pub fn new(freq: f32, gain_db: f32, sample_rate: f32) -> Self {
        Self {
            ic1eq: 0.0,
            g: (PI * freq / sample_rate).tan(),
            sample_rate,
            freq,
            gain_db,
        }
    }

    pub fn set_params(&mut self, freq: f32, gain_db: f32) {
        self.freq = freq;
        self.gain_db = gain_db;
        self.g = (PI * self.freq / self.sample_rate).tan();
    }

    pub fn get_freq(&self) -> f32 {
        self.freq
    }

    pub fn get_gain_db(&self) -> f32 {
        self.gain_db
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.gain_db.abs() < 0.1 {
            return input;
        }
        let v1 = (input - self.ic1eq) * (self.g / (1.0 + self.g));
        let v2 = v1 + self.ic1eq;
        self.ic1eq = v2 + v1;
        let a = 10.0_f32.powf(self.gain_db / 20.0);
        input + (a - 1.0) * v2
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
    }
}

// =============================================================================
// 2nd Order Peaking EQ (Bell) using SVF - wraps SvfBiquad
// =============================================================================

/// 2nd order peaking (bell) EQ filter - wraps SvfBiquad and adds gain mixing
#[derive(Clone, Debug)]
pub struct PeakingEqSvf {
    biquad: SvfBiquad,
    sample_rate: f32,
    freq: f32,
    q: f32,
    gain_db: f32,
}

impl PeakingEqSvf {
    pub fn new(freq: f32, q: f32, gain_db: f32, sample_rate: f32) -> Self {
        Self {
            biquad: SvfBiquad::new(freq, sample_rate, q),
            sample_rate,
            freq,
            q,
            gain_db,
        }
    }

    pub fn set_params(&mut self, freq: f32, q: f32, gain_db: f32) {
        self.freq = freq.clamp(20.0, self.sample_rate * 0.45);
        self.q = q.clamp(0.1, 10.0);
        self.gain_db = gain_db;
        self.biquad.update(self.freq, self.sample_rate, self.q);
    }

    pub fn get_freq(&self) -> f32 {
        self.freq
    }

    pub fn get_gain_db(&self) -> f32 {
        self.gain_db
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        if self.gain_db.abs() < 0.1 {
            return input;
        }
        let (_, bp, _) = self.biquad.process(input);
        let k = 1.0 / self.q;
        let gain_linear = 10.0_f32.powf(self.gain_db.abs() / 40.0);
        if self.gain_db > 0.0 {
            input + bp * (gain_linear * gain_linear - 1.0) * k
        } else {
            input - bp * (1.0 - 1.0 / (gain_linear * gain_linear)) * k
        }
    }

    pub fn reset(&mut self) {
        self.biquad.reset();
    }
}

// =============================================================================
// Configurable High Pass Filter - wraps SvfBiquad
// =============================================================================

/// 12 dB/oct high-pass with configurable frequency - wraps SvfBiquad
#[derive(Clone, Debug)]
pub struct SvfHighPass {
    biquad: SvfBiquad,
    sample_rate: f32,
    freq: f32,
}

impl SvfHighPass {
    pub fn new(freq: f32, sample_rate: f32) -> Self {
        Self {
            biquad: SvfBiquad::new(freq, sample_rate, Q_BUTTERWORTH),
            sample_rate,
            freq,
        }
    }

    pub fn set_freq(&mut self, freq: f32) {
        self.freq = freq.clamp(20.0, self.sample_rate * 0.45);
        self.biquad.update(self.freq, self.sample_rate, Q_BUTTERWORTH);
    }

    pub fn get_freq(&self) -> f32 {
        self.freq
    }

    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let (_, _, hp) = self.biquad.process(input);
        hp
    }

    pub fn reset(&mut self) {
        self.biquad.reset();
    }
}

// =============================================================================
// Band Extract Filter (Flat-Top Bandpass via Cascaded HP → LP)
// =============================================================================

// =============================================================================
// Notch Filter (Band-Reject) using SVF
// =============================================================================

/// Notch (band-reject) filter using SVF topology
/// Output = LP + HP (rejects the band around center frequency)
/// Used for hum removal at specific frequencies
#[derive(Clone, Debug)]
pub struct NotchFilter {
    svf: SvfBiquad,
    freq: f32,
    sample_rate: f32,
}

impl NotchFilter {
    /// Create a new notch filter
    /// freq: center frequency to reject (Hz)
    /// q: quality factor (higher = narrower notch, typical 10-50 for hum removal)
    pub fn new(freq: f32, sample_rate: f32, q: f32) -> Self {
        Self {
            svf: SvfBiquad::new(freq, sample_rate, q),
            freq,
            sample_rate,
        }
    }

    /// Update notch frequency
    pub fn set_freq(&mut self, freq: f32, q: f32) {
        self.freq = freq;
        self.svf.update(freq, self.sample_rate, q);
    }

    /// Get current center frequency
    pub fn get_freq(&self) -> f32 {
        self.freq
    }

    /// Process one sample through the notch filter
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let (lp, _bp, hp) = self.svf.process(input);
        // Notch = LP + HP (rejects the band)
        lp + hp
    }

    /// Reset filter state
    pub fn reset(&mut self) {
        self.svf.reset();
    }
}

// =============================================================================
// Band Extract Filter (Flat-Top Bandpass via Cascaded HP → LP)
// =============================================================================

/// Extracts a frequency band with flat response using cascaded HP → LP
/// Used for de-essing where we want to cut a flat frequency range, not a peaked notch
///
/// Unlike a resonant bandpass (peaked response), this produces:
/// - Flat response between start_freq and stop_freq
/// - Butterworth roll-off at the edges (12 dB/oct)
#[derive(Clone, Debug)]
pub struct BandExtractFilter {
    hp: SvfBiquad,      // High-pass at start_freq
    lp: SvfBiquad,      // Low-pass at stop_freq
    start_freq: f32,
    stop_freq: f32,
    sample_rate: f32,
}

impl BandExtractFilter {
    /// Create a new band extract filter
    /// start_freq: lower edge of the band (Hz)
    /// stop_freq: upper edge of the band (Hz)
    pub fn new(start_freq: f32, stop_freq: f32, sample_rate: f32) -> Self {
        Self {
            hp: SvfBiquad::new(start_freq, sample_rate, Q_BUTTERWORTH),
            lp: SvfBiquad::new(stop_freq, sample_rate, Q_BUTTERWORTH),
            start_freq,
            stop_freq,
            sample_rate,
        }
    }

    /// Update band frequencies
    pub fn set_band(&mut self, start_freq: f32, stop_freq: f32) {
        if (self.start_freq - start_freq).abs() > 0.1 {
            self.start_freq = start_freq;
            self.hp.update(start_freq, self.sample_rate, Q_BUTTERWORTH);
        }
        if (self.stop_freq - stop_freq).abs() > 0.1 {
            self.stop_freq = stop_freq;
            self.lp.update(stop_freq, self.sample_rate, Q_BUTTERWORTH);
        }
    }

    /// Get current start frequency
    pub fn get_start_freq(&self) -> f32 {
        self.start_freq
    }

    /// Get current stop frequency
    pub fn get_stop_freq(&self) -> f32 {
        self.stop_freq
    }

    /// Process one sample, extracting the band
    /// Returns the signal content between start_freq and stop_freq
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        // HP passes frequencies above start_freq
        let (_, _, hp_out) = self.hp.process(input);
        // LP passes frequencies below stop_freq (from HP output)
        let (lp_out, _, _) = self.lp.process(hp_out);
        // Result: flat-top bandpass between start and stop
        lp_out
    }

    /// Reset filter state
    pub fn reset(&mut self) {
        self.hp.reset();
        self.lp.reset();
    }
}

