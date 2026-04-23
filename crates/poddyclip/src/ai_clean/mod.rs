//! AI cleaning processor backed by the MossFormer2 speech enhancement model.
//!
//! The public surface stays generic so callers can use "AI Clean" without
//! depending on a model-branded module name.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use ort::value::TensorRef;
use ort::{ep, session::Session};
use realfft::{num_complex, ComplexToReal, RealFftPlanner, RealToComplex};

const MODEL_SAMPLE_RATE: u32 = 48_000;
const WIN_LEN: usize = 1920;
const HOP_SIZE: usize = 384;
const FFT_LEN: usize = 1920;
const FREQ_BINS: usize = FFT_LEN / 2 + 1;
const NUM_MELS: usize = 60;
const FEAT_DIM: usize = NUM_MELS * 3;
const PREEMPH: f32 = 0.97;
const DELTA_WIN: usize = 2;

const ONE_TIME_DECODE_SECS: f32 = 12.0;
const DECODE_WINDOW_SECS: f32 = 8.0;
const DECODE_STRIDE_RATIO: f32 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiCleanMode {
    OneShot,
    Segmented,
}

#[derive(Debug, Clone, Copy)]
pub struct AiCleanPlan {
    pub mode: AiCleanMode,
    pub model_sample_rate: u32,
    pub input_samples: usize,
    pub model_samples: usize,
    pub segment_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct AiCleanProgress {
    pub mode: AiCleanMode,
    pub completed_segments: usize,
    pub total_segments: usize,
    pub fraction_complete: f32,
}

struct AiCleanRuntimeState {
    session: Session,
}

pub struct AiCleanRuntime {
    state: Mutex<AiCleanRuntimeState>,
    mel_fb: Vec<f32>,
    fft_forward: Arc<dyn RealToComplex<f32>>,
    fft_inverse: Arc<dyn ComplexToReal<f32>>,
    window: Vec<f32>,
}

pub struct AiCleanProcessor {
    runtime: Arc<AiCleanRuntime>,
}

impl AiCleanRuntime {
    pub fn new(input_sample_rate: u32) -> Result<Self> {
        Self::new_with_cuda(input_sample_rate, true)
    }

    pub fn new_with_cuda(input_sample_rate: u32, use_cuda: bool) -> Result<Self> {
        if input_sample_rate != MODEL_SAMPLE_RATE {
            bail!(
                "AiCleanRuntime requires {} Hz audio, got {} Hz",
                MODEL_SAMPLE_RATE,
                input_sample_rate
            );
        }

        let model_dir = model_dir();
        let onnx_path = model_dir.join("model.onnx");
        let mel_path = model_dir.join("mel_fb.bin");

        let mut builder = Session::builder()?.with_intra_threads(4)?;
        if use_cuda {
            builder = builder.with_execution_providers([ep::CUDA::default()
                .with_device_id(0)
                .build()
                .error_on_failure()])?;
        }
        let session = builder.commit_from_file(&onnx_path).with_context(|| {
            if use_cuda {
                format!(
                    "failed to initialize MossFormer2 ONNX session with CUDA for {}",
                    onnx_path.display()
                )
            } else {
                format!(
                    "failed to initialize MossFormer2 ONNX session on CPU for {}",
                    onnx_path.display()
                )
            }
        })?;

        let mel_fb = load_mel_filterbank(&mel_path)?;

        let mut planner = RealFftPlanner::<f32>::new();
        let fft_forward = planner.plan_fft_forward(FFT_LEN);
        let fft_inverse = planner.plan_fft_inverse(FFT_LEN);

        let window: Vec<f32> = (0..WIN_LEN)
            .map(|n| 0.54 - 0.46 * (2.0 * PI * n as f32 / (WIN_LEN as f32 - 1.0)).cos())
            .collect();

        Ok(Self {
            state: Mutex::new(AiCleanRuntimeState { session }),
            mel_fb,
            fft_forward,
            fft_inverse,
            window,
        })
    }

