//! sonnetics-core: wake-word inference library.
//!
//! Streaming ONNX-based wake-word detection engine.
//! Designed for reuse via Rust, Python (PyO3), or JavaScript (WASM).

pub mod log_mel;
pub mod wake_engine;

pub use wake_engine::WakeEngine;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(all(not(target_arch = "wasm32"), feature = "python"))]
mod python;
