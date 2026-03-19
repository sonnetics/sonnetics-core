//! Minimal example. Replace `path/to/model` with your extracted model directory.

use anyhow::Result;
use sonnetics_core::WakeEngine;
use std::path::Path;

fn main() -> Result<()> {
    let mut engine = WakeEngine::from_path(Path::new("path/to/model"), 16_000, 1)?;
    let audio_samples = vec![0.0_f32; 8_000];
    let phrase = engine.detect(&audio_samples, 0.25)?;
    println!("{:?}", phrase);
    Ok(())
}