    pub fn model_sample_rate(&self) -> u32 {
        MODEL_SAMPLE_RATE
    }

    pub fn analyze_run(&self, input_samples: usize) -> AiCleanPlan {
        let one_time_samples = (ONE_TIME_DECODE_SECS * MODEL_SAMPLE_RATE as f32) as usize;
        if input_samples <= one_time_samples {
            return AiCleanPlan {
                mode: AiCleanMode::OneShot,
                model_sample_rate: MODEL_SAMPLE_RATE,
                input_samples,
                model_samples: input_samples,
                segment_count: 1,
            };
        }

        let window = (DECODE_WINDOW_SECS * MODEL_SAMPLE_RATE as f32) as usize;
        let stride = (window as f32 * DECODE_STRIDE_RATIO) as usize;
        let total_len = padded_length(input_samples, window, stride);
        let segment_count = ((total_len - window) / stride) + 1;

        AiCleanPlan {
            mode: AiCleanMode::Segmented,
            model_sample_rate: MODEL_SAMPLE_RATE,
            input_samples,
            model_samples: input_samples,
            segment_count,
        }
    }

    pub fn process(&self, audio: &[f32]) -> Result<Vec<f32>> {
        self.process_with_progress(audio, |_| Ok(()))
    }

    pub fn process_with_progress<F>(&self, audio: &[f32], mut on_progress: F) -> Result<Vec<f32>>
    where
        F: FnMut(AiCleanProgress) -> Result<()>,
    {
        let plan = self.analyze_run(audio.len());
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("AiCleanRuntime lock poisoned"))?;
        self.process_native(audio, plan, &mut on_progress, &mut state.session)
    }

    fn process_native<F>(
        &self,
        audio: &[f32],
        plan: AiCleanPlan,
        on_progress: &mut F,
        session: &mut Session,
    ) -> Result<Vec<f32>>
    where
        F: FnMut(AiCleanProgress) -> Result<()>,
    {
        match plan.mode {
            AiCleanMode::OneShot => {
                let output = self.process_segment(audio, session)?;
                on_progress(AiCleanProgress {
                    mode: plan.mode,
                    completed_segments: 1,
                    total_segments: 1,
                    fraction_complete: 1.0,
                })?;
                Ok(output)
            }
            AiCleanMode::Segmented => self.process_segmented(audio, plan, on_progress, session),
        }
    }

    fn process_segmented<F>(
        &self,
        audio: &[f32],
        plan: AiCleanPlan,
        on_progress: &mut F,
        session: &mut Session,
    ) -> Result<Vec<f32>>
    where
        F: FnMut(AiCleanProgress) -> Result<()>,
    {
        let window = (DECODE_WINDOW_SECS * MODEL_SAMPLE_RATE as f32) as usize;
        let stride = (window as f32 * DECODE_STRIDE_RATIO) as usize;
        let give_up_length = (window - stride) / 2;

        let mut padded = audio.to_vec();
        let input_len = padded.len();
        let total_len = padded_length(input_len, window, stride);
        padded.resize(total_len, 0.0);

        let mut output = vec![0f32; total_len];
        let total_segments = ((total_len - window) / stride) + 1;

        for segment_index in 0..total_segments {
            let current_idx = segment_index * stride;
            let segment = &padded[current_idx..current_idx + window];
            let enhanced = self.process_segment(segment, session)?;

            if current_idx == 0 {
                let end = window - give_up_length;
                output[..end].copy_from_slice(&enhanced[..end]);
            } else {
                let src_start = give_up_length;
                let src_end = window - give_up_length;
                let dst_start = current_idx + give_up_length;
                let dst_end = current_idx + window - give_up_length;
                output[dst_start..dst_end].copy_from_slice(&enhanced[src_start..src_end]);
            }

            on_progress(AiCleanProgress {
                mode: plan.mode,
                completed_segments: segment_index + 1,
                total_segments,
                fraction_complete: (segment_index + 1) as f32 / total_segments as f32,
            })?;
        }

        output.truncate(input_len);
        Ok(output)
    }

