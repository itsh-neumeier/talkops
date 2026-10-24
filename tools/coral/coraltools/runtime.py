"""Runs compiled CTC and language ID models on the Edge TPU or the CPU.

The Python runtime is either `ai_edge_litert` (LiteRT, the successor of
`tflite_runtime`) or `tflite_runtime`. The Edge TPU needs `libedgetpu.so.1`
built for a matching TensorFlow Lite version; hostinfo.py records both.

Audio is processed in fixed windows, as the Edge TPU needs fixed tensor
shapes. `Stream` runs each window as soon as its audio is complete, so at
the end of a turn only the last window or two remain. This is the same
schedule that the coral-stt service uses for the live preview.
"""
from __future__ import annotations

import json
import threading
import time
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

from . import ctc
from .features import Frontend, log_mel


def _interpreter_module():
    try:
        from ai_edge_litert import interpreter as mod  # type: ignore
        return mod
    except ImportError:
        from tflite_runtime import interpreter as mod  # type: ignore
        return mod


def make_interpreter(model: str, device: str | None, threads: int = 1):
    """`device` is a libedgetpu device string ("usb:0", "pci:0") or None for CPU."""
    mod = _interpreter_module()
    delegates = [mod.load_delegate("libedgetpu.so.1", {"device": device})] if device else []
    it = mod.Interpreter(model_path=model, experimental_delegates=delegates, num_threads=threads)
    it.allocate_tensors()
    return it


def _quantize(x: np.ndarray, detail: dict) -> np.ndarray:
    if detail["dtype"] in (np.int8, np.uint8):
        scale, zero = detail["quantization"]
        info = np.iinfo(detail["dtype"])
        return np.clip(np.round(x / scale + zero), info.min, info.max).astype(detail["dtype"])
    return x.astype(detail["dtype"])


def _dequantize(y: np.ndarray, detail: dict) -> np.ndarray:
    if detail["dtype"] in (np.int8, np.uint8):
        scale, zero = detail["quantization"]
        return (y.astype(np.float32) - zero) * scale
    return y.astype(np.float32)


def meta_path(model: str) -> Path:
    """model_edgetpu.tflite and model.tflite both use model.json."""
    p = Path(model)
    return p.with_name(p.stem.removesuffix("_edgetpu") + ".json")


class Tpu:
    """One Edge TPU (or the CPU). One request runs at a time on a device."""

    def __init__(self, device: str | None):
        self.device = device
        self.lock = threading.Lock()
        self.busy_secs = 0.0


@dataclass
class Model:
    path: str
    tpu: Tpu
    meta: dict = field(default_factory=dict)

    def __post_init__(self) -> None:
        if not self.meta:
            self.meta = json.loads(meta_path(self.path).read_text())
        self.it = make_interpreter(self.path, self.tpu.device)
        self.inp = self.it.get_input_details()[0]
        self.out = self.it.get_output_details()[0]
        self.fe = Frontend.from_meta(self.meta)
        self.window = int(self.meta["window_frames"])
        # "nct": (1, features, frames); "ntc": (1, frames, features); "nhwc": (1, features, frames, 1).
        self.layout = self.meta.get("input_layout", "nct")

    @property
    def window_samples(self) -> int:
        return (self.window - 1) * self.fe.hop

    def features(self, samples: np.ndarray) -> np.ndarray:
        feats = log_mel(samples, self.fe)[:, : self.window]
        if feats.shape[1] < self.window:
            feats = np.pad(feats, ((0, 0), (0, self.window - feats.shape[1])))
        return feats

    def run(self, feats: np.ndarray) -> np.ndarray:
        if self.layout == "ntc":
            x = feats.T[None]
        elif self.layout == "nhwc":
            x = feats[None, :, :, None]
        else:
            x = feats[None]
        x = _quantize(x, self.inp)
        with self.tpu.lock:
            t = time.perf_counter()
            self.it.set_tensor(self.inp["index"], x)
            self.it.invoke()
            y = self.it.get_tensor(self.out["index"])
            self.tpu.busy_secs += time.perf_counter() - t
        return _dequantize(y, self.out)[0]


