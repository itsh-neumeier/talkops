"""Command line options shared by bench_file.py and bench_live.py."""
from __future__ import annotations

import argparse


def add_arguments(ap: argparse.ArgumentParser) -> None:
    ap.add_argument("--engine", choices=["coral", "whisper"], default="coral")
    ap.add_argument("--device", default="usb:0",
                    help="libedgetpu device (usb:0, pci:0) or 'cpu' for the CPU reference")
    ap.add_argument("--model", action="append", default=[], metavar="LANG=PATH",
                    help="compiled CTC model per language, e.g. de=models/de/model_edgetpu.tflite")
    ap.add_argument("--lang-mode", choices=["fixed", "confidence", "classifier"], default="fixed",
                    help="fixed: manifest language (or --preferred); confidence: run all models; "
                         "classifier: --langid model on the first window of each turn")
    ap.add_argument("--langid", help="compiled language ID model")
    ap.add_argument("--preferred", help="tenant language, used first (default: first --model)")
    ap.add_argument("--whisper-cli", default="/opt/whisper/whisper-cli")
    ap.add_argument("--whisper-model", default="/models/ggml-base.bin")
    ap.add_argument("--whisper-threads", type=int, default=2,
                    help="threads per whisper run (the live preview uses at most 2)")


def coral(args):
    from .runtime import CtcModel, LangId, Recognizer, Tpu

    tpu = Tpu(None if args.device == "cpu" else args.device)
    models = {}
    for spec in args.model:
        lang, _, path = spec.partition("=")
        models[lang] = CtcModel(path, tpu)
    if not models:
        raise SystemExit("--model LANG=PATH is required for --engine coral")
    langid = LangId(args.langid, tpu) if args.langid else None
    if args.lang_mode == "classifier" and not langid:
        raise SystemExit("--lang-mode classifier needs --langid")

    def recognizer(language: str | None) -> Recognizer:
        # fixed: the reference language of the utterance (an oracle);
        # otherwise the tenant language comes first.
        first = language if args.lang_mode == "fixed" else (args.preferred or next(iter(models)))
        return Recognizer(models, args.lang_mode, langid, first)

    return tpu, models, langid, recognizer


def whisper(args):
    from .whisper import Whisper

    return Whisper(args.whisper_cli, args.whisper_model, args.whisper_threads)
