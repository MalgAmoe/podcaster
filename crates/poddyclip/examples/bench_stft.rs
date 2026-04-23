//! Simple STFT processing benchmark
//!
//! Run with: cargo run --release --example bench_stft

use poddyclip::denoiser::RealtimeDenoiser;
use poddyclip::denoiser::SpectralGate;
use poddyclip::dereverb::DeReverbProcessor;
use std::time::Instant;

fn main() {
    let sample_rate = 48000;

    // Create test signals of different lengths
    let durations = [10, 30, 60]; // seconds

    println!("STFT Processing Benchmark");
    println!("==========================\n");

    for duration in durations {
        let num_samples = sample_rate as usize * duration;

        // Generate test signal (simple sine wave with some noise)
        let audio: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.3
                    + (t * 880.0 * 2.0 * std::f32::consts::PI).sin() * 0.1
                    + (i as f32 * 0.0001).sin() * 0.05 // pseudo-noise
            })
            .collect();

        println!(
            "Audio length: {} seconds ({} samples)",
            duration, num_samples
        );
        println!("-----------------------------------------");

        // Benchmark Denoiser
        {
            let mut denoiser = RealtimeDenoiser::new(sample_rate);
            let start = Instant::now();
            let _output = denoiser.process(&audio);
            let elapsed = start.elapsed();
            let realtime_factor = duration as f64 / elapsed.as_secs_f64();
            println!(
                "  Denoiser:     {:>7.2}ms ({:.1}x realtime)",
                elapsed.as_secs_f64() * 1000.0,
                realtime_factor
            );
        }

        // Benchmark SpectralGate
        {
            let mut gate = SpectralGate::new_with_preset(sample_rate, 2).unwrap();
            let start = Instant::now();
            let _output = gate.process(&audio);
            let elapsed = start.elapsed();
            let realtime_factor = duration as f64 / elapsed.as_secs_f64();
            println!(
                "  SpectralGate: {:>7.2}ms ({:.1}x realtime)",
                elapsed.as_secs_f64() * 1000.0,
                realtime_factor
            );
        }

        // Benchmark DeReverb
        {
            let mut dereverb = DeReverbProcessor::new_with_preset(sample_rate, 2).unwrap();
            let start = Instant::now();
            let _output = dereverb.process(&audio);
            let elapsed = start.elapsed();
            let realtime_factor = duration as f64 / elapsed.as_secs_f64();
            println!(
                "  DeReverb:     {:>7.2}ms ({:.1}x realtime)",
                elapsed.as_secs_f64() * 1000.0,
                realtime_factor
            );
        }

        // Combined pipeline (like real usage)
        {
            let mut denoiser = RealtimeDenoiser::new(sample_rate);
            let mut gate = SpectralGate::new_with_preset(sample_rate, 2).unwrap();
            let mut dereverb = DeReverbProcessor::new_with_preset(sample_rate, 2).unwrap();

            let start = Instant::now();
            let step1 = denoiser.process(&audio);
            let step2 = gate.process(&step1);
            let _step3 = dereverb.process(&step2);
            let elapsed = start.elapsed();
            let realtime_factor = duration as f64 / elapsed.as_secs_f64();
            println!(
                "  Combined:     {:>7.2}ms ({:.1}x realtime)",
                elapsed.as_secs_f64() * 1000.0,
                realtime_factor
            );
        }

        println!();
    }
}
