//! WASM bindings for sonnetics-core.
//!
//! Exports `init(files, sample_rate, channels)` matching sonnetics-js detector API.
//! `files` is Record<string, ArrayBuffer> with keys: manifest.json, models/layer1.onnx, models/layer2.onnx.

use std::collections::HashMap;

use js_sys::{ArrayBuffer, Object, Uint8Array};
use wasm_bindgen::prelude::*;

use crate::wake_engine::WakeEngine;

fn js_files_to_map(files: JsValue) -> Result<HashMap<String, Vec<u8>>, JsError> {
    let obj = files
        .dyn_ref::<Object>()
        .ok_or_else(|| JsError::new("files must be an object"))?;
    let mut map = HashMap::new();
    for entry in Object::entries(obj) {
        let arr = entry
            .dyn_into::<js_sys::Array>()
            .map_err(|_| JsError::new("invalid files object"))?;
        let key = arr
            .get(0)
            .as_string()
            .ok_or_else(|| JsError::new("key must be string"))?;
        let val = arr.get(1);
        let vec: Vec<u8> = if let Ok(ab) = val.clone().dyn_into::<ArrayBuffer>() {
            Uint8Array::new_with_byte_offset_and_length(&ab, 0, ab.byte_length()).to_vec()
        } else if let Ok(u8arr) = val.dyn_into::<Uint8Array>() {
            u8arr.to_vec()
        } else {
            return Err(JsError::new(
                "file value must be ArrayBuffer or Uint8Array",
            ));
        };
        map.insert(key, vec);
    }
    Ok(map)
}

/// Create engine from file map.
/// files: Record<string, ArrayBuffer> with manifest.json, models/layer1.onnx, models/layer2.onnx.
/// Called as init(files, sample_rate, channels) from sonnetics-js.
#[wasm_bindgen(js_name = init)]
pub fn init_with_files(
    files: JsValue,
    sample_rate: u32,
    channels: u16,
) -> Result<WasmWakeEngine, JsError> {
    let map = js_files_to_map(files)?;
    let inner = WakeEngine::from_files(&map, sample_rate, channels)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(WasmWakeEngine { inner })
}

/// Wake-word inference engine. Create with `WakeEngine::new()`, then call `process()`.
#[wasm_bindgen]
pub struct WasmWakeEngine {
    inner: WakeEngine,
}

#[wasm_bindgen]
impl WasmWakeEngine {
    /// Reset hidden state (e.g. when starting a new session).
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Returns P(wake) per frame for the given audio samples (interleaved, f32).
    #[wasm_bindgen(js_name = "detectProbs")]
    pub fn detect_probs(&mut self, audio: &[f32]) -> Result<Vec<f32>, JsError> {
        self.inner.process(audio).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Returns the detected phrase if any frame's P(wake) >= threshold, null otherwise. Resets hidden state when triggered.
    pub fn detect(&mut self, audio: &[f32], threshold: f32) -> Result<Option<String>, JsError> {
        self.inner.detect(audio, threshold).map_err(|e| JsError::new(&e.to_string()))
    }
}
