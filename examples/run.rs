use anyhow::Result;
use sonnetics_core::WakeEngine;
use std::path::Path;

fn main() -> Result<()> {
    let model_path = Path::new("path_to_model.tar.gz");
    let mut engine = WakeEngine::from_path(model_path, 16_000, 1)?;

    let audio = vec![0.0_f32; 8_000];
    let phrase = engine.detect(&audio, 0.25)?;

    println!("detect() -> {:?}", phrase);
    Ok(())
}