    fn process_segment(&self, audio: &[f32], session: &mut Session) -> Result<Vec<f32>> {
        let scaled: Vec<f32> = audio.iter().map(|&x| x * 32768.0).collect();
        let (features, num_frames) = self.compute_features(&scaled);
        if num_frames == 0 {
            return Ok(vec![0.0; audio.len()]);
        }

        let mask = self.run_model(session, &features, num_frames)?;
        let (stft_real, stft_imag, stft_frames) = self.stft(&scaled);

        let num_frames = num_frames.min(stft_frames);
        let mut masked_real = vec![0f32; num_frames * FREQ_BINS];
        let mut masked_imag = vec![0f32; num_frames * FREQ_BINS];
        for t in 0..num_frames {
            let frame_offset = t * FREQ_BINS;
            for f in 0..FREQ_BINS {
                let idx = frame_offset + f;
                masked_real[idx] = stft_real[idx] * mask[idx];
                masked_imag[idx] = stft_imag[idx] * mask[idx];
            }
        }

        let output = self.istft(&masked_real, &masked_imag, num_frames, scaled.len());
        Ok(output.iter().map(|&x| x / 32768.0).collect())
    }

    fn run_model(
        &self,
        session: &mut Session,
        features: &[f32],
        num_frames: usize,
    ) -> Result<Vec<f32>> {
        let input =
            TensorRef::from_array_view(([1i64, num_frames as i64, FEAT_DIM as i64], features))?;
        let outputs = session.run(ort::inputs![input])?;
        let output_array = outputs[0].try_extract_tensor::<f32>()?;
        let data = output_array.1;

        Ok(data[..num_frames * FREQ_BINS].to_vec())
    }

    fn stft(&self, audio: &[f32]) -> (Vec<f32>, Vec<f32>, usize) {
        let num_frames = frame_count(audio.len());
        let mut reals = vec![0f32; num_frames * FREQ_BINS];
        let mut imags = vec![0f32; num_frames * FREQ_BINS];
        let mut scratch = self.fft_forward.make_scratch_vec();

        for i in 0..num_frames {
            let start = i * HOP_SIZE;
            let mut buf = vec![0f32; FFT_LEN];
            let end = (start + WIN_LEN).min(audio.len());
            for j in 0..(end - start) {
                buf[j] = audio[start + j] * self.window[j];
            }

            let mut spectrum = self.fft_forward.make_output_vec();
            self.fft_forward
                .process_with_scratch(&mut buf, &mut spectrum, &mut scratch)
                .expect("FFT size is fixed");

            let frame_offset = i * FREQ_BINS;
            for (bin_idx, c) in spectrum.iter().enumerate().take(FREQ_BINS) {
                reals[frame_offset + bin_idx] = c.re;
                imags[frame_offset + bin_idx] = c.im;
            }
        }

        (reals, imags, num_frames)
    }

