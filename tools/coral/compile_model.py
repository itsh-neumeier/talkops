#!/usr/bin/env python3
"""Compiles an int8 TFLite model with edgetpu_compiler and records the op mapping.

    python3 -I tools/coral/compile_model.py models/de/model.tflite

Writes model_edgetpu.tflite, model_edgetpu.log (compiler output) and
model_edgetpu.compile.json (parsed: TPU share of ops, on-chip and off-chip
parameter memory, subgraphs, operators that stay on the CPU). The compiler
is an x86-64 Debian package from the Coral repository; it runs offline.
Use --segments N to split a large model over N TPUs (pipelining).
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import compiler_log  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("model")
    ap.add_argument("--compiler", default="edgetpu_compiler")
    ap.add_argument("--segments", type=int, default=1)
    args = ap.parse_args()

    model = Path(args.model)
    cmd = [args.compiler, "-s", "-o", str(model.parent), str(model)]
    if args.segments > 1:
        cmd[1:1] = ["--num_segments", str(args.segments)]
    run = subprocess.run(cmd, capture_output=True, text=True)
    log = run.stdout + run.stderr
    base = model.with_name(model.stem + "_edgetpu")
    base.with_suffix(".log").write_text(log)
    if run.returncode != 0:
        sys.exit(f"edgetpu_compiler failed ({run.returncode}); see {base.with_suffix('.log')}")
    result = compiler_log.parse(log)
    result["model"] = model.name
    Path(str(base) + ".compile.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({k: result[k] for k in ("ops_total", "ops_tpu", "tpu_share", "on_chip_bytes",
                                              "off_chip_bytes", "subgraphs")}))
    cpu_ops = [o for o in result["operators"] if not o["status"].startswith("Mapped to Edge TPU")]
    for o in cpu_ops:
        print(f"CPU: {o['op']} x{o['count']}: {o['status']}", file=sys.stderr)


if __name__ == "__main__":
    main()
