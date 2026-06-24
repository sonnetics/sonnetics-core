//! Helpers for quick-start wake-word detection.
//!
//! Gated behind the `helpers` feature:
//!
//! - [`ensure_model`] — download a model from the Sonnetics CDN by ID and cache it locally
//! - [`MicStream`] — capture live audio from the default microphone
//!
//! # Example
//!
//! ```ignore
//! use sonnetics_core::helpers::{ensure_model, MicStream};
//! use sonnetics_core::WakeEngine;
//!
//! let model_path = ensure_model("sonnetics-model-a770c126-a4ff-4be4-b95e-7e104a01da73")?;
//! let mut mic = MicStream::open_default()?;
//! let mut engine = WakeEngine::from_path(&model_path, mic.sample_rate(), mic.channels())?;
//!
//! for chunk in mic {
//!     if let Some(phrase) = engine.detect(&chunk, 0.25)? {
//!         println!("Heard: {phrase}");
//!     }
//! }
//! # Ok::<_, anyhow::Error>(())
//! ```

use std::path::PathBuf;
use std::sync::mpsc;

use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use flate2::read::GzDecoder;
use tar::Archive;

const CDN_BASE: &str = "https://cdn.sonnetics.com/models";
const MODEL_ID_PREFIX: &str = "sonnetics-model-";

/// Normalize a model identifier.
///
/// Accepts bare UUIDs or `sonnetics-model-{uuid}` prefixed strings.
pub fn normalize_model_id(model_id: &str) -> String {
    if model_id.starts_with(MODEL_ID_PREFIX) {
        model_id.to_string()
    } else {
        format!("{}{}", MODEL_ID_PREFIX, model_id)
    }
}

fn cdn_url(model_id: &str) -> String {
    format!("{}/{}.tar.gz", CDN_BASE, normalize_model_id(model_id))
}

/// Ensure a model is downloaded and cached, returning the path to its directory.
///
/// `model_id` accepts a bare UUID or the `sonnetics-model-{uuid}` format.
///
/// The model is cached in `$SONNETICS_CACHE_DIR/<model-id>` if set,
/// otherwise `~/.cache/sonnetics/<model-id>` (Unix) or
/// `%APPDATA%/sonnetics/cache/<model-id>` (Windows).
pub fn ensure_model(model_id: &str) -> Result<PathBuf> {
    let model_id = normalize_model_id(model_id);
    let cache_root = cache_root()?;
    let model_dir = cache_root.join(&model_id);

    if model_dir.join("manifest.json").exists() {
        return Ok(model_dir);
    }

    let url = cdn_url(&model_id);
    eprintln!("Downloading model {model_id} (first run only)...");

    let response = reqwest::blocking::get(&url)
        .with_context(|| format!("failed to download model from {url}"))?
        .error_for_status()
        .with_context(|| format!("model download returned non-success status for {url}"))?;

    let bytes = response.bytes().context("failed to read model bytes")?;
    let decoder = GzDecoder::new(bytes.as_ref());
    let mut archive = Archive::new(decoder);

    std::fs::create_dir_all(&model_dir).context("failed to create model cache directory")?;
    archive
        .unpack(&model_dir)
        .context("failed to extract model archive")?;

    // If the archive nested files in a subdirectory, flatten them.
    let nested = model_dir.join(&model_id);
    if nested.exists() {
        flatten_dir(&nested, &model_dir)?;
        std::fs::remove_dir_all(&nested).context("failed to remove nested directory")?;
    }

    eprintln!("Model ready at {}", model_dir.display());
    Ok(model_dir)
}

fn cache_root() -> Result<PathBuf> {
    // Honour the same env var as Python and JS
    if let Ok(dir) = std::env::var("SONNETICS_CACHE_DIR") {
        return Ok(PathBuf::from(dir));
    }

    #[cfg(target_os = "windows")]
    {
        let app_data = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join("AppData").join("Roaming")
            });
        Ok(app_data.join("sonnetics").join("cache"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home).join(".cache").join("sonnetics"))
    }
}

fn flatten_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    for entry in std::fs::read_dir(src).context("failed to read nested directory")? {
        let entry = entry?;
        let file_name = entry.file_name();
        let target = dst.join(&file_name);
        if target.exists() {
            std::fs::remove_file(&target).ok();
        }
        std::fs::rename(entry.path(), &target)
            .with_context(|| format!("failed to move {:?} to {:?}", entry.path(), target))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Microphone capture
// ---------------------------------------------------------------------------

/// Live audio stream from the default microphone.
///
/// Each call to [`next`](Iterator::next) blocks until a chunk of audio is
/// available. Audio is always delivered as `f32` samples (mono mixing and
/// resampling to 16 kHz is handled internally by [`WakeEngine`]).
pub struct MicStream {
    sample_rate: u32,
    channels: u16,
    receiver: mpsc::Receiver<Vec<f32>>,
    _stream: cpal::Stream,
}

impl MicStream {
    /// Open the default audio input device and begin capturing.
    ///
    /// Returns an error if no input device is available or the stream
    /// cannot be started.
    pub fn open_default() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("no default audio input device found")?;

        let config = device
            .default_input_config()
            .context("failed to get default input config")?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();

        let (tx, receiver) = mpsc::channel();
        let err_fn = |err| eprintln!("audio stream error: {err}");

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream::<f32, _, _>(
                &config.into(),
                move |data: &[f32], _| {
                    let _ = tx.send(data.to_vec());
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream::<i16, _, _>(
                &config.into(),
                move |data: &[i16], _| {
                    let samples: Vec<f32> =
                        data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
                    let _ = tx.send(samples);
                },
                err_fn,
                None,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream::<u16, _, _>(
                &config.into(),
                move |data: &[u16], _| {
                    let samples: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 - 32768.0) / 32768.0)
                        .collect();
                    let _ = tx.send(samples);
                },
                err_fn,
                None,
            )?,
            _ => anyhow::bail!("unsupported sample format on input device"),
        };

        stream.play().context("failed to start audio stream")?;

        Ok(Self {
            sample_rate,
            channels: channels as u16,
            receiver,
            _stream: stream,
        })
    }

    /// Sample rate of the audio device (Hz).
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Number of channels in the captured audio.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

impl Iterator for MicStream {
    type Item = Vec<f32>;

    /// Block until the next audio chunk arrives.
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}
