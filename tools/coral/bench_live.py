#!/usr/bin/env python3
"""Live preview benchmark: N simultaneous calls (2 channels each) in real time.

Each channel plays manifest utterances at real-time speed, separated by
1.5 s of silence, and uses the turn detector of the TalkOps realtime client
(coraltools/turns.py). The channels of one call start with an offset, so
the parties speak partly at the same time, as in a real call.

Coral: windows run as soon as their audio is complete; at the end of a
turn, the rest runs. whisper.cpp: the turn runs after the commit (the
`base` model with --whisper-threads, as the worker does for the preview).

Latency = time from the end of the turn (commit) to the final text.
Backlog = how far the slowest channel is behind real time at the end.

    python3 -I tools/coral/bench_live.py data/tel/manifest.jsonl --calls 1 \\
        --model de=... --model en=... --lang-mode classifier --langid ... \\
        --device usb:0 --minutes 5 --out results/live-usb-1call.json
"""
import argparse
import json
import os
import statistics
import sys
import threading
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import audio, engines, manifest, turns  # noqa: E402
from coraltools.wer import ErrorRate  # noqa: E402

RATE = 16000
CHUNK_SECS = 0.2  # the worker polls the growing recording every 200 ms
GAP_SECS = 1.5
SCORE_LOCK = threading.Lock()


def channel_audio(items: list, offset: int, secs: float) -> tuple[np.ndarray, list]:
    """Concatenates utterances from `offset` until `secs` of audio."""
    parts, refs, total, i = [], [], 0.0, offset
    while total < secs:
        item = items[i % len(items)]
        x, rate = audio.read_wav(item["path"])
        x = audio.resample(x, rate, RATE)
        refs.append((total, total + len(x) / RATE, item))
        parts += [x, np.zeros(int(GAP_SECS * RATE), dtype=np.float32)]
        total += len(x) / RATE + GAP_SECS
        i += 1
    return np.concatenate(parts), refs


def run_channel(args, samples, refs, recognizer, whisper, start_at, out, errors) -> None:
    det = turns.Turns(RATE)
    chunk = int(CHUNK_SECS * RATE)
    turn, buf = None, []
    latencies, texts, lag = [], [], 0.0
    pos = 0

    def finish(commit_due: float, start: float, end: float) -> None:
        nonlocal turn, buf
        if args.engine == "coral":
            r = turn.finish() if turn else {"text": "", "language": None}
        else:
            seg = np.concatenate(buf) if buf else np.zeros(0, dtype=np.float32)
            fixed = args.language or args.preferred
            text, _ = whisper.transcribe(seg, fixed if args.lang_mode == "fixed" else None)
            r = {"text": text, "language": None}
        latencies.append(time.perf_counter() - commit_due)
        texts.append((start, end, r))
        turn, buf = None, []

    while pos < len(samples):
        due = start_at + (pos + chunk) / RATE
        now = time.perf_counter()
        if now < due:
            time.sleep(due - now)
        lag = max(lag, time.perf_counter() - due)
        for action in det.push(samples[pos:pos + chunk].tolist()):
            if isinstance(action, turns.Append):
                x = np.asarray(action.samples, dtype=np.float32)
                if args.engine == "coral":
                    if turn is None:
                        turn = recognizer.turn()
                    turn.push(x)
                else:
                    buf.append(x)
            elif isinstance(action, turns.Commit):
                finish(due, action.start, action.end)
            else:
                turn, buf = None, []
        pos += chunk
    action = det.finish()
    if isinstance(action, turns.Commit):
        finish(time.perf_counter(), action.start, action.end)
    final_lag = time.perf_counter() - (start_at + len(samples) / RATE)
    # Score: join hypothesis turns and references that overlap in time.
    with SCORE_LOCK:
        for s, e, item in refs:
            hyp = " ".join(r["text"] for ts, te, r in texts if ts < e and te > s)
            errors.setdefault(item["language"], ErrorRate()).add(item["text"], hyp)
    out.append({"latencies": latencies, "max_lag": lag, "final_lag": final_lag,
                "examples": [{"start": round(s, 2), "end": round(e, 2), **{k: r.get(k) for k in ("text", "language")}}
                             for s, e, r in texts[:6]]})


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("manifest")
    ap.add_argument("--calls", type=int, default=1)
    ap.add_argument("--minutes", type=float, default=3.0)
    ap.add_argument("--language", help="use only lines of this language")
    ap.add_argument("--out", required=True)
    engines.add_arguments(ap)
    args = ap.parse_args()

    items = manifest.load(args.manifest, args.language)
    if not items:
        raise SystemExit("empty manifest")
    tpu = recognizer = whisper = None
    if args.engine == "coral":
        tpu, models, _, make = engines.coral(args)
        recognizer = make(args.preferred or (args.language or next(iter(models))))
    else:
        whisper = engines.whisper(args)
    secs = args.minutes * 60
    channels = []
    for c in range(args.calls * 2):
        x, refs = channel_audio(items, offset=c * 7, secs=secs)
        # Second party starts 2 s later: overlapping speech in each call.
        x = np.concatenate([np.zeros(int(2.0 * RATE * (c % 2)), dtype=np.float32), x])
        refs = [(s + 2.0 * (c % 2), e + 2.0 * (c % 2), it) for s, e, it in refs]
        channels.append((x, refs))

    results: list = []
    errors: dict = {}
    cpu0, t0 = os.times(), time.perf_counter()
    start_at = t0 + 0.5
    threads = [threading.Thread(target=run_channel,
                                args=(args, x, refs, recognizer, whisper, start_at, results, errors))
               for x, refs in channels]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    wall = time.perf_counter() - t0
    cpu1 = os.times()
    cpu = (cpu1.user - cpu0.user) + (cpu1.system - cpu0.system) + (whisper.cpu_secs if whisper else 0.0)
    lat = sorted(v for r in results for v in r["latencies"])
    total = ErrorRate()
    for e in errors.values():
        total.word_edits += e.word_edits
        total.words += e.words
    result = {
        "kind": "live",
        "engine": args.engine,
        "device": args.device if args.engine == "coral" else f"cpu x{args.whisper_threads}",
        "lang_mode": args.lang_mode,
        "calls": args.calls,
        "channels": len(channels),
        "audio_secs_per_channel": round(secs, 1),
        "segments": len(lat),
        "latency_p50": round(statistics.median(lat), 3) if lat else None,
        "latency_p95": round(lat[int(0.95 * (len(lat) - 1))], 3) if lat else None,
        "latency_max": round(lat[-1], 3) if lat else None,
        "backlog_secs": round(max(max(r["final_lag"] for r in results), 0.0), 3),
        "max_lag_secs": round(max(r["max_lag"] for r in results), 3),
        "cpu_cores": round(cpu / wall, 3),
        "tpu_busy_share": round(tpu.busy_secs / wall, 3) if tpu else None,
        "wer": round(total.wer, 4) if total.wer is not None else None,
        "wer_by_language": {k: round(v.wer, 4) for k, v in errors.items() if v.wer is not None},
        "examples": results[0]["examples"] if results else [],
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(result, indent=2, ensure_ascii=False))
    print(json.dumps({k: v for k, v in result.items() if k != "examples"}))


if __name__ == "__main__":
    main()
