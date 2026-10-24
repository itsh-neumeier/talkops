# Coral Edge TPU speech recognition tools (Phase 0)

These offline tools measure if a Google Coral Edge TPU can do German and
English speech recognition for TalkOps. The results go into
[`docs/research/coral-stt-benchmark.md`](../../docs/research/coral-stt-benchmark.md).
TalkOps images do not contain these tools.

## Files

| File | Purpose | Needs |
| --- | --- | --- |
| `candidates.json` | Candidate models, sources and licenses | - |
| `hostinfo.py` | Host, driver, library and Edge TPU device facts | Python only |
| `prepare_audio.py` | Telephone channel simulation (G.711) and manifest | numpy |
| `export_nemo.py` | NeMo CTC checkpoint to fixed-window int8 TFLite | `requirements-convert.txt` |
| `train_langid.py` | German/English language ID model, int8 TFLite | TensorFlow |
| `compile_model.py` | `edgetpu_compiler` run and op mapping report | Edge TPU compiler |
| `bench_file.py` | WER, CER, real-time factor, CPU, language ID accuracy | `requirements-bench.txt` |
| `bench_live.py` | Live preview: N calls, segment latency, backlog, CPU | `requirements-bench.txt` |
| `gate_report.py` | Gate evaluation and result tables in the report | Python only |
| `probe/` | Rust check of the safe `edgefirst-tflite` delegate path | Rust, TFLite C library |

The service `coral-stt` (`crates/talkops-coral-stt`) uses the same front
end, windows and CTC decoding as these tools. Its test
`cargo test -p talkops-coral-stt --test tflite -- --ignored` runs real
models on the CPU or the TPU (see the test header).
| `coraltools/` | Shared modules | see the module comments |
| `tests/` | Unit tests | Python (numpy for some tests) |

## Procedure

Use separate virtual environments for conversion and benchmark. Keep data,
models and results outside the repository, for example in `~/coral-work`.
Run Python with `-I`.

### 1. Prepare the hosts

1. Benchmark host with a USB Accelerator: install `libedgetpu1-std` and
   its udev rule. Benchmark host with an M.2 or Mini PCIe card: install the
   gasket/apex driver; the device node is `/dev/apex_0`.
2. Install the Edge TPU compiler on an x86-64 workstation.
3. Record the host facts:

   ```sh
   python3 -I tools/coral/hostinfo.py > ~/coral-work/results/host-usb.json
   ```

   A USB device in state `firmware-not-loaded` (ID `1a6e:089a`) is normal
   before the first inference. After the first inference it shows as
   `18d1:9302` with a new device number.

### 2. Prepare the test audio

Write a manifest with one JSON object per line (`audio`, `text`,
`language`, optional `channel`), then:

```sh
python3 -I tools/coral/prepare_audio.py cv-test.jsonl ~/coral-work/data/test --codec pcma
python3 -I tools/coral/prepare_audio.py cv-train.jsonl ~/coral-work/data/train --codec pcma
```

### 3. Export and compile the models

```sh
python3 -m venv ~/coral-work/venv-convert
~/coral-work/venv-convert/bin/pip install -r tools/coral/requirements-convert.txt
~/coral-work/venv-convert/bin/python -I tools/coral/export_nemo.py \
    --candidate de-quartznet-15x5 --calibration ~/coral-work/data/train/manifest.jsonl \
    --out ~/coral-work/models/de
~/coral-work/venv-convert/bin/python -I tools/coral/export_nemo.py \
    --candidate en-citrinet-256 --calibration ~/coral-work/data/train/manifest.jsonl \
    --out ~/coral-work/models/en
~/coral-work/venv-convert/bin/python -I tools/coral/train_langid.py \
    ~/coral-work/data/train/manifest.jsonl --validation ~/coral-work/data/test/manifest.jsonl \
    --out ~/coral-work/models/langid
for m in de en langid; do
  python3 -I tools/coral/compile_model.py ~/coral-work/models/$m/model.tflite
  cp ~/coral-work/models/$m/model_edgetpu.compile.json ~/coral-work/results/compile-$m.json
done
```

`onnx2tf` can need extra packages (`onnx_graphsurgeon`, `sng4onnx`,
`onnxsim`); its error message names them. Record `pip freeze` with the models.

For production, also compile the three models in one compiler call
(co-compilation), so one TPU caches the parameters of all of them. Then
copy each `*_edgetpu.tflite` with its `model.json` into the models volume
of `coral-stt` (`models/coral/{de,en,langid}/model_edgetpu.tflite`):

```sh
mkdir -p ~/coral-work/cocompiled && cd ~/coral-work/cocompiled
for m in de en langid; do cp ../models/$m/model.tflite $m.tflite; done
edgetpu_compiler -s de.tflite en.tflite langid.tflite
for m in de en langid; do cp ${m}_edgetpu.tflite ../models/$m/model_edgetpu.tflite; done
```

The compiler names each output after its input (`de_edgetpu.tflite`); the
inputs need different names.

### 4. Measure

On each host (USB and M.2/PCIe), with `D=usb:0` or `D=pci:0`:

```sh
M="--model de=$HOME/coral-work/models/de/model_edgetpu.tflite --model en=$HOME/coral-work/models/en/model_edgetpu.tflite"
L="--lang-mode classifier --langid $HOME/coral-work/models/langid/model_edgetpu.tflite --preferred de"
T=~/coral-work/data/test/manifest.jsonl
R=~/coral-work/results

python3 -I tools/coral/bench_file.py $T $M $L --device $D --out $R/file-$D.json
for n in 1 2 3; do
  python3 -I tools/coral/bench_live.py $T $M $L --device $D --calls $n --minutes 5 --out $R/live-$D-$n.json
done
# Baseline: whisper.cpp base (same host)
python3 -I tools/coral/bench_file.py $T --engine whisper --lang-mode confidence --whisper-threads 4 \
    --whisper-model /path/to/ggml-base.bin --out $R/file-whisper-base.json
python3 -I tools/coral/bench_live.py $T --engine whisper --lang-mode confidence --calls 1 --minutes 5 \
    --whisper-model /path/to/ggml-base.bin --out $R/live-whisper-base.json
```

`--lang-mode confidence` for whisper.cpp means automatic language detection.
Use `--device cpu` for a CPU reference of the same TFLite models.

Rust probe (needs `libtensorflowlite_c.so` of the TFLite version that
`libedgetpu` was built for):

```sh
cargo build --release --manifest-path tools/coral/probe/Cargo.toml
tools/coral/probe/target/release/coral-probe --tflite /path/to/libtensorflowlite_c.so \
    --device $D --model ~/coral-work/models/de/model_edgetpu.tflite --runs 200
```

### 5. Evaluate the gate

Read the `examples` in the live results. Then:

```sh
python3 -I tools/coral/gate_report.py ~/coral-work/results \
    --coral-live ~/coral-work/results/live-usb:0-1.json \
    --whisper-live ~/coral-work/results/live-whisper-base.json --readable yes
```

## Tests

```sh
python3 -m unittest discover -s tools/coral/tests -v
```

CI runs the tests that need no numpy. The numpy tests run when numpy is
installed.
