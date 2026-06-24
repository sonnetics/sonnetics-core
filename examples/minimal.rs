//! Minimal API reference — bring your own model and audio.
//!
//! This example shows the core WakeEngine API without the `batteries` feature.
//! You must provide:
//!   1. A path to a downloaded & extracted Sonnetics model directory
//!   2. Your own audio samples (e.g. loaded from a WAV file)
//!
//! For a ready-to-run demo with auto-download and mic capture, see the `demo` example:
//!   cargo run --example demo --features batteries

use anyhow::Result;
use sonnetics_core::WakeEngine;
use std::path::Path;

fn main() -> Result<()> {
    // Replace with the path to your extracted model directory.
    // Download one from: https://cdn.sonnetics.com/models/sonnetics-model-<uuid>.tar.gz
    let mut engine = WakeEngine::from_path(Path::new("path/to/extracted/model"), 16_000, 1)?;

    // Replace with your own audio samples (e.g. read from a microphone or WAV file).
    let audio_samples = vec![0.0_f32; 8_000];

    let phrase = engine.detect(&audio_samples, 0.5)?;
    println!("{:?}", phrase);
    Ok(())
}
