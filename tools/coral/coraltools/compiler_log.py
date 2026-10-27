"""Parses the console output of `edgetpu_compiler -s`."""
from __future__ import annotations

import re

_UNITS = {"B": 1, "KiB": 1024, "MiB": 1024**2, "GiB": 1024**3}


def _size(text: str) -> int | None:
    m = re.match(r"\s*([\d.]+)\s*(B|KiB|MiB|GiB)", text)
    return int(float(m.group(1)) * _UNITS[m.group(2)]) if m else None


def parse(log: str) -> dict:
    """Returns op counts, memory use and the per-operator status table.

    When every operation maps to the Edge TPU, the compiler does not print
    the "will run on" lines; then all operations count as TPU operations.
    """
    result: dict = {"compiler_version": None, "ops_total": None, "ops_tpu": None,
                    "ops_cpu": None, "subgraphs": None, "on_chip_bytes": None,
                    "off_chip_bytes": None, "operators": []}
    for line in log.splitlines():
        key, _, value = line.partition(":")
        key = key.strip()
        if line.startswith("Edge TPU Compiler version"):
            result["compiler_version"] = line.rsplit(" ", 1)[-1].strip()
        elif key == "On-chip memory used for caching model parameters":
            result["on_chip_bytes"] = _size(value)
        elif key == "Off-chip memory used for streaming uncached model parameters":
            result["off_chip_bytes"] = _size(value)
        elif key == "Number of Edge TPU subgraphs":
            result["subgraphs"] = int(value)
        elif key == "Total number of operations":
            result["ops_total"] = int(value)
        elif key == "Number of operations that will run on Edge TPU":
            result["ops_tpu"] = int(value)
        elif key == "Number of operations that will run on CPU":
            result["ops_cpu"] = int(value)
        else:
            m = re.match(r"^([A-Z][A-Z0-9_]+)\s+(\d+)\s+(\S.*)$", line.strip())
            if m:
                result["operators"].append(
                    {"op": m.group(1), "count": int(m.group(2)), "status": m.group(3).strip()})
    if result["ops_total"] is not None and result["ops_tpu"] is None:
        result["ops_tpu"], result["ops_cpu"] = result["ops_total"], 0
    total = result["ops_total"]
    result["tpu_share"] = result["ops_tpu"] / total if total else None
    return result
