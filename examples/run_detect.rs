//! Example: detect wake word in a WAV file (boolean trigger).
//!
//! Usage:
//!   cargo run --example run_detect -- <model_dir> <path.wav> [threshold]
//!
//! model_dir must contain manifest.json and the model files it references.
//! threshold defaults to 0.25.

use anyhow::{Context, Result};
use hound::WavReader;
use sonnetics_core::wake_engine::WakeEngine;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <model_dir> <path.wav> [threshold]", args[0]);
        eprintln!("  model_dir must contain manifest.json and the files it references");
        eprintln!("  threshold defaults to 0.25");
        std::process::exit(1);
    }

    let model_path = Path::new(&args[1]);
    let wav_path = &args[2];
    let threshold: f32 = args.get(3).map(|s| s.parse().unwrap_or(0.25)).unwrap_or(0.25);

    let mut reader = WavReader::open(wav_path).context("open WAV file")?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = (1u32 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / scale)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
    };

    let mut engine = WakeEngine::new(model_path, sample_rate, channels)?;

    println!(
        "Processing {} ({} Hz, {} ch, threshold {})\n",
        wav_path, sample_rate, channels, threshold
    );

    const CHUNK_SAMPLES: usize = 2048;
    let mut pos = 0;
    let mut trigger_count = 0;
    while pos < samples.len() {
        let end = (pos + CHUNK_SAMPLES).min(samples.len());
        let chunk = &samples[pos..end];
        if let Some(phrase) = engine.detect(chunk, threshold)? {
            trigger_count += 1;
            println!("Detected '{}' at ~{:.2}s", phrase, pos as f32 / sample_rate as f32);
        }
        pos = end;
    }

    println!("\nTotal detections: {}", trigger_count);
    Ok(())
}
