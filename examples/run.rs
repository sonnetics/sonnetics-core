//! Minimal example. Replace `path/to/model` with your extracted model directory.

use anyhow::Result;
use sonnetics_core::WakeEngine;
use std::path::Path;

fn main() -> Result<()> {
    // Download a "Hey Alfred" model from the Sonnetics CDN:
    // https://cdn.sonnetics.com/models/sonnetics-model-efea8354-3f81-4c61-9d50-7452cb901620.tar.gz
    // Or download your own custom model from the sonnetics dashboard

    let mut engine = WakeEngine::from_path(Path::new("path/to/extracted/model"), 16_000, 1)?;
    let audio_samples = vec![0.0_f32; 8_000];
    let phrase = engine.detect(&audio_samples, 0.25)?;
    println!("{:?}", phrase);
    Ok(())
}
