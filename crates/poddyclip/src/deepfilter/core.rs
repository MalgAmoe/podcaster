//! DeepFilterNet denoiser core implementation

use anyhow::Result;
use df::tract::{DfParams, DfTract, RuntimeParams};
use df::transforms::resample;
use ndarray::{Array2, ArrayD, Axis};

use super::analysis::DfAnalysis;

/// DeepFilterNet-based AI denoiser for voice enhancement
///
/// This processor uses the DeepFilterNet deep learning model for noise reduction.
/// It is specifically designed for voice/speech denoising and automatically
/// handles resampling for non-48kHz audio.
///
/// # Example
///
/// ```ignore
/// let mut denoiser = DeepFilterDenoiser::new(48000)?;
/// let output = denoiser.process(&input_samples);
/// ```
/// SNR bracket for caching model parameters
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SnrBracket {
    VeryNoisy,  // < 10 dB
    Noisy,      // 10-20 dB
    Moderate,   // 20-30 dB
    Clean,      // 30-40 dB
    VeryClean,  // > 40 dB
}

impl SnrBracket {
    fn from_snr(snr: f32) -> Self {
        match snr {
            s if s < 10.0 => Self::VeryNoisy,
            s if s < 20.0 => Self::Noisy,
            s if s < 30.0 => Self::Moderate,
            s if s < 40.0 => Self::Clean,
            _ => Self::VeryClean,
        }
    }
}

pub struct DeepFilterDenoiser {
    /// The DeepFilterNet model (uses Tract backend)
    model: DfTract,
    /// Original input sample rate
    input_sample_rate: u32,
    /// Model's native sample rate (always 48kHz)
    model_sample_rate: usize,
    /// Runtime parameters for the model
    runtime_params: RuntimeParams,
    /// Cached SNR bracket to avoid unnecessary model rebuilds
    cached_snr_bracket: Option<SnrBracket>,
}

impl DeepFilterDenoiser {
    /// Create a new DeepFilterDenoiser with default parameters
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Input sample rate (audio will be resampled to 48kHz if needed)
    ///
    /// # Returns
    ///
    /// Result containing the denoiser or an error if model loading fails
    pub fn new(sample_rate: u32) -> Result<Self> {
        Self::new_with_snr(sample_rate, None)
    }

    /// Create a new DeepFilterDenoiser with parameters tuned for specific SNR
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Input sample rate (audio will be resampled to 48kHz if needed)
    /// * `analysis` - Optional analysis results to tune parameters upfront
    ///
    /// # Returns
    ///
    /// Result containing the denoiser or an error if model loading fails
    pub fn new_with_analysis(sample_rate: u32, analysis: &DfAnalysis) -> Result<Self> {
        Self::new_with_snr(sample_rate, Some(analysis.estimated_snr))
    }

    /// Internal constructor with optional SNR for parameter tuning
    fn new_with_snr(sample_rate: u32, snr: Option<f32>) -> Result<Self> {
        let (bracket, atten_lim, min_thresh, max_thresh, post_filter_beta) = match snr {
            Some(s) => {
                let b = SnrBracket::from_snr(s);
                let (a, min_t, max_t) = match b {
                    SnrBracket::VeryNoisy => (45.0, -12.0, 38.0),
                    SnrBracket::Noisy => (35.0, -14.0, 36.0),
                    SnrBracket::Moderate => (25.0, -16.0, 34.0),
                    SnrBracket::Clean => (18.0, -18.0, 30.0),
                    SnrBracket::VeryClean => (12.0, -20.0, 25.0),
                };
                let pf = if s < 20.0 { 0.005 } else { 0.0 };
                (Some(b), a, min_t, max_t, pf)
            }
            None => (None, 25.0, -16.0, 34.0, 0.0), // Moderate defaults
        };

        let r_params = RuntimeParams::default_with_ch(1)
            .with_atten_lim(atten_lim)
            .with_post_filter(post_filter_beta)
            .with_thresholds(min_thresh, max_thresh, max_thresh);

        let df_params = DfParams::default();
        let model = DfTract::new(df_params, &r_params)?;
        let model_sr = model.sr;

        Ok(Self {
            model,
            input_sample_rate: sample_rate,
            model_sample_rate: model_sr,
            runtime_params: r_params,
            cached_snr_bracket: bracket,
        })
    }

    /// Get the model's native sample rate (48kHz)
    pub fn model_sample_rate(&self) -> usize {
        self.model_sample_rate
    }

    /// Get the hop size used by the model
    pub fn hop_size(&self) -> usize {
        self.model.hop_size
    }

    /// Process audio with default parameters
    ///
    /// # Arguments
    ///
    /// * `audio` - Mono audio samples (f32, normalized to [-1, 1])
    ///
    /// # Returns
    ///
    /// Processed audio samples at the original sample rate
    pub fn process(&mut self, audio: &[f32]) -> Vec<f32> {
        self.process_internal(audio, None)
    }

    /// Process audio with auto-tuned parameters based on analysis
    ///
    /// # Arguments
    ///
    /// * `audio` - Mono audio samples (f32, normalized to [-1, 1])
    /// * `analysis` - Analysis results from `analyze_for_deepfilter`
    ///
    /// # Returns
    ///
    /// Processed audio samples at the original sample rate
    pub fn process_with_analysis(&mut self, audio: &[f32], analysis: &DfAnalysis) -> Vec<f32> {
        self.process_internal(audio, Some(analysis))
    }

