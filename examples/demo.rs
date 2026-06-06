//! Out-of-the-box demo: detects wake words from your microphone.
//!
//! Run: cargo run --example demo --features helpers
//!
//! The first run downloads the model (~8 MB) and caches it.
//! Subsequent runs start instantly.
//!
//! Customise the model_id to use your own model from the Sonnetics dashboard.

use sonnetics_core::helpers::{ensure_model, MicStream};
use sonnetics_core::WakeEngine;

const MODEL_ID: &str = "sonnetics-model-efea8354-3f81-4c61-9d50-7452cb901620";

fn main() -> anyhow::Result<()> {
    let model_path = ensure_model(MODEL_ID)?;
    let mic = MicStream::open_default()?;
    let mut engine = WakeEngine::from_path(&model_path, mic.sample_rate(), mic.channels())?;

    println!("🔊 Listening for wake word... (Ctrl+C to stop)");

    for chunk in mic {
        if let Some(phrase) = engine.detect(&chunk, 0.25)? {
            println!("✅ Heard: {phrase}");
        }
    }

    Ok(())
}
