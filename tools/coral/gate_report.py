#!/usr/bin/env python3
"""Evaluates the Phase 0 gate and writes the result tables into the report.

    python3 -I tools/coral/gate_report.py results/ \\
        --coral-live results/live-usb-1call.json \\
        --whisper-live results/live-whisper-base.json --readable yes

Reads every *.json result in the directory (compile, file and live results)
and replaces the sections between the result markers in
docs/research/coral-stt-benchmark.md. Use --readable after you read the
example transcripts in the live results: is the text usable as a live preview?
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import gate  # noqa: E402

REPORT = Path(__file__).resolve().parents[2] / "docs/research/coral-stt-benchmark.md"


def fmt(v) -> str:
    if v is None:
        return "-"
    if isinstance(v, dict):
        return ", ".join(f"{k} {fmt(x)}" for k, x in v.items()) or "-"
    if isinstance(v, float):
        return f"{v:.3g}"
    return str(v)


def table(rows: list[dict], cols: list[tuple[str, str]]) -> str:
    out = ["| " + " | ".join(h for h, _ in cols) + " |", "|" + " --- |" * len(cols)]
    for r in rows:
        out.append("| " + " | ".join(fmt(r.get(k)) for _, k in cols) + " |")
    return "\n".join(out)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("results")
    ap.add_argument("--coral-live", required=True)
    ap.add_argument("--whisper-live")
    ap.add_argument("--readable", choices=["yes", "no"])
    ap.add_argument("--report", default=str(REPORT))
    args = ap.parse_args()

    compiled, files, lives = [], [], []
    for p in sorted(Path(args.results).glob("**/*.json")):
        data = json.loads(p.read_text())
        data["file"] = p.name
        if "ops_total" in data:
            compiled.append(data)
        elif data.get("kind") == "file":
            files.append(data)
        elif data.get("kind") == "live":
            lives.append(data)

    doc = Path(args.report).read_text()
    doc = gate.replace_section(doc, "<!-- results:compile:begin -->", "<!-- results:compile:end -->", table(
        compiled, [("Model", "model"), ("Ops", "ops_total"), ("TPU ops", "ops_tpu"), ("TPU share", "tpu_share"),
                   ("On-chip bytes", "on_chip_bytes"), ("Off-chip bytes", "off_chip_bytes"),
                   ("Subgraphs", "subgraphs")]))
    doc = gate.replace_section(doc, "<!-- results:file:begin -->", "<!-- results:file:end -->", table(
        files, [("Result", "file"), ("Engine", "engine"), ("Device", "device"), ("Language", "lang_mode"),
                ("WER", "wer"), ("CER", "cer"), ("RTF", "rtf"), ("CPU s/audio s", "cpu_secs_per_audio_sec"),
                ("Lang ID", "langid_accuracy")]))
    doc = gate.replace_section(doc, "<!-- results:live:begin -->", "<!-- results:live:end -->", table(
        lives, [("Result", "file"), ("Engine", "engine"), ("Device", "device"), ("Calls", "calls"),
                ("p50 s", "latency_p50"), ("p95 s", "latency_p95"), ("Backlog s", "backlog_secs"),
                ("CPU cores", "cpu_cores"), ("TPU busy", "tpu_busy_share"), ("WER", "wer")]))
    coral = json.loads(Path(args.coral_live).read_text())
    whisper = json.loads(Path(args.whisper_live).read_text()) if args.whisper_live else None
    readable = None if args.readable is None else args.readable == "yes"
    evaluation = gate.evaluate(coral, whisper, readable)
    doc = gate.replace_section(doc, "<!-- results:gate:begin -->", "<!-- results:gate:end -->",
                               gate.markdown(evaluation))
    Path(args.report).write_text(doc)
    print(evaluation["result"])


if __name__ == "__main__":
    main()
