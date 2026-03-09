//! Streaming wake-word inference via ONNX (layer 1 + layer 2).
//!
//! Uses a fixed-size ring buffer, processes incrementally, returns P(wake) per frame.
//! Accepts arbitrary sample rate and channel count; downmixes to mono and resamples to 16 kHz.

use anyhow::{Context, Result};
use resampler::{Attenuation, Latency, ResamplerFir, SampleRate};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use tract_ndarray::Array4;
use tract_onnx::prelude::*;

use crate::log_mel::log_mel_spectrogram;

const SAMPLE_RATE_ERR: &str = "sample rate not supported (supported: 22050, 16000, 32000, 44100, 48000, 88200, 96000, 176400, 192000, 384000)";

#[derive(Debug, Deserialize)]
struct ManifestModel {
    id: String,
    file: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    models: Vec<ManifestModel>,
    /// Optional label per class. Index = class id. For binary: [negative, positive], use labels[1] when detected.
    #[serde(default)]
    labels: Option<Vec<String>>,
}

fn load_model_from_read(
    r: &mut dyn Read,
    name: &str,
) -> Result<TypedRunnableModel<TypedModel>> {
    tract_onnx::onnx()
        .model_for_read(r)
        .map_err(|e| anyhow::anyhow!("load {name} model: {}", e))?
        .into_optimized()
        .map_err(|e| anyhow::anyhow!("optimize {name} model: {}", e))?
        .into_runnable()
        .map_err(|e| anyhow::anyhow!("optimize {name} model: {}", e))
}

fn create_resampler(
    sample_rate: u32,
) -> Result<(
    Option<ResamplerFir>,
    Vec<f32>,
    Vec<f32>,
)> {
    if sample_rate == SAMPLE_RATE {
        return Ok((None, Vec::new(), Vec::new()));
    }
    let input_rate = SampleRate::try_from(sample_rate)
        .map_err(|_| anyhow::anyhow!("{sample_rate} Hz {SAMPLE_RATE_ERR}"))?;
    let r = ResamplerFir::new(
        1,
        input_rate,
        SampleRate::Hz16000,
        Latency::Sample64,
        Attenuation::Db90,
    );
    let buf_size = r.buffer_size_output();
    Ok((Some(r), vec![0.0; buf_size], Vec::new()))
}

/// Audio constants matching training (16 kHz, hop 200, 40 mels, 0.2s stride).
const SAMPLE_RATE: u32 = 16_000;
const LAYER2_STATE_LAYERS: usize = 2;
const LAYER2_STATE_SIZE: usize = 128;

const FRAME_SAMPLES: usize = 6600; // ~0.41 s, yields 32 mel frames
const HOP_SAMPLES: usize = 3200;   // 0.2 s
const WINDOW_FRAMES: usize = 32;

/// Streaming wake-word inference engine.
pub struct WakeEngine {
    ring: RingBuffer,
    layer1_model: TypedRunnableModel<TypedModel>,
    layer2_model: TypedRunnableModel<TypedModel>,
    hidden: Option<TValue>,
    channels: u16,
    resampler: Option<ResamplerFir>,
    resampler_output_buf: Vec<f32>,
    resampler_input_buf: Vec<f32>,
    /// Label per class. For binary, index 1 = positive class.
    phrase_detected: String,
}

struct RingBuffer {
    buf: Vec<f32>,
    capacity: usize,
    start: usize,
    len: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0.0; capacity],
            capacity,
            start: 0,
            len: 0,
        }
    }

    fn available(&self) -> usize {
        self.len
    }

    fn read_frame(&self, n: usize) -> Vec<f32> {
        assert!(n <= self.len);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let idx = (self.start + i) % self.capacity;
            out.push(self.buf[idx]);
        }
        out
    }

    fn advance(&mut self, n: usize) {
        assert!(n <= self.len);
        self.start = (self.start + n) % self.capacity;
        self.len -= n;
    }

    fn push_one(&mut self, sample: f32) {
        assert!(
            self.len < self.capacity,
            "ring buffer overflow - process frames first"
        );
        let write_idx = (self.start + self.len) % self.capacity;
        self.buf[write_idx] = sample;
        self.len += 1;
    }

    fn push_and_drain(&mut self, audio: &[f32]) -> Vec<Vec<f32>> {
        let mut frames = Vec::new();
        for &sample in audio {
            while self.len >= self.capacity && self.available() >= FRAME_SAMPLES {
                frames.push(self.read_frame(FRAME_SAMPLES));
                self.advance(HOP_SAMPLES);
            }
            if self.len < self.capacity {
                self.push_one(sample);
            }
            while self.available() >= FRAME_SAMPLES {
                frames.push(self.read_frame(FRAME_SAMPLES));
                self.advance(HOP_SAMPLES);
            }
        }
        frames
    }
}

