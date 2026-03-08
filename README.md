# sonnetics-core

Wake-word inference library for [Sonnetics](https://sonnetics.com). Streaming ONNX-based detection with log-mel spectrograms. Designed for reuse via:

- **Rust** — direct crate usage
- **Python** — PyO3 bindings (planned)
- **JavaScript** — WebAssembly (see `wasm-pack build --target bundler`)

## Features

- **`WakeEngine`** — streaming wake-word inference from ONNX (layer 1 + layer 2)
- **`log_mel_spectrogram`** — log-mel spectrogram matching PyTorch/torchaudio
- Accepts arbitrary sample rate and channels; downmixes to mono and resamples to 16 kHz internally

## Model format

Expects a model pack with `manifest.json` that lists models by id. Example layout:

```
model_dir/
  manifest.json
  models/
    layer1.onnx
    layer2.onnx
```

`manifest.json`:

```json
{
  "version": "1",
  "models": [
    { "id": "layer1", "file": "models/layer1.onnx" },
    { "id": "layer2", "file": "models/layer2.onnx" }
  ]
}
```

## Example

Process a WAV file:

```bash
cargo run --example run -- <model_dir> <path.wav>
```

```rust
use sonnetics_core::wake_engine::WakeEngine;
use std::path::Path;

let mut engine = WakeEngine::new(Path::new("./models"), 16000, 1)?;
let probs = engine.process(&audio_samples)?;
for p in probs {
    println!("P(wake) = {:.4}", p);
}
```

## Building for JavaScript/WASM

Requires [wasm-pack](https://rustwasm.github.io/wasm-pack/) and the wasm32 target:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
wasm-pack build --target bundler --out-dir pkg
```

The `pkg/` directory contains the npm package. Publish with `cd pkg && npm publish`.

## License

Apache-2.0