    fn istft(
        &self,
        reals: &[f32],
        imags: &[f32],
        num_frames: usize,
        output_len: usize,
    ) -> Vec<f32> {
        let full_len = if num_frames > 0 {
            (num_frames - 1) * HOP_SIZE + WIN_LEN
        } else {
            0
        };
        let mut output = vec![0f32; full_len];
        let mut window_sum = vec![0f32; full_len];
        let mut scratch = self.fft_inverse.make_scratch_vec();

        for i in 0..num_frames {
            let frame_offset = i * FREQ_BINS;
            let mut spectrum = vec![num_complex::Complex::new(0.0, 0.0); FREQ_BINS];
            for bin_idx in 0..FREQ_BINS {
                spectrum[bin_idx] = num_complex::Complex::new(
                    reals[frame_offset + bin_idx],
                    imags[frame_offset + bin_idx],
                );
            }

            spectrum[0].im = 0.0;
            let last = spectrum.len() - 1;
            spectrum[last].im = 0.0;

            let mut buf = self.fft_inverse.make_output_vec();
            self.fft_inverse
                .process_with_scratch(&mut spectrum, &mut buf, &mut scratch)
                .expect("FFT size is fixed");

            let start = i * HOP_SIZE;
            let norm = 1.0 / FFT_LEN as f32;
            for j in 0..WIN_LEN {
                let windowed = buf[j] * norm * self.window[j];
                output[start + j] += windowed;
                window_sum[start + j] += self.window[j] * self.window[j];
            }
        }

        for (sample, weight) in output.iter_mut().zip(window_sum.iter()) {
            if *weight > 1e-8 {
                *sample /= *weight;
            }
        }

        output.truncate(output_len);
        output
    }

    fn compute_features(&self, audio: &[f32]) -> (Vec<f32>, usize) {
        let preemphasized = preemphasis(audio);
        let (power, num_frames) = self.power_spectrum(&preemphasized);
        if num_frames == 0 {
            return (Vec::new(), 0);
        }

        let fbank = self.apply_mel_filterbank(&power, num_frames);
        let delta = compute_deltas_flat(&fbank, num_frames, NUM_MELS);
        let delta_delta = compute_deltas_flat(&delta, num_frames, NUM_MELS);

        let mut features = vec![0f32; num_frames * FEAT_DIM];
        for frame_idx in 0..num_frames {
            let mel_offset = frame_idx * NUM_MELS;
            let feat_offset = frame_idx * FEAT_DIM;
            features[feat_offset..feat_offset + NUM_MELS]
                .copy_from_slice(&fbank[mel_offset..mel_offset + NUM_MELS]);
            features[feat_offset + NUM_MELS..feat_offset + (NUM_MELS * 2)]
                .copy_from_slice(&delta[mel_offset..mel_offset + NUM_MELS]);
            features[feat_offset + (NUM_MELS * 2)..feat_offset + FEAT_DIM]
                .copy_from_slice(&delta_delta[mel_offset..mel_offset + NUM_MELS]);
        }

        (features, num_frames)
    }

    fn power_spectrum(&self, audio: &[f32]) -> (Vec<f32>, usize) {
        let num_frames = frame_count(audio.len());
        let mut result = vec![0f32; num_frames * FREQ_BINS];
        let mut scratch = self.fft_forward.make_scratch_vec();

        for i in 0..num_frames {
            let start = i * HOP_SIZE;
            let mut buf = vec![0f32; FFT_LEN];
            let end = (start + WIN_LEN).min(audio.len());
            for j in 0..(end - start) {
                buf[j] = audio[start + j] * self.window[j];
            }

            let mut spectrum = self.fft_forward.make_output_vec();
            self.fft_forward
                .process_with_scratch(&mut buf, &mut spectrum, &mut scratch)
                .expect("FFT size is fixed");

            let frame_offset = i * FREQ_BINS;
            for (bin_idx, c) in spectrum.iter().enumerate().take(FREQ_BINS) {
                result[frame_offset + bin_idx] = c.re * c.re + c.im * c.im;
            }
        }

        (result, num_frames)
    }

    fn apply_mel_filterbank(&self, power_spec: &[f32], num_frames: usize) -> Vec<f32> {
        let mut mel = vec![0f32; num_frames * NUM_MELS];
        for frame_idx in 0..num_frames {
            let power_offset = frame_idx * FREQ_BINS;
            let mel_offset = frame_idx * NUM_MELS;
            for f in 0..FREQ_BINS {
                let value = power_spec[power_offset + f];
                let fb_offset = f * NUM_MELS;
                for m in 0..NUM_MELS {
                    mel[mel_offset + m] += value * self.mel_fb[fb_offset + m];
                }
            }
            for m in 0..NUM_MELS {
                mel[mel_offset + m] = mel[mel_offset + m].max(1e-10).ln();
            }
        }
        mel
    }
}

