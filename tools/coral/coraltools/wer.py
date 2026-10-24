"""Word and character error rates with a simple text normalisation."""
from __future__ import annotations

import re
import unicodedata

_PUNCT = re.compile(r"[^\w\s']", re.UNICODE)
_SPACE = re.compile(r"\s+")


def normalize(text: str) -> str:
    """Lowercase, NFC, no punctuation, single spaces.

    Numbers are not expanded. Use references that spell numbers the same way
    as the model output (the CTC models write words, Whisper often digits);
    the report states this limitation.
    """
    text = unicodedata.normalize("NFC", text).lower().replace("’", "'")
    text = _PUNCT.sub(" ", text)
    return _SPACE.sub(" ", text).strip()


def edit_distance(ref: list, hyp: list) -> int:
    """Levenshtein distance between two token lists."""
    prev = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        cur = [i] + [0] * len(hyp)
        for j, h in enumerate(hyp, 1):
            cur[j] = min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (r != h))
        prev = cur
    return prev[-1]


class ErrorRate:
    """Accumulates edits and reference length over many utterances."""

    def __init__(self) -> None:
        self.word_edits = 0
        self.words = 0
        self.char_edits = 0
        self.chars = 0

    def add(self, reference: str, hypothesis: str) -> None:
        ref, hyp = normalize(reference), normalize(hypothesis)
        self.word_edits += edit_distance(ref.split(), hyp.split())
        self.words += len(ref.split())
        self.char_edits += edit_distance(list(ref), list(hyp))
        self.chars += len(ref)

    @property
    def wer(self) -> float | None:
        return self.word_edits / self.words if self.words else None

    @property
    def cer(self) -> float | None:
        return self.char_edits / self.chars if self.chars else None
