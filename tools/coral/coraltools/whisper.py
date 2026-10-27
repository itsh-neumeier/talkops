"""whisper.cpp baseline (the local `base` model of the TalkOps media worker)."""
from __future__ import annotations

import json
import os
import resource
import subprocess
import tempfile
import time

import numpy as np

from . import audio


class Whisper:
    def __init__(self, cli: str, model: str, threads: int):
        self.cli, self.model, self.threads = cli, model, threads
        self.cpu_secs = 0.0

    def transcribe(self, samples: np.ndarray, language: str | None) -> tuple[str, float]:
        """Returns the text and the wall time. Adds child CPU time to `cpu_secs`."""
        with tempfile.TemporaryDirectory() as tmp:
            wav = os.path.join(tmp, "in.wav")
            audio.write_wav(wav, samples, 16000)
            cmd = [self.cli, "-m", self.model, "-f", wav, "-t", str(self.threads), "-np",
                   "-oj", "-of", os.path.join(tmp, "out"), "-l", language or "auto"]
            before = resource.getrusage(resource.RUSAGE_CHILDREN)
            t = time.perf_counter()
            subprocess.run(cmd, check=True, capture_output=True)
            wall = time.perf_counter() - t
            after = resource.getrusage(resource.RUSAGE_CHILDREN)
            self.cpu_secs += (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime)
            with open(os.path.join(tmp, "out.json"), encoding="utf-8") as f:
                data = json.load(f)
        text = " ".join(s.get("text", "").strip() for s in data.get("transcription", []))
        return text.strip(), wall
