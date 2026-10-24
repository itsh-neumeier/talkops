"""Reads the manifest written by prepare_audio.py."""
from __future__ import annotations

import json
from pathlib import Path


def load(path: str, language: str | None = None, limit: int = 0) -> list[dict]:
    base = Path(path).parent
    items = []
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        item = json.loads(line)
        if language and item["language"] != language:
            continue
        item["path"] = str(base / item["audio"])
        items.append(item)
        if limit and len(items) >= limit:
            break
    return items
