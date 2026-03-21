//! Integration tests: download model from URL, run inference on audio samples.

use flate2::read::GzDecoder;
use std::path::{Path, PathBuf};
use tar::Archive;

const MODEL_URL: &str =
    "https://cdn.sonnetics.com/models/sonnetics-model-efea8354-3f81-4c61-9d50-7452cb901620.tar.gz";

const SAMPLE_RATE: u32 = 16_000;
const CHANNELS: u16 = 1;
const THRESHOLD: f32 = 0.25;
const CHUNK_SIZE: usize = 2048;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn ensure_manifest_at_root(dir: &Path) -> anyhow::Result<()> {
    let manifest = dir.join("manifest.json");
    if manifest.exists() {
        Ok(())
    } else {
        anyhow::bail!(
            "manifest.json must be at top level of archive, not found at {}",
            manifest.display()
        )
    }
}

fn load_model(url: &str) -> anyhow::Result<(tempfile::TempDir, PathBuf)> {
    let resp = reqwest::blocking::get(url)?.error_for_status()?;
    let bytes = resp.bytes()?;
    let decoder = GzDecoder::new(bytes.as_ref());
    let mut archive = Archive::new(decoder);

    let temp_dir = tempfile::tempdir()?;
    archive.unpack(temp_dir.path())?;

    ensure_manifest_at_root(temp_dir.path())?;
    let model_path = temp_dir.path().to_path_buf();
    Ok((temp_dir, model_path))
}

fn load_wav(path: &Path) -> anyhow::Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|x| x as f32 / 32768.0))
        .collect::<Result<_, _>>()?;
    Ok(samples)
}

fn run_detection(
    engine: &mut sonnetics_core::WakeEngine,
    audio: &[f32],
) -> anyhow::Result<Option<String>> {
    for chunk in audio.chunks(CHUNK_SIZE) {
        if let Some(phrase) = engine.detect(chunk, THRESHOLD)? {
            return Ok(Some(phrase));
        }
    }
    Ok(None)
}

macro_rules! test_audio_fixture {
    ($name:ident, $file:expr, $should_detect:expr) => {
        #[test]
        fn $name() {
            let (_temp_dir, model_path) = load_model(MODEL_URL).expect("failed to download model");

            let mut engine = sonnetics_core::WakeEngine::from_path(&model_path, SAMPLE_RATE, CHANNELS)
                .expect("failed to create engine");

            let audio_path = fixtures_dir().join($file);
            let audio = load_wav(&audio_path).expect("failed to load audio fixture");

            let result = run_detection(&mut engine, &audio).expect("detection failed");

            if $should_detect {
                assert!(
                    result.is_some(),
                    "expected detection for {}, got None",
                    $file
                );
            } else {
                assert!(
                    result.is_none(),
                    "expected no detection for {}, got {:?}",
                    $file,
                    result
                );
            }
        }
    };
}

test_audio_fixture!(test_positive_sample, "positive.wav", true);
test_audio_fixture!(test_negative_sample, "negative.wav", false);

#[test]
fn test_silence() {
    let (_temp_dir, model_path) = load_model(MODEL_URL).expect("failed to download model");

    let mut engine =
        sonnetics_core::WakeEngine::from_path(&model_path, SAMPLE_RATE, CHANNELS)
            .expect("failed to create engine");

    let silence: Vec<f32> = vec![0.0; SAMPLE_RATE as usize * 2]; // 2 seconds
    let result = run_detection(&mut engine, &silence).expect("detection failed");
    assert!(
        result.is_none(),
        "silence should not trigger detection, got {:?}",
        result
    );
}
