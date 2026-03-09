//! PyO3 bindings for sonnetics-core.
//!
//! Exports `init(files, sample_rate, channels)` and `WakeEngine` with `detect()`.
//! `files` is dict[str, bytes] with keys: manifest.json, models/layer1.onnx, models/layer2.onnx.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::wake_engine::WakeEngine;

fn dict_to_map(dict: &Bound<'_, PyDict>) -> PyResult<HashMap<String, Vec<u8>>> {
    let mut map = HashMap::new();
    for (key, value) in dict {
        let k: String = key.extract()?;
        let v: Vec<u8> = value.extract()?;
        map.insert(k, v);
    }
    Ok(map)
}

/// Create engine from file map.
/// files: dict[str, bytes] with manifest.json, models/layer1.onnx, models/layer2.onnx.
#[pyfunction]
fn init(files: &Bound<'_, PyDict>, sample_rate: u32, channels: u16) -> PyResult<PyWakeEngine> {
    let map = dict_to_map(files)?;
    let inner = WakeEngine::new(&map, sample_rate, channels)
        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
    Ok(PyWakeEngine { inner })
}

/// Wake-word inference engine. Create with `init()`, then call `detect()`.
/// Not thread-safe due to tract ONNX internals (Rc).
#[pyclass(unsendable)]
struct PyWakeEngine {
    inner: WakeEngine,
}

#[pymethods]
impl PyWakeEngine {
    /// Reset hidden state (e.g. when starting a new session).
    fn reset(&mut self) {
        self.inner.reset();
    }

    /// Process audio and return the detected phrase if any frame's P(wake) >= threshold, None otherwise.
    /// Resets hidden state when triggered.
    /// audio: sequence of float32 samples (interleaved if multi-channel).
    fn detect(&mut self, audio: Vec<f32>, threshold: f32) -> PyResult<Option<String>> {
        self.inner
            .detect(&audio, threshold)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))
    }
}

/// Python module entry point.
#[pymodule]
fn sonnetics_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init, m)?)?;
    m.add_class::<PyWakeEngine>()?;
    Ok(())
}
