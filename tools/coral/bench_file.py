#!/usr/bin/env python3
"""File transcription benchmark: WER/CER, real-time factor, host CPU, language ID.

Measures the use case "recording and voicemail, first pass". Each manifest
line is transcribed as one file.

    python3 -I tools/coral/bench_file.py data/tel/manifest.jsonl \\
        --model de=models/de/model_edgetpu.tflite --model en=models/en/model_edgetpu.tflite \\
        --lang-mode classifier --langid models/langid/model_edgetpu.tflite \\
        --device usb:0 --out results/file-usb-coral.json
    python3 -I tools/coral/bench_file.py data/tel/manifest.jsonl --engine whisper \\
        --whisper-threads 4 --out results/file-whisper-base.json
"""
import argparse
import json
import os
import statistics
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import audio, engines, manifest  # noqa: E402
from coraltools.wer import ErrorRate  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("manifest")
    ap.add_argument("--language", help="use only lines of this language")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--out", required=True)
    ap.add_argument("--samples", type=int, default=5, help="example transcripts to keep per language")
    engines.add_arguments(ap)
    args = ap.parse_args()

    items = manifest.load(args.manifest, args.language, args.limit)
    if args.engine == "coral":
        tpu, models, _, recognizer = engines.coral(args)
    else:
        w = engines.whisper(args)
    rates: dict[str, ErrorRate] = {}
    examples: dict[str, list] = {}
    lang_ok = lang_total = 0
    audio_secs = 0.0
    walls = []
    cpu0, t0 = os.times(), time.perf_counter()
    for item in items:
        x, rate = audio.read_wav(item["path"])
        x = audio.resample(x, rate, 16000)
        audio_secs += len(x) / 16000
        t = time.perf_counter()
        if args.engine == "coral":
            turn = recognizer(item["language"]).turn()
            turn.push(x)
            r = turn.finish()
            text, lang = r["text"], r["language"]
        else:
            lang = item["language"] if args.lang_mode == "fixed" else None
            text, _ = w.transcribe(x, lang)
            lang = lang or item["language"]
        walls.append(time.perf_counter() - t)
        rates.setdefault(item["language"], ErrorRate()).add(item["text"], text)
        if args.lang_mode != "fixed":
            lang_total += 1
            lang_ok += lang == item["language"]
        ex = examples.setdefault(item["language"], [])
        if len(ex) < args.samples:
            ex.append({"reference": item["text"], "hypothesis": text, "language": lang})
    wall = time.perf_counter() - t0
    cpu1 = os.times()
    cpu = (cpu1.user - cpu0.user) + (cpu1.system - cpu0.system)
    if args.engine == "whisper":
        cpu += w.cpu_secs
    result = {
        "kind": "file",
        "engine": args.engine,
        "device": args.device if args.engine == "coral" else f"cpu x{args.whisper_threads}",
        "lang_mode": args.lang_mode,
        "utterances": len(items),
        "audio_secs": round(audio_secs, 1),
        "rtf": round(sum(walls) / audio_secs, 4) if audio_secs else None,
        "latency_p50": round(statistics.median(walls), 3) if walls else None,
        "cpu_secs_per_audio_sec": round(cpu / audio_secs, 4) if audio_secs else None,
        "cpu_cores": round(cpu / wall, 3) if wall else None,
        "tpu_busy_share": round(tpu.busy_secs / wall, 3) if args.engine == "coral" and wall else None,
        "wer": {k: round(v.wer, 4) for k, v in rates.items() if v.wer is not None},
        "cer": {k: round(v.cer, 4) for k, v in rates.items() if v.cer is not None},
        "langid_accuracy": round(lang_ok / lang_total, 4) if lang_total else None,
        "examples": examples,
    }
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    Path(args.out).write_text(json.dumps(result, indent=2, ensure_ascii=False))
    print(json.dumps({k: v for k, v in result.items() if k != "examples"}))


if __name__ == "__main__":
    main()
