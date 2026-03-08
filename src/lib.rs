//! sonnetics-core: wake-word inference library.
//!
//! Streaming ONNX-based wake-word detection with log-mel spectrograms.
//! Designed for reuse via Rust, Python (PyO3), or JavaScript (WASM).

pub mod log_mel;
pub mod wake_engine;

#[cfg(target_arch = "wasm32")]
mod wasm;
