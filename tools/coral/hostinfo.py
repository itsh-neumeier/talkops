#!/usr/bin/env python3
"""Prints host, runtime and Edge TPU device facts as JSON.

Run it on the benchmark host before each measurement and keep the output
with the results. It reads only sysfs, /proc and /dev and loads no driver.

    python3 -I tools/coral/hostinfo.py > results/host.json
"""
import glob
import json
import os
import platform
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import devices  # noqa: E402


def cpu_model() -> str | None:
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return None


def libraries() -> dict:
    found = {}
    for pattern in ("libedgetpu.so*", "libtensorflowlite_c.so*", "libtensorflow-lite.so*"):
        for d in ("/usr/lib", "/usr/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu", "/usr/local/lib"):
            for p in glob.glob(os.path.join(d, pattern)):
                found[p] = os.path.realpath(p)
    return found


def python_runtime() -> dict:
    out = {}
    for name in ("ai_edge_litert", "tflite_runtime", "numpy"):
        try:
            mod = __import__(name)
            out[name] = getattr(mod, "__version__", "present")
        except ImportError:
            out[name] = None
    return out


def main() -> None:
    info = {
        "kernel": platform.release(),
        "machine": platform.machine(),
        "cpu": cpu_model(),
        "cpu_count": os.cpu_count(),
        "python": sys.version.split()[0],
        "virtualisation": Path("/run/systemd/container").read_text().strip()
        if Path("/run/systemd/container").exists() else None,
        "apex_module": Path("/sys/module/apex").is_dir(),
        "gasket_module": Path("/sys/module/gasket").is_dir(),
        "devices": devices.discover(),
        "libraries": libraries(),
        "python_packages": python_runtime(),
    }
    json.dump(info, sys.stdout, indent=2)
    print()


if __name__ == "__main__":
    main()
