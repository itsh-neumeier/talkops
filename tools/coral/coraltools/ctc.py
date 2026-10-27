"""CTC greedy decoding, window stitching and confidence.

The Edge TPU runs a model with a fixed input window. A turn longer than one
window is cut into overlapping windows. `stitch` keeps the centre frames of
each window, so every output frame has left and right context.
"""
from __future__ import annotations

import math
from typing import Sequence


def windows(total: int, size: int, overlap: int) -> list[tuple[int, int]]:
    """Input frame ranges [start, end) that cover `total` frames.

    The last window can end after `total`; the caller pads it with silence.
    """
    if size <= 0 or not 0 <= overlap < size:
        raise ValueError("need size > 0 and 0 <= overlap < size")
    step = size - overlap
    out = [(0, size)]
    while out[-1][1] < total:
        start = out[-1][0] + step
        out.append((start, start + size))
    return out


def stitch(outputs: Sequence[Sequence[Sequence[float]]], size: int, overlap: int,
           total: int, subsampling: int) -> list[Sequence[float]]:
    """Joins per-window output frames into one sequence for `total` input frames.

    `outputs[i]` are the output frames of window i (input frames / subsampling).
    Each window gives the frames from half the overlap after its start (not
    for the first window) to half the overlap before its end (not for the
    last window).
    """
    half = overlap // 2
    joined: list[Sequence[float]] = []
    for i, (start, end) in enumerate(windows(total, size, overlap)):
        if i >= len(outputs):
            break
        keep_from = 0 if i == 0 else start + half
        keep_to = min(end if i == len(outputs) - 1 else end - (overlap - half), total)
        first = (keep_from - start) // subsampling
        last = math.ceil((keep_to - start) / subsampling)
        joined.extend(outputs[i][first:last])
    return joined


def argmax(row: Sequence[float]) -> int:
    best = 0
    for k in range(1, len(row)):
        if row[k] > row[best]:
            best = k
    return best


def greedy(frames: Sequence[Sequence[float]], labels: Sequence[str], blank: int,
           frame_secs: float = 0.0) -> tuple[str, list[tuple[str, float, float]]]:
    """Greedy CTC: collapse repeats, drop blanks.

    Returns the text and word timings (word, start, end) in seconds when
    `frame_secs` is set. A space label (" ") separates words; models with
    sentencepiece labels use "▁" as the word start marker.
    """
    return collapse([argmax(row) for row in frames], labels, blank, frame_secs)


def collapse(ids: Sequence[int], labels: Sequence[str], blank: int,
             frame_secs: float = 0.0) -> tuple[str, list[tuple[str, float, float]]]:
    """Greedy CTC on the best label id of each frame (see `greedy`)."""
    out: list[tuple[str, int]] = []
    prev = blank
    for t, k in enumerate(ids):
        if k != blank and k != prev:
            out.append((labels[k], t))
        prev = k
    words: list[tuple[str, float, float]] = []
    cur, start, end = "", None, 0
    for token, t in out:
        piece_start = token.startswith("▁") or token == " "
        if piece_start and cur:
            words.append((cur, start * frame_secs, (end + 1) * frame_secs))
            cur, start = "", None
        piece = token.replace("▁", "").replace(" ", "")
        if piece:
            if start is None:
                start = t
            cur += piece
            end = t
    if cur:
        words.append((cur, start * frame_secs, (end + 1) * frame_secs))
    return " ".join(w for w, _, _ in words), words


def log_softmax(row: Sequence[float]) -> list[float]:
    m = max(row)
    z = math.log(sum(math.exp(v - m) for v in row)) + m
    return [v - z for v in row]


def confidence(frames: Sequence[Sequence[float]], blank: int, logits: bool = True) -> float:
    """Mean log probability of the best non-blank label over non-blank frames.

    Used for language selection without a classifier: run the DE and the EN
    model and keep the text with the higher confidence. Returns -inf when no
    frame is non-blank.
    """
    total, count = 0.0, 0
    for row in frames:
        lp = log_softmax(row) if logits else list(row)
        k = argmax(lp)
        if k != blank:
            total += lp[k]
            count += 1
    return total / count if count else float("-inf")