class CtcModel(Model):
    def __post_init__(self) -> None:
        super().__post_init__()
        self.overlap = int(self.meta.get("overlap_frames", self.window // 4))
        self.subsampling = int(self.meta.get("subsampling", 2))
        # NeMo CTC: the blank is the class after the last label.
        self.labels = list(self.meta["labels"]) + ["<blank>"]
        self.blank = len(self.labels) - 1
        self.language = self.meta.get("language")

    def window_logits(self, samples: np.ndarray) -> np.ndarray:
        y = self.run(self.features(samples))
        return y if y.shape[-1] == len(self.labels) else y.T

    def stream(self) -> "Stream":
        return Stream(self)


class LangId(Model):
    def __post_init__(self) -> None:
        super().__post_init__()
        self.classes = self.meta["classes"]

    def classify(self, samples: np.ndarray) -> tuple[str, float]:
        p = self.run(self.features(samples)).reshape(-1)
        k = int(p.argmax())
        return self.classes[k], float(p[k])


class Stream:
    """Incremental CTC over one turn. `push` returns nothing; `finish` the text."""

    def __init__(self, model: CtcModel):
        self.m = model
        self.samples = np.zeros(0, dtype=np.float32)
        self.outputs: list[np.ndarray] = []
        self.step = (model.window - model.overlap) * model.fe.hop

    def _run_ready(self, final: bool) -> None:
        while True:
            start = len(self.outputs) * self.step
            end = start + self.m.window_samples
            if end > len(self.samples) and not (final and start < len(self.samples)):
                return
            self.outputs.append(self.m.window_logits(self.samples[start:end]))
            if end >= len(self.samples):
                return

    def push(self, samples: np.ndarray) -> None:
        self.samples = np.concatenate([self.samples, samples])
        self._run_ready(final=False)

    def finish(self) -> tuple[str, list, float]:
        if len(self.samples) == 0:
            return "", [], float("-inf")
        self._run_ready(final=True)
        total = self.m.fe.frames(len(self.samples))
        rows = ctc.stitch(self.outputs, self.m.window, self.m.overlap, total, self.m.subsampling)
        frames = np.stack(rows) if rows else np.zeros((0, len(self.m.labels)), dtype=np.float32)
        ids = frames.argmax(axis=1)
        frame_secs = self.m.fe.window_stride * self.m.subsampling
        text, words = ctc.collapse(ids.tolist(), self.m.labels, self.m.blank, frame_secs)
        return text, words, confidence(frames, ids, self.m.blank, self.m.meta.get("log_probs", True))


def confidence(frames: np.ndarray, ids: np.ndarray, blank: int, log_probs: bool) -> float:
    """numpy version of ctc.confidence."""
    if not log_probs:
        m = frames.max(axis=1, keepdims=True)
        frames = frames - (m + np.log(np.exp(frames - m).sum(axis=1, keepdims=True)))
    keep = ids != blank
    if not keep.any():
        return float("-inf")
    return float(frames[keep, ids[keep]].mean())


class Recognizer:
    """Selects the language of a turn: fixed, by CTC confidence, or by classifier.

    - fixed: one model.
    - confidence: every model runs on every window (N times the TPU load);
      the text with the higher confidence wins.
    - classifier: the language ID model runs on the first window of the turn;
      then only the selected CTC model runs.
    """

    def __init__(self, models: dict[str, CtcModel], mode: str, langid: LangId | None = None,
                 preferred: str | None = None):
        self.models, self.mode, self.langid = models, mode, langid
        self.preferred = preferred or next(iter(models))

    def turn(self) -> "Turn":
        return Turn(self)


class Turn:
    def __init__(self, r: Recognizer):
        self.r = r
        self.buffer = np.zeros(0, dtype=np.float32)
        self.language: str | None = None
        self.streams: dict[str, Stream] = {}
        if r.mode == "fixed" or len(r.models) == 1:
            self._select(r.preferred)
        elif r.mode == "confidence":
            self.streams = {k: m.stream() for k, m in r.models.items()}

    def _select(self, language: str) -> None:
        self.language = language if language in self.r.models else self.r.preferred
        self.streams = {self.language: self.r.models[self.language].stream()}

    def push(self, samples: np.ndarray) -> None:
        if not self.streams:  # classifier mode, language not chosen yet
            self.buffer = np.concatenate([self.buffer, samples])
            if len(self.buffer) >= self.r.langid.window_samples:
                self._select(self.r.langid.classify(self.buffer)[0])
                samples, self.buffer = self.buffer, np.zeros(0, dtype=np.float32)
            else:
                return
        for s in self.streams.values():
            s.push(samples)

    def finish(self) -> dict:
        if not self.streams:
            if len(self.buffer) == 0:
                return {"text": "", "language": None}
            self._select(self.r.langid.classify(self.buffer)[0])
            self.streams[self.language].push(self.buffer)
        results = {k: s.finish() for k, s in self.streams.items()}
        best = max(results, key=lambda k: results[k][2])
        text, words, conf = results[best]
        return {"text": text, "words": words, "confidence": conf, "language": best}
