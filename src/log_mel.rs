//! Log-mel spectrogram computation.
//!
//! Matches PyTorch/torchaudio: MelSpectrogram + AmplitudeToDB.

use ndarray::Array2;

const N_FFT: usize = 400;
const HOP_LENGTH: usize = 200;
const N_MELS: usize = 40;
const SAMPLE_RATE: f64 = 16_000.0;

/// Compute log-mel spectrogram (time, n_mels) matching train.py:
/// MelSpectrogram(sample_rate, n_mels=40, hop_length=200, center=False) + AmplitudeToDB().
pub fn log_mel_spectrogram(audio: &[f32]) -> Array2<f64> {
    let mut spectrogram = mel_spec::stft::Spectrogram::new(N_FFT, HOP_LENGTH);
    let filters = mel_spec::mel::mel(SAMPLE_RATE, N_FFT, N_MELS, None, None, true, false);

    let mut frames = Vec::new();
    for chunk in audio.chunks(HOP_LENGTH) {
        if let Some(fft) = spectrogram.add(chunk) {
            let log_mel = mel_spec::mel::log_mel_spectrogram(&fft, &filters);
            let db = log_mel.mapv(|x| 10.0 * x);
            frames.push(db);
        }
    }

    if frames.is_empty() {
        return Array2::zeros((0, N_MELS));
    }

    let n_time = frames.len();
    let mut out = Array2::zeros((n_time, N_MELS));
    for (t, frame) in frames.into_iter().enumerate() {
        for m in 0..N_MELS {
            out[[t, m]] = frame[[m, 0]];
        }
    }
    out
}
