"""Log-mel front end on the host CPU, compatible with NeMo
`AudioToMelSpectrogramPreprocessor` defaults (Hann window, Slaney mel scale,
log with guard 2**-24, per-feature normalisation).

The Edge TPU runs only the acoustic model. The front end costs little CPU
compared with the model; bench_*.py measures it as part of the host load.
"""
from __future__ import annotations

from dataclasses import dataclass

import numpy as np


@dataclass(frozen=True)
class Frontend:
    sample_rate: int = 16000
    window_size: float = 0.02
    window_stride: float = 0.01
    n_fft: int = 512
    features: int = 64
    preemph: float = 0.97
    normalize: str = "per_feature"
    log_guard: float = 2.0**-24

    @classmethod
    def from_meta(cls, meta: dict) -> "Frontend":
        keys = cls.__dataclass_fields__.keys()
        return cls(**{k: v for k, v in meta.get("frontend", {}).items() if k in keys})

    @property
    def hop(self) -> int:
        return int(round(self.sample_rate * self.window_stride))

    @property
    def win(self) -> int:
        return int(round(self.sample_rate * self.window_size))

    def frames(self, samples: int) -> int:
        return samples // self.hop + 1


def _hz_to_mel(f):
    f = np.asanyarray(f, dtype=np.float64)
    lin = f / (200.0 / 3)
    log = 15.0 + np.log(np.maximum(f, 1e-10) / 1000.0) / (np.log(6.4) / 27.0)
    return np.where(f >= 1000.0, log, lin)


def _mel_to_hz(m):
    m = np.asanyarray(m, dtype=np.float64)
    lin = m * (200.0 / 3)
    log = 1000.0 * np.exp((np.log(6.4) / 27.0) * (m - 15.0))
    return np.where(m >= 15.0, log, lin)


def mel_filterbank(rate: int, n_fft: int, n_mels: int) -> np.ndarray:
    """Slaney-style mel filter bank with area normalisation (as librosa)."""
    fft_freqs = np.linspace(0, rate / 2, n_fft // 2 + 1)
    mels = np.linspace(_hz_to_mel(0.0), _hz_to_mel(rate / 2), n_mels + 2)
    hz = _mel_to_hz(mels)
    fdiff = np.diff(hz)
    ramps = hz[:, None] - fft_freqs[None, :]
    lower = -ramps[:-2] / fdiff[:-1, None]
    upper = ramps[2:] / fdiff[1:, None]
    weights = np.maximum(0, np.minimum(lower, upper))
    weights *= (2.0 / (hz[2:n_mels + 2] - hz[:n_mels]))[:, None]
    return weights.astype(np.float32)


def log_mel(x: np.ndarray, fe: Frontend) -> np.ndarray:
    """Returns features with shape (features, frames)."""
    x = np.asarray(x, dtype=np.float32)
    x = np.append(x[:1], x[1:] - fe.preemph * x[:-1]) if fe.preemph else x
    pad = fe.n_fft // 2
    x = np.pad(x, (pad, pad), mode="constant")
    n = 1 + (len(x) - fe.n_fft) // fe.hop
    window = np.zeros(fe.n_fft, dtype=np.float32)
    off = (fe.n_fft - fe.win) // 2
    window[off:off + fe.win] = np.hanning(fe.win + 1)[:-1]
    idx = np.arange(fe.n_fft)[None, :] + fe.hop * np.arange(n)[:, None]
    spec = np.abs(np.fft.rfft(x[idx] * window, n=fe.n_fft)) ** 2
    mel = mel_filterbank(fe.sample_rate, fe.n_fft, fe.features) @ spec.T
    feats = np.log(mel + fe.log_guard)
    if fe.normalize == "per_feature":
        mean = feats.mean(axis=1, keepdims=True)
        std = feats.std(axis=1, keepdims=True) + 1e-5
        feats = (feats - mean) / std
    return feats.astype(np.float32)
