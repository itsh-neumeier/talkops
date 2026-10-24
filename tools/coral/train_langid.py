#!/usr/bin/env python3
"""Trains a small German/English language ID model and exports it as int8 TFLite.

Input: manifests from prepare_audio.py (telephone audio) with German and
English lines, for example from Mozilla Common Voice (CC0). Use different
speakers for training and for the benchmark.

The model sees one 2.56 s window of log-mel features (the same front end
as the CTC models) and returns two class probabilities. The coral-stt
service runs it on the first window of a turn and then uses only the CTC
model of that language.

    python3 -I tools/coral/train_langid.py data/train/manifest.jsonl \\
        --validation data/dev/manifest.jsonl --out models/langid
    python3 -I tools/coral/compile_model.py models/langid/model.tflite
"""
import argparse
import json
import random
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from coraltools import audio, manifest  # noqa: E402
from coraltools.features import Frontend, log_mel  # noqa: E402

CLASSES = ["de", "en"]


def windows(items: list, fe: Frontend, window: int, per_item: int, seed: int):
    rng = random.Random(seed)
    need = (window - 1) * fe.hop
    xs, ys = [], []
    for item in items:
        if item["language"] not in CLASSES:
            continue
        x, rate = audio.read_wav(item["path"])
        x = audio.resample(x, rate, fe.sample_rate)
        for _ in range(per_item):
            if len(x) > need:
                s = rng.randrange(0, len(x) - need)
                part = x[s:s + need]
            else:
                part = np.pad(x, (0, need - len(x)))
            xs.append(log_mel(part, fe)[:, :window])
            ys.append(CLASSES.index(item["language"]))
    return np.stack(xs)[..., None].astype(np.float32), np.array(ys)


def build(tf, features: int, window: int):
    layers = tf.keras.layers
    inp = layers.Input((features, window, 1))
    x = inp
    for filters in (16, 32, 64, 64):
        x = layers.Conv2D(filters, 3, padding="same", use_bias=False)(x)
        x = layers.BatchNormalization()(x)
        x = layers.ReLU()(x)
        x = layers.MaxPool2D(2)(x)
    x = layers.GlobalAveragePooling2D()(x)
    x = layers.Dropout(0.3)(x)
    out = layers.Dense(len(CLASSES), activation="softmax")(x)
    return tf.keras.Model(inp, out)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("train")
    ap.add_argument("--validation", required=True)
    ap.add_argument("--window", type=int, default=256)
    ap.add_argument("--per-item", type=int, default=2)
    ap.add_argument("--epochs", type=int, default=15)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    import tensorflow as tf  # type: ignore

    fe = Frontend()
    xt, yt = windows(manifest.load(args.train), fe, args.window, args.per_item, 1)
    xv, yv = windows(manifest.load(args.validation), fe, args.window, 1, 2)
    model = build(tf, fe.features, args.window)
    model.compile(optimizer="adam", loss="sparse_categorical_crossentropy", metrics=["accuracy"])
    model.fit(xt, yt, validation_data=(xv, yv), epochs=args.epochs, batch_size=64)
    accuracy = float(model.evaluate(xv, yv, verbose=0)[1])

    converter = tf.lite.TFLiteConverter.from_keras_model(model)
    converter.optimizations = [tf.lite.Optimize.DEFAULT]
    converter.representative_dataset = lambda: ([xt[i:i + 1]] for i in range(0, len(xt), max(1, len(xt) // 300)))
    converter.target_spec.supported_ops = [tf.lite.OpsSet.TFLITE_BUILTINS_INT8]
    converter.inference_input_type = tf.int8
    converter.inference_output_type = tf.int8
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    tflite = converter.convert()
    (out / "model.tflite").write_bytes(tflite)
    meta = {
        "candidate": "langid-de-en-cnn",
        "classes": CLASSES,
        "window_frames": args.window,
        "input_layout": "nhwc",
        "frontend": fe.__dict__,
        "validation_accuracy_float": round(accuracy, 4),
        "license": "own model, trained on CC0 data",
        "license_verified": True,
        "tflite_bytes": len(tflite),
        "tensorflow": tf.__version__,
    }
    (out / "model.json").write_text(json.dumps(meta, indent=2))
    print(json.dumps(meta))


if __name__ == "__main__":
    main()