impl WakeEngine {
    /// Create engine from in-memory model pack. Map keys: "manifest.json" and the file paths referenced therein (e.g. "models/layer1.onnx", "models/layer2.onnx").
    /// Audio is downmixed to mono if `channels > 1` and resampled to 16 kHz if `sample_rate != 16000`.
    /// Supported sample rates: 22050, 16000, 32000, 44100, 48000, 88200, 96000, 176400, 192000, 384000.
    pub fn new(
        files: &HashMap<String, Vec<u8>>,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self> {
        let manifest_bytes = files
            .get("manifest.json")
            .context("pack must contain manifest.json")?;
        let manifest: Manifest =
            serde_json::from_slice(manifest_bytes).context("parse manifest.json")?;

        let mut layer1_bytes: Option<&Vec<u8>> = None;
        let mut layer2_bytes: Option<&Vec<u8>> = None;
        for m in &manifest.models {
            if m.id == "layer1" {
                layer1_bytes = files.get(&m.file);
            } else if m.id == "layer2" {
                layer2_bytes = files.get(&m.file);
            }
        }

        let layer1 = layer1_bytes.context("manifest must define model with id 'layer1'")?;
        let layer2 = layer2_bytes.context("manifest must define model with id 'layer2'")?;
        let phrase_detected = manifest
            .labels
            .as_ref()
            .and_then(|l| l.get(1).cloned())
            .unwrap_or_else(|| "wake".to_string());

        Self::build_from_bytes(layer1, layer2, sample_rate, channels, phrase_detected)
    }

    /// Create engine from a model directory on disk. Reads manifest.json and the files it references.
    pub fn from_path(model_path: &Path, sample_rate: u32, channels: u16) -> Result<Self> {
        let manifest_path = model_path.join("manifest.json");
        let manifest_bytes = fs::read(&manifest_path)
            .with_context(|| format!("read manifest at {:?}", manifest_path))?;
        let manifest: Manifest =
            serde_json::from_slice(&manifest_bytes).context("parse manifest.json")?;

        let mut files = HashMap::new();
        files.insert("manifest.json".to_string(), manifest_bytes);

        for m in &manifest.models {
            let file_path = model_path.join(&m.file);
            let bytes = fs::read(&file_path)
                .with_context(|| format!("read {:?} at {:?}", m.file, file_path))?;
            files.insert(m.file.clone(), bytes);
        }

        Self::new(&files, sample_rate, channels)
    }

    fn build_from_bytes(
        layer1_bytes: &[u8],
        layer2_bytes: &[u8],
        sample_rate: u32,
        channels: u16,
        phrase_detected: String,
    ) -> Result<Self> {
        let mut layer1_cursor = Cursor::new(layer1_bytes);
        let mut layer2_cursor = Cursor::new(layer2_bytes);

        let layer1_model = load_model_from_read(&mut layer1_cursor, "layer 1")?;
        let layer2_model = load_model_from_read(&mut layer2_cursor, "layer 2")?;
        Self::build(layer1_model, layer2_model, sample_rate, channels, phrase_detected)
    }

    fn build(
        layer1_model: TypedRunnableModel<TypedModel>,
        layer2_model: TypedRunnableModel<TypedModel>,
        sample_rate: u32,
        channels: u16,
        phrase_detected: String,
    ) -> Result<Self> {
        let (resampler, resampler_output_buf, resampler_input_buf) =
            create_resampler(sample_rate)?;
        let capacity = FRAME_SAMPLES + HOP_SAMPLES;
        Ok(Self {
            ring: RingBuffer::new(capacity),
            layer1_model,
            layer2_model,
            hidden: None,
            channels,
            resampler,
            resampler_output_buf,
            resampler_input_buf,
            phrase_detected,
        })
    }

    /// Reset hidden state (call when starting a new session).
    pub fn reset(&mut self) {
        self.hidden = None;
        self.resampler_input_buf.clear();
        if let Some(ref mut r) = self.resampler {
            r.reset();
        }
    }

    /// Process interleaved audio at the configured sample rate and channels.
    /// Returns one P(wake) probability per frame produced.
    pub fn process(&mut self, audio: &[f32]) -> Result<Vec<f32>> {
        let mono = self.downmix(audio);
        let at_16k = self.resample(&mono)?;
        let frames = self.ring.push_and_drain(&at_16k);
        let mut probs = Vec::with_capacity(frames.len());
        for window in frames {
            probs.push(self.process_one_frame(&window)?);
        }
        Ok(probs)
    }

    /// Process audio and return Some(phrase) if any frame's P(wake) >= threshold, None otherwise.
    /// Resets hidden state when triggered.
    pub fn detect(&mut self, audio: &[f32], threshold: f32) -> Result<Option<String>> {
        let probs = self.process(audio)?;
        let triggered = probs.iter().any(|&p| p >= threshold);
        if triggered {
            self.reset();
            Ok(Some(self.phrase_detected.clone()))
        } else {
            Ok(None)
        }
    }

    fn downmix(&self, audio: &[f32]) -> Vec<f32> {
        let ch = self.channels as usize;
        if ch <= 1 {
            return audio.to_vec();
        }
        audio
            .chunks(ch)
            .map(|c| c.iter().sum::<f32>() / ch as f32)
            .collect()
    }

    fn resample(&mut self, mono: &[f32]) -> Result<Vec<f32>> {
        let Some(ref mut r) = self.resampler else {
            return Ok(mono.to_vec());
        };
        self.resampler_input_buf.extend_from_slice(mono);
        let mut out = Vec::new();
        let buf = &mut self.resampler_output_buf;
        while !self.resampler_input_buf.is_empty() {
            let (consumed, produced) = r.resample(&self.resampler_input_buf, buf)?;
            if consumed == 0 {
                break;
            }
            out.extend_from_slice(&buf[..produced]);
            self.resampler_input_buf.drain(..consumed);
        }
        Ok(out)
    }

    fn process_one_frame(&mut self, window: &[f32]) -> Result<f32> {
        let mel = log_mel_spectrogram(window);
        if mel.nrows() < WINDOW_FRAMES {
            return Ok(0.0);
        }
        let mel = mel.slice(ndarray::s![..WINDOW_FRAMES, ..]);
        let layer1_input: TValue = mel_to_layer1_input(&mel).into_tvalue();
        let layer1_result = self.layer1_model.run(tvec!(layer1_input))?;
        let embedding = layer1_result[0].clone();
        let (logits, new_hidden) = self.run_layer2(embedding)?;
        self.hidden = Some(new_hidden);
        Ok(softmax_class1(&logits))
    }

    fn run_layer2(&self, embedding: TValue) -> Result<(Vec<f32>, TValue)> {
        let hidden_t: TValue = self.hidden.as_ref().map_or_else(
            || {
                tract_ndarray::Array3::<f32>::zeros((LAYER2_STATE_LAYERS, 1, LAYER2_STATE_SIZE))
                    .into_tvalue()
            },
            |h| h.clone(),
        );

        let result = self.layer2_model.run(tvec!(embedding, hidden_t))?;
        let a0 = result[0].to_array_view::<f32>().context("layer 2 output 0")?;
        let a1 = result[1].to_array_view::<f32>().context("layer 2 output 1")?;
        let (logits, new_hidden) = if a0.len() == 2 {
            (a0.iter().copied().collect::<Vec<_>>(), result[1].clone())
        } else if a1.len() == 2 {
            (a1.iter().copied().collect::<Vec<_>>(), result[0].clone())
        } else {
            anyhow::bail!(
                "layer 2 model: expected one output of size 2 (logits), got {} and {} elements",
                a0.len(),
                a1.len()
            );
        };
        Ok((logits, new_hidden))
    }
}

fn mel_to_layer1_input(mel: &ndarray::ArrayView2<f64>) -> tract_ndarray::Array4<f32> {
    let (n_time, n_mels) = (mel.nrows(), mel.ncols());
    Array4::from_shape_fn((1, 1, n_mels, n_time), |(_, _, m, t)| mel[[t, m]] as f32)
}

fn softmax_class1(logits: &[f32]) -> f32 {
    if logits.len() < 2 {
        return 0.0;
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp0 = (logits[0] - max).exp();
    let exp1 = (logits[1] - max).exp();
    exp1 / (exp0 + exp1)
}
