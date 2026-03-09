//! Example: get per-frame P(wake) for a WAV file (detectProbs interface).
//!
//! Usage:
//!   cargo run --example run -- <model_dir> <path.wav>
//!
//! model_dir must contain manifest.json and the model files it references.

use anyhow::{Context, Result};
use hound::WavReader;
use sonnetics_core::wake_engine::WakeEngine;
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <model_dir> <path.wav>", args[0]);
        eprintln!("  model_dir must contain manifest.json and the files it references");
        std::process::exit(1);
    }

    let model_path = Path::new(&args[1]);
    let wav_path = &args[2];

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

    let mut engine = WakeEngine::from_path(model_path, sample_rate, channels)?;

    println!("Processing {} ({} Hz, {} ch)\n", wav_path, sample_rate, channels);

    const CHUNK_SAMPLES: usize = 1024;
    let mut pos = 0;
    while pos < samples.len() {
        let end = (pos + CHUNK_SAMPLES).min(samples.len());
        let chunk = &samples[pos..end];
        let probs = engine.process(chunk)?;
        for p in probs {
            println!("P(wake) = {:.4}", p);
        }
        pos = end;
    }

    Ok(())
}
