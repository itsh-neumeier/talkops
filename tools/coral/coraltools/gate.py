"""Evaluates the Phase 0 gate from benchmark result files.

Gate (user requirement, ADR draft in docs/research/coral-stt-benchmark.md):
1. One call (two channels) runs in real time on one TPU.
2. Live latency per segment is below about 1.5 s (p95 is used).
3. Host CPU load is clearly below whisper.cpp `base` (factor 0.5 or less).
4. The text is readable as a live preview (manual rating, WER is a guide).
"""
from __future__ import annotations

LATENCY_P95_MAX = 1.5
CPU_RATIO_MAX = 0.5
WER_GUIDE_MAX = 0.35


def _status(ok: bool | None) -> str:
    return {True: "PASS", False: "FAIL", None: "PENDING"}[ok]


def _per_channel(result: dict) -> float | None:
    cpu = result.get("cpu_cores")
    channels = result.get("channels") or 2 * result.get("calls", 1)
    return cpu / channels if cpu is not None and channels else None


def _round(v: float | None) -> float | None:
    return None if v is None else round(v, 3)


def evaluate(coral_live: dict, whisper_live: dict | None, readable: bool | None) -> dict:
    """`coral_live` and `whisper_live` are outputs of bench_live.py."""
    checks = []
    realtime = coral_live.get("calls", 0) >= 1 and coral_live.get("backlog_secs", 1e9) <= 1.0
    checks.append(("Real time: >= 1 call (2 channels) on one TPU, backlog <= 1 s",
                   f"{coral_live.get('calls')} call(s), backlog {coral_live.get('backlog_secs')} s",
                   realtime))
    p95 = coral_live.get("latency_p95")
    checks.append((f"Segment latency p95 < {LATENCY_P95_MAX} s",
                   f"p50 {coral_live.get('latency_p50')} s, p95 {p95} s",
                   None if p95 is None else p95 < LATENCY_P95_MAX))
    # CPU per channel, so runs with a different number of calls compare.
    cpu = _per_channel(coral_live)
    base = _per_channel(whisper_live) if whisper_live else None
    ratio = cpu / base if cpu is not None and base else None
    checks.append((f"Host CPU per channel <= {CPU_RATIO_MAX} x whisper.cpp base",
                   f"{_round(cpu)} vs {_round(base)} cores per channel (ratio {_round(ratio)})",
                   None if ratio is None else ratio <= CPU_RATIO_MAX))
    wer = coral_live.get("wer")
    checks.append(("Readable as live preview (manual rating)",
                   f"WER {None if wer is None else round(wer, 3)} (guide <= {WER_GUIDE_MAX})",
                   readable))
    states = [c[2] for c in checks]
    overall = False if False in states else (None if None in states else True)
    return {"checks": [{"criterion": c, "value": v, "status": _status(s)} for c, v, s in checks],
            "result": _status(overall)}


def markdown(evaluation: dict) -> str:
    lines = ["| Criterion | Measured | Status |", "| --- | --- | --- |"]
    for c in evaluation["checks"]:
        lines.append(f"| {c['criterion']} | {c['value']} | {c['status']} |")
    lines.append("")
    lines.append(f"Gate result: **{evaluation['result']}**")
    return "\n".join(lines)


def replace_section(document: str, begin: str, end: str, body: str) -> str:
    """Replaces the text between two marker lines (markers stay)."""
    a, b = document.index(begin) + len(begin), document.index(end)
    return document[:a] + "\n" + body.rstrip() + "\n" + document[b:]
