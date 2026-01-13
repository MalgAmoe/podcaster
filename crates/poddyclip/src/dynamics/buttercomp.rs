//! ButterComp2 - Smooth bipolar compressor
//!
//! Ported from Airwindows ButterComp2 (MIT License)
//! Original by Chris Johnson
//!
//! Key characteristics:
//! - Bipolar: Processes positive/negative halves separately
//! - Interleaved: 4 control paths (A/B × pos/neg) alternate each sample
//! - Opto-like release: Slows recovery when output is loud
//! - Soft knee: Inverse square gain calculation

#![allow(dead_code)]

// =============================================================================
// ButterComp Presets
// =============================================================================

/// Preset names (1-5)
pub const BUTTERCOMP_PRESET_NAMES: [&str; 5] =
    ["Gentle", "Light", "Moderate", "Strong", "Aggressive"];

/// ButterComp presets - compression amount (0-1 scale)
pub const BUTTERCOMP_PRESETS: [f32; 5] = [
    0.3, // 1: Gentle
    0.5, // 2: Light
    0.7, // 3: Moderate (default was 0.8)
    0.85, // 4: Strong
    1.0, // 5: Aggressive
];

/// Get preset name by level (1-5), returns "Unknown" for invalid levels
pub fn get_buttercomp_preset_name(level: u8) -> &'static str {
    BUTTERCOMP_PRESET_NAMES
        .get((level as usize).saturating_sub(1))
        .unwrap_or(&"Unknown")
}

/// Get ButterComp compress amount by level (1-5)
pub fn get_buttercomp_preset(level: u8) -> Option<f32> {
    BUTTERCOMP_PRESETS
        .get((level as usize).saturating_sub(1))
        .copied()
}

/// Flush denormals to zero to prevent CPU spikes
#[inline]
fn flush_denormal(x: f64) -> f64 {
    if x.abs() < 1.18e-37 { 0.0 } else { x }
}

/// Single channel ButterComp2 processor
#[derive(Clone, Debug)]
pub struct ButterComp2 {
    sample_rate: f32,
    overallscale: f32,

    // Interleaved control states (A/B banks × pos/neg)
    control_a_pos: f64,
    control_a_neg: f64,
    control_b_pos: f64,
    control_b_neg: f64,

    // Envelope targets
    target_pos: f64,
    target_neg: f64,

    // Feedback for opto-like release
    last_output: f64,

    // Alternates between A/B banks
    flip: bool,

    // Parameters
    compress: f32, // 0-1
    output: f32,   // 0-1 (displayed 0-2)
    wet: f32,      // 0-1
}