    /// Internal processing implementation
    fn process_internal(&mut self, audio: &[f32], analysis: Option<&DfAnalysis>) -> Vec<f32> {
        // Apply auto-tuning if analysis provided
        if let Some(analysis) = analysis {
            self.auto_tune(analysis);
        }

        // Convert to ndarray format (1 channel, N samples)
        let samples: Array2<f32> = Array2::from_shape_vec((1, audio.len()), audio.to_vec())
            .expect("Failed to create array");

        // Preprocess (DC removal, high-pass, gain)
        let mut samples = samples;
        if let Some(analysis) = analysis {
            self.preprocess(&mut samples, analysis);
        }

        // Resample to model sample rate if needed
        let original_sr = self.input_sample_rate as usize;
        let noisy = if original_sr != self.model_sample_rate {
            match resample(samples.view(), original_sr, self.model_sample_rate, None) {
                Ok(resampled) => resampled,
                Err(_) => return audio.to_vec(), // Return original on error
            }
        } else {
            samples
        };
        let noisy = noisy.as_standard_layout();

        // Create output array
        let mut enhanced: Array2<f32> = match ArrayD::default(noisy.shape()).into_dimensionality() {
            Ok(arr) => arr,
            Err(_) => return audio.to_vec(),
        };

        // Process in hop-sized chunks
        for (ns_f, enh_f) in noisy
            .view()
            .axis_chunks_iter(Axis(1), self.model.hop_size)
            .zip(
                enhanced
                    .view_mut()
                    .axis_chunks_iter_mut(Axis(1), self.model.hop_size),
            )
        {
            if ns_f.len_of(Axis(1)) < self.model.hop_size {
                break;
            }
            if let Err(_) = self.model.process(ns_f, enh_f) {
                // On error, output zeros (will be handled by overlap)
                continue;
            }
        }

        // Resample back to original sample rate if needed
        let output = if original_sr != self.model_sample_rate {
            match resample(enhanced.view(), self.model_sample_rate, original_sr, None) {
                Ok(resampled) => resampled,
                Err(_) => enhanced,
            }
        } else {
            enhanced
        };

        // Convert back to Vec
        output.row(0).to_vec()
    }

    /// Auto-tune model parameters based on audio analysis
    fn auto_tune(&mut self, analysis: &DfAnalysis) {
        let new_bracket = SnrBracket::from_snr(analysis.estimated_snr);

        // Skip rebuild if SNR bracket hasn't changed
        if self.cached_snr_bracket == Some(new_bracket) {
            return;
        }

        let (atten_lim, min_thresh, max_thresh) = match new_bracket {
            SnrBracket::VeryNoisy => (45.0, -12.0, 38.0),
            SnrBracket::Noisy => (35.0, -14.0, 36.0),
            SnrBracket::Moderate => (25.0, -16.0, 34.0),
            SnrBracket::Clean => (18.0, -18.0, 30.0),
            SnrBracket::VeryClean => (12.0, -20.0, 25.0),
        };

        // Post-filter beta for very noisy signals
        let post_filter_beta = if analysis.estimated_snr < 20.0 {
            0.005
        } else {
            0.0
        };

        // Update runtime params
        self.runtime_params = RuntimeParams::default_with_ch(1)
            .with_atten_lim(atten_lim)
            .with_post_filter(post_filter_beta)
            .with_thresholds(min_thresh, max_thresh, max_thresh);

        // Recreate model with new params
        let df_params = DfParams::default();
        if let Ok(model) = DfTract::new(df_params, &self.runtime_params) {
            self.model = model;
            self.cached_snr_bracket = Some(new_bracket);
        }
    }

    /// Preprocess audio (DC removal, high-pass filter, gain adjustment)
    fn preprocess(&self, samples: &mut Array2<f32>, analysis: &DfAnalysis) {
        use std::f32::consts::PI;

        // DC offset removal
        if analysis.has_dc_offset {
            for mut channel in samples.axis_iter_mut(Axis(0)) {
                let mean = channel.iter().sum::<f32>() / channel.len() as f32;
                channel.mapv_inplace(|x| x - mean);
            }
        }

        // High-pass filter for rumble
        if analysis.low_freq_energy_ratio > 0.20 {
            let cutoff = if analysis.low_freq_energy_ratio > 0.35 {
                100.0
            } else {
                60.0
            };

            let rc = 1.0 / (2.0 * PI * cutoff);
            let dt = 1.0 / self.input_sample_rate as f32;
            let alpha = rc / (rc + dt);

            for mut channel in samples.axis_iter_mut(Axis(0)) {
                let mut prev_in = 0.0f32;
                let mut prev_out = 0.0f32;
                for sample in channel.iter_mut() {
                    let out = alpha * (prev_out + *sample - prev_in);
                    prev_in = *sample;
                    prev_out = out;
                    *sample = out;
                }
            }
        }

        // Gain adjustment for quiet audio
        if analysis.peak_db < -6.0 {
            let gain = 10f32.powf((-1.0 - analysis.peak_db) / 20.0);
            samples.mapv_inplace(|x| (x * gain).clamp(-0.99, 0.99));
        }
    }

    /// Reset the processor state
    ///
    /// Call this between processing different audio files or channels.
    /// Preserves the cached SNR bracket so the model won't be rebuilt
    /// if the next channel has similar noise characteristics.
    pub fn reset(&mut self) {
        // Recreate model to reset internal STFT state
        // Keep cached_snr_bracket so auto_tune can skip rebuild if SNR is similar
        let df_params = DfParams::default();
        if let Ok(model) = DfTract::new(df_params, &self.runtime_params) {
            self.model = model;
        }
    }

    /// Full reset including cached parameters
    ///
    /// Call this between processing completely different audio files
    pub fn reset_full(&mut self) {
        self.cached_snr_bracket = None;
        self.reset();
    }
}
