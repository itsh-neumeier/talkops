#!/usr/bin/env python3
"""Builds a telephone-quality test set and its manifest.

Input: a JSON Lines manifest with one utterance per line:

    {"audio": "clip.wav", "text": "reference", "language": "de"}
    {"audio": "call.wav", "channel": 1, "text": "...", "language": "en"}

`audio` is a PCM16 WAV path relative to the manifest. Convert MP3 corpora
(Common Voice) to WAV before, for example with ffmpeg. Stereo TalkOps call
recordings need `channel` (0 = caller, 1 = called party); they are already
telephone audio, so use --codec none for them.

Output: one 16 kHz mono WAV per line in OUT_DIR and OUT_DIR/manifest.jsonl.

    python3 -I tools/coral/prepare_audio.py cv-de.jsonl out/de --codec pcma
"""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import audio  # noqa: E402


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("manifest")
    ap.add_argument("out_dir")
    ap.add_argument("--codec", choices=["pcmu", "pcma", "none"], default="pcma",
                    help="G.711 law for the telephone channel (pcma is usual in Europe)")
    ap.add_argument("--limit", type=int, default=0, help="use only the first N lines")
    args = ap.parse_args()

    src = Path(args.manifest)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    lines = [json.loads(line) for line in src.read_text(encoding="utf-8").splitlines() if line.strip()]
    if args.limit:
        lines = lines[: args.limit]
    with (out / "manifest.jsonl").open("w", encoding="utf-8") as m:
        for i, item in enumerate(lines):
            x, rate = audio.read_wav(str(src.parent / item["audio"]), item.get("channel"))
            if args.codec == "none":
                y = audio.resample(x, rate, 16000)
            else:
                y = audio.telephone(x, rate, args.codec, 16000)
            name = f"{i:05d}.wav"
            audio.write_wav(str(out / name), y, 16000)
            m.write(json.dumps({"audio": name, "text": item["text"], "language": item["language"],
                                "secs": round(len(y) / 16000, 3), "codec": args.codec},
                               ensure_ascii=False) + "\n")
    print(f"{len(lines)} utterances -> {out / 'manifest.jsonl'}")


if __name__ == "__main__":
    main()
