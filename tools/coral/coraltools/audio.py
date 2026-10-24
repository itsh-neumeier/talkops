"""WAV input/output, resampling and a G.711 telephone channel (numpy).

TalkOps records calls as 8 kHz (narrowband) or 16 kHz stereo WAV. The
benchmark sends wideband test corpora through a simulated telephone channel:
300-3400 Hz band, 8 kHz, G.711 mu-law or A-law, then back to 16 kHz.
"""
from __future__ import annotations

import wave

import numpy as np


def read_wav(path: str, channel: int | None = None) -> tuple[np.ndarray, int]:
    """Reads PCM16 WAV as float32 in [-1, 1]. Mixes down unless `channel` is set."""
    with wave.open(path, "rb") as w:
        if w.getsampwidth() != 2:
            raise ValueError(f"{path}: only PCM16 WAV is supported")
        rate, channels = w.getframerate(), w.getnchannels()
        data = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float32) / 32768.0
    data = data.reshape(-1, channels)
    if channel is not None:
        return data[:, channel].copy(), rate
    return data.mean(axis=1), rate


def write_wav(path: str, samples: np.ndarray, rate: int) -> None:
    pcm = np.clip(np.round(samples * 32767.0), -32768, 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        w.writeframes(pcm.tobytes())


def _lowpass(cutoff: float, taps: int = 101) -> np.ndarray:
    """Windowed-sinc low-pass; `cutoff` is relative to the sample rate (0..0.5)."""
    n = np.arange(taps) - (taps - 1) / 2
    h = 2 * cutoff * np.sinc(2 * cutoff * n) * np.blackman(taps)
    return h / h.sum()


def resample(x: np.ndarray, src: int, dst: int) -> np.ndarray:
    """Rational resampling by up-sampling, filtering and down-sampling."""
    if src == dst:
        return x.astype(np.float32)
    g = np.gcd(src, dst)
    up, down = dst // g, src // g
    y = np.zeros(len(x) * up, dtype=np.float64)
    y[::up] = x
    h = _lowpass(0.5 / max(up, down), taps=64 * max(up, down) + 1) * up
    y = np.convolve(y, h, mode="same")[::down]
    return y.astype(np.float32)


def bandpass(x: np.ndarray, rate: int, low: float = 300.0, high: float = 3400.0) -> np.ndarray:
    hp = -_lowpass(low / rate)
    hp[len(hp) // 2] += 1.0
    y = np.convolve(x, _lowpass(high / rate), mode="same")
    return np.convolve(y, hp, mode="same").astype(np.float32)


MU = 255.0


def mulaw_roundtrip(x: np.ndarray) -> np.ndarray:
    """Quantises to 8-bit mu-law codes and back (G.711 shape, 256 levels)."""
    x = np.clip(x, -1.0, 1.0)
    y = np.sign(x) * np.log1p(MU * np.abs(x)) / np.log1p(MU)
    code = np.round((y + 1) / 2 * 255)
    y = code / 255 * 2 - 1
    return (np.sign(y) * np.expm1(np.abs(y) * np.log1p(MU)) / MU).astype(np.float32)


A = 87.6


def alaw_roundtrip(x: np.ndarray) -> np.ndarray:
    x = np.clip(x, -1.0, 1.0)
    ax = np.abs(x)
    small = ax < 1 / A
    y = np.where(small, A * ax / (1 + np.log(A)), (1 + np.log(np.maximum(A * ax, 1e-12))) / (1 + np.log(A)))
    y = np.sign(x) * y
    code = np.round((y + 1) / 2 * 255)
    y = code / 255 * 2 - 1
    ay = np.abs(y)
    small = ay < 1 / (1 + np.log(A))
    out = np.where(small, ay * (1 + np.log(A)) / A, np.exp(ay * (1 + np.log(A)) - 1) / A)
    return (np.sign(y) * out).astype(np.float32)


def telephone(x: np.ndarray, rate: int, codec: str = "pcmu", out_rate: int = 16000) -> np.ndarray:
    """Simulates a narrowband G.711 call leg and returns audio at `out_rate`."""
    y = resample(x, rate, 8000)
    y = bandpass(y, 8000)
    y = mulaw_roundtrip(y) if codec == "pcmu" else alaw_roundtrip(y)
    return resample(y, 8000, out_rate)