impl ButterComp2 {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            overallscale: sample_rate / 44100.0,
            control_a_pos: 1.0,
            control_a_neg: 1.0,
            control_b_pos: 1.0,
            control_b_neg: 1.0,
            target_pos: 1.0,
            target_neg: 1.0,
            last_output: 0.0,
            flip: false,
            compress: 0.0,
            output: 0.5,
            wet: 1.0,
        }
    }

    /// Set compression amount (0-1)
    pub fn set_compress(&mut self, compress: f32) {
        self.compress = compress.clamp(0.0, 1.0);
    }

    /// Get current compression amount
    pub fn get_compress(&self) -> f32 {
        self.compress
    }

    /// Set output level (0-1, displayed as 0-2)
    pub fn set_output(&mut self, output: f32) {
        self.output = output.clamp(0.0, 1.0);
    }

    /// Get current output level
    pub fn get_output(&self) -> f32 {
        self.output
    }

    /// Set dry/wet mix (0-1)
    pub fn set_wet(&mut self, wet: f32) {
        self.wet = wet.clamp(0.0, 1.0);
    }

    /// Get current wet mix
    pub fn get_wet(&self) -> f32 {
        self.wet
    }

    /// Process a single sample
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let input_sample = input as f64;
        let dry_sample = input_sample;

        // Calculate gains from compress parameter
        let input_gain = 10.0f64.powf((self.compress as f64 * 14.0) / 20.0);
        let comp_factor = 0.012 * (self.compress as f64 / 135.0);
        let output_level = self.output as f64 * 2.0;

        // Output gain compensation
        let output_gain = ((input_gain - 1.0) / 1.5) + 1.0;

        // Apply input gain
        let mut sample = input_sample * input_gain;

        // Calculate divisor - this is the opto-like behavior
        // Slows compressor recovery when output was high
        let mut divisor = comp_factor / (1.0 + self.last_output.abs());
        divisor /= self.overallscale as f64;
        let remainder = divisor;
        divisor = 1.0 - divisor;

        // Process positive half of waveform
        let mut input_pos = sample + 1.0;
        if input_pos < 0.0 {
            input_pos = 0.0;
        }
        let mut output_pos = input_pos / 2.0;
        if output_pos > 1.0 {
            output_pos = 1.0;
        }
        input_pos *= input_pos; // Square for envelope

        // Update positive target with exponential smoothing
        self.target_pos *= divisor;
        self.target_pos += input_pos * remainder;
        self.target_pos = flush_denormal(self.target_pos);
        let calc_pos = (1.0 / self.target_pos).powi(2); // Inverse square for soft knee

        // Process negative half of waveform
        let mut input_neg = (-sample) + 1.0;
        if input_neg < 0.0 {
            input_neg = 0.0;
        }
        let mut output_neg = input_neg / 2.0;
        if output_neg > 1.0 {
            output_neg = 1.0;
        }
        input_neg *= input_neg; // Square for envelope

        // Update negative target with exponential smoothing
        self.target_neg *= divisor;
        self.target_neg += input_neg * remainder;
        self.target_neg = flush_denormal(self.target_neg);
        let calc_neg = (1.0 / self.target_neg).powi(2); // Inverse square for soft knee

        // Update control values based on signal polarity and flip state
        if sample > 0.0 {
            // Working on positive half
            if self.flip {
                self.control_a_pos *= divisor;
                self.control_a_pos += calc_pos * remainder;
                self.control_a_pos = flush_denormal(self.control_a_pos);
            } else {
                self.control_b_pos *= divisor;
                self.control_b_pos += calc_pos * remainder;
                self.control_b_pos = flush_denormal(self.control_b_pos);
            }
        } else {
            // Working on negative half
            if self.flip {
                self.control_a_neg *= divisor;
                self.control_a_neg += calc_neg * remainder;
                self.control_a_neg = flush_denormal(self.control_a_neg);
            } else {
                self.control_b_neg *= divisor;
                self.control_b_neg += calc_neg * remainder;
                self.control_b_neg = flush_denormal(self.control_b_neg);
            }
        }

        // Calculate total multiplier by blending pos/neg based on input position
        let total_multiplier = if self.flip {
            (self.control_a_pos * output_pos) + (self.control_a_neg * output_neg)
        } else {
            (self.control_b_pos * output_pos) + (self.control_b_neg * output_neg)
        };

        // Apply compression
        sample *= total_multiplier;
        sample /= output_gain;

        // Apply output level
        if output_level != 1.0 {
            sample *= output_level;
        }

        // Apply wet/dry mix
        let wet = self.wet as f64;
        if wet != 1.0 {
            sample = (sample * wet) + (dry_sample * (1.0 - wet));
        }

        // Store output for next sample's release calculation
        self.last_output = flush_denormal(sample);

        // Alternate flip state
        self.flip = !self.flip;

        sample as f32
    }

    /// Process mono buffer in-place
    pub fn process_mono(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    /// Reset all state
    pub fn reset(&mut self) {
        self.control_a_pos = 1.0;
        self.control_a_neg = 1.0;
        self.control_b_pos = 1.0;
        self.control_b_neg = 1.0;
        self.target_pos = 1.0;
        self.target_neg = 1.0;
        self.last_output = 0.0;
        self.flip = false;
    }
}

impl crate::traits::AudioProcessor for ButterComp2 {
    fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            *sample = self.process(*sample);
        }
    }

    fn reset(&mut self) {
        self.reset()
    }
}

impl crate::traits::MonoProcessor for ButterComp2 {}

impl crate::traits::Processor for ButterComp2 {
    fn new(sample_rate: f32) -> Self {
        Self::new(sample_rate)
    }
}

