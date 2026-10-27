#!/usr/bin/env python3
"""Exports a NeMo CTC model to a fixed-window, full-integer TFLite model.

Steps: NeMo checkpoint -> PyTorch encoder+decoder with a fixed input of
(1, features, WINDOW) -> ONNX -> TensorFlow SavedModel (onnx2tf) -> int8
TFLite with int8 input and output (needed by the Edge TPU) -> metadata JSON
for the host front end and the CTC decoder.

Calibration uses telephone audio (the output of prepare_audio.py), so the
int8 ranges fit the audio that TalkOps sends.

    python3 -I tools/coral/export_nemo.py --candidate de-quartznet-15x5 \\
        --calibration data/tel-de/manifest.jsonl --out models/de
    python3 -I tools/coral/compile_model.py models/de/model.tflite

Run this offline on a workstation (x86-64, no TPU needed). It downloads the
checkpoint once from the source in candidates.json.
"""
import argparse
import hashlib
import json
import random
import subprocess
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import audio, manifest  # noqa: E402
from coraltools.features import Frontend, log_mel  # noqa: E402

HERE = Path(__file__).resolve().parent


def candidate(cid: str) -> dict:
    data = json.loads((HERE / "candidates.json").read_text())
    for c in data["ctc"]:
        if c["id"] == cid:
            return c
    raise SystemExit(f"unknown candidate {cid}")


def load_nemo(source: str):
    import nemo.collections.asr as nemo_asr  # type: ignore

    if source.endswith(".nemo") and Path(source).exists():
        return nemo_asr.models.ASRModel.restore_from(source, map_location="cpu")
    name = source.split(" ")[0].removeprefix("nvidia/")
    return nemo_asr.models.ASRModel.from_pretrained(name, map_location="cpu")


def frontend_of(model) -> Frontend:
    p = model.cfg.preprocessor
    return Frontend(
        sample_rate=int(p.get("sample_rate", 16000)),
        window_size=float(p.get("window_size", 0.02)),
        window_stride=float(p.get("window_stride", 0.01)),
        n_fft=int(p.get("n_fft", 512) or 512),
        features=int(p.get("features", 64)),
        normalize=str(p.get("normalize", "per_feature")),
    )


def export_onnx(model, fe: Frontend, window: int, path: Path) -> int:
    import torch

    class Acoustic(torch.nn.Module):
        def __init__(self, encoder, decoder):
            super().__init__()
            self.encoder, self.decoder = encoder, decoder

        def forward(self, feats):
            length = torch.full((1,), feats.shape[-1], dtype=torch.int64)
            encoded, _ = self.encoder(audio_signal=feats, length=length)
            return self.decoder(encoder_output=encoded)

    net = Acoustic(model.encoder, model.decoder).eval()
    dummy = torch.zeros(1, fe.features, window)
    with torch.no_grad():
        frames_out = net(dummy).shape[1]
    torch.onnx.export(net, (dummy,), str(path), input_names=["features"], output_names=["log_probs"],
                      opset_version=13, dynamic_axes=None, dynamo=False)
    return frames_out


def calibration_windows(items: list, fe: Frontend, window: int, count: int):
    rng = random.Random(0)
    need = (window - 1) * fe.hop
    picked = 0
    while picked < count:
        item = rng.choice(items)
        x, rate = audio.read_wav(item["path"])
        x = audio.resample(x, rate, fe.sample_rate)
        if len(x) > need:
            s = rng.randrange(0, len(x) - need)
            x = x[s:s + need]
        yield log_mel(x, fe)[:, :window]
        picked += 1


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--candidate", required=True, help="id in candidates.json")
    ap.add_argument("--checkpoint", help="local .nemo file instead of the download")
    ap.add_argument("--calibration", required=True, help="manifest from prepare_audio.py")
    ap.add_argument("--window", type=int, default=256, help="input frames (10 ms each)")
    ap.add_argument("--overlap", type=int, default=64, help="overlap between windows in frames")
    ap.add_argument("--calibration-windows", type=int, default=300)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    cand = candidate(args.candidate)
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    model = load_nemo(args.checkpoint or cand["source"])
    model.preprocessor.featurizer.dither = 0.0
    fe = frontend_of(model)
    labels = list(model.decoder.vocabulary)

    onnx_path = out / "model.onnx"
    frames_out = export_onnx(model, fe, args.window, onnx_path)
    subsampling = round(args.window / frames_out)

    saved = out / "saved_model"
    subprocess.run(["onnx2tf", "-i", str(onnx_path), "-o", str(saved), "-osd", "-n"], check=True)

    import tensorflow as tf  # type: ignore

    items = manifest.load(args.calibration, cand["language"])
    if not items:
        raise SystemExit("no calibration audio for the candidate language")
    converter = tf.lite.TFLiteConverter.from_saved_model(str(saved))
    probe = tf.saved_model.load(str(saved)).signatures["serving_default"]
    in_shape = list(next(iter(probe.structured_input_signature[1].values())).shape)
    layout = "nct" if in_shape[1] == fe.features else "ntc"

    def representative():
        for feats in calibration_windows(items, fe, args.window, args.calibration_windows):
            x = feats[None] if layout == "nct" else feats.T[None]
            yield [x.astype(np.float32)]

    converter.optimizations = [tf.lite.Optimize.DEFAULT]
    converter.representative_dataset = representative
    converter.target_spec.supported_ops = [tf.lite.OpsSet.TFLITE_BUILTINS_INT8]
    converter.inference_input_type = tf.int8
    converter.inference_output_type = tf.int8
    tflite = converter.convert()
    (out / "model.tflite").write_bytes(tflite)

    src = Path(args.checkpoint) if args.checkpoint else None
    meta = {
        "candidate": cand["id"],
        "language": cand["language"],
        "source": cand["source"],
        "license": cand["license"],
        "license_verified": cand["license_verified"],
        "checkpoint_sha256": hashlib.sha256(src.read_bytes()).hexdigest() if src else None,
        "labels": labels,
        "log_probs": True,
        "window_frames": args.window,
        "overlap_frames": args.overlap,
        "subsampling": subsampling,
        "input_layout": layout,
        "frontend": fe.__dict__,
        "tflite_bytes": len(tflite),
        "tensorflow": tf.__version__,
    }
    (out / "model.json").write_text(json.dumps(meta, indent=2, ensure_ascii=False))
    print(json.dumps({k: meta[k] for k in ("candidate", "window_frames", "subsampling", "input_layout",
                                            "tflite_bytes")}))


if __name__ == "__main__":
    main()