impl AiCleanProcessor {
    pub fn new(input_sample_rate: u32) -> Result<Self> {
        Ok(Self {
            runtime: Arc::new(AiCleanRuntime::new(input_sample_rate)?),
        })
    }

    pub fn from_runtime(runtime: Arc<AiCleanRuntime>) -> Self {
        Self { runtime }
    }

    pub fn model_sample_rate(&self) -> u32 {
        self.runtime.model_sample_rate()
    }

    pub fn analyze_run(&self, input_samples: usize) -> AiCleanPlan {
        self.runtime.analyze_run(input_samples)
    }

    pub fn process(&self, audio: &[f32]) -> Result<Vec<f32>> {
        self.runtime.process(audio)
    }

    pub fn process_with_progress<F>(&self, audio: &[f32], on_progress: F) -> Result<Vec<f32>>
    where
        F: FnMut(AiCleanProgress) -> Result<()>,
    {
        self.runtime.process_with_progress(audio, on_progress)
    }
}

fn model_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("mossformer2")
}

fn load_mel_filterbank(path: &Path) -> Result<Vec<f32>> {
    let mel_data = std::fs::read(path)?;
    if mel_data.len() < 8 {
        bail!("mel_fb.bin too small");
    }
    let rows = u32::from_le_bytes(mel_data[0..4].try_into()?) as usize;
    let cols = u32::from_le_bytes(mel_data[4..8].try_into()?) as usize;
    if rows != FREQ_BINS || cols != NUM_MELS {
        bail!(
            "mel_fb.bin shape mismatch: expected {}x{}, got {}x{}",
            FREQ_BINS,
            NUM_MELS,
            rows,
            cols
        );
    }
    let float_data = &mel_data[8..];
    if float_data.len() != rows * cols * 4 {
        bail!("mel_fb.bin data size mismatch");
    }

    Ok(float_data
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().expect("chunk size fixed")))
        .collect())
}

fn preemphasis(audio: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(audio.len());
    if audio.is_empty() {
        return out;
    }
    out.push(audio[0]);
    for i in 1..audio.len() {
        out.push(audio[i] - PREEMPH * audio[i - 1]);
    }
    out
}

fn compute_deltas_flat(features: &[f32], num_frames: usize, dim: usize) -> Vec<f32> {
    if num_frames == 0 {
        return vec![];
    }

    let denom: f32 = 2.0 * (1..=DELTA_WIN as i32).map(|n| (n * n) as f32).sum::<f32>();
    let mut deltas = vec![0f32; num_frames * dim];

    for t in 0..num_frames {
        for n in 1..=DELTA_WIN as i32 {
            let ahead_idx = (t as i32 + n).clamp(0, num_frames as i32 - 1) as usize;
            let behind_idx = (t as i32 - n).clamp(0, num_frames as i32 - 1) as usize;
            for d in 0..dim {
                deltas[t * dim + d] +=
                    n as f32 * (features[ahead_idx * dim + d] - features[behind_idx * dim + d]);
            }
        }
        for d in 0..dim {
            deltas[t * dim + d] /= denom;
        }
    }

    deltas
}

fn frame_count(audio_len: usize) -> usize {
    if audio_len >= WIN_LEN {
        (audio_len - WIN_LEN) / HOP_SIZE + 1
    } else {
        0
    }
}

fn padded_length(input_len: usize, window: usize, stride: usize) -> usize {
    if input_len < window {
        window
    } else if input_len < window + stride {
        window + stride
    } else if (input_len - window).is_multiple_of(stride) {
        input_len
    } else {
        let padding = stride - ((input_len - window) % stride);
        input_len + padding
    }
}
