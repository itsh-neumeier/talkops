# Coral Edge TPU speech recognition: Phase 0 benchmark

- Status: **Tools ready. Measurements pending. Gate not evaluated.** On
  2026-10-24 the user decided to implement Phases 2 to 7 before the
  measurements and to correct the result afterwards if the gate fails
  (see section 8).
- Date: 2026-10-24
- Update 2026-10-27: the user will run the actual Rust-service trial on the
  production host. The earlier [Python USB comparison](coral-results/20261026-preview-usb/README.md)
  does not establish the Rust-service gate. Use the
  [trial and diagnostic guide](coral-production-trial.md) for that handoff.
- Scope: Phase 0 (feasibility gate) of the task "Coral Edge TPU speech
  recognition for German and English". Phases 2 to 7 (service
  `coral-stt`, integration, fallback, UI, deployment, documentation) are
  implemented ([ADR 0046](../adr/0046-coral-edge-tpu-speech-recognition.md)).
- Tools: [`tools/coral/`](../../tools/coral/README.md)
- Related: [ADR 0012](../adr/0012-aufzeichnung-und-transkription.md),
  [ADR 0019](../adr/0019-transkription-in-zwei-durchgaengen-und-ki-api.md),
  [ADR 0022](../adr/0022-active-call-identities-and-live-previews.md),
  [ADR 0024](../adr/0024-transcription-engines-per-purpose.md)

## 1. Purpose

TalkOps can transcribe the live preview, recordings and voicemails with
whisper.cpp on the CPU or with an OpenAI-compatible connection. This study
finds out if a Google Coral Edge TPU in the TalkOps server can do the first
pass of German and English speech recognition. The intended operation is a
hybrid (ADR 0019): the Coral gives the quick live preview and the first
pass; local Whisper replaces the text in the second pass.

Whisper cannot run on the Edge TPU. The Edge TPU runs only full-integer
(int8) TensorFlow Lite models with fixed tensor shapes. It has about 8 MB of
on-chip memory for model parameters; larger models stream their parameters
from host memory for each inference. Thus the candidates are small CTC
models with fixed windows, plus a German/English language identification
(language ID) classifier.

## 2. Gate

The gate is a hard condition. If it fails, the work stops after this report
and the report proposes alternatives (section 9).

| No. | Criterion | Measurement | Limit |
| --- | --- | --- | --- |
| G1 | One call (2 channels) in real time on one TPU | `bench_live.py --calls 1` | backlog at the end <= 1 s |
| G2 | Live latency per segment | p95 from the end of a turn to the final text | < 1.5 s |
| G3 | Host CPU load | CPU cores per channel of the Coral run / CPU cores per channel of whisper.cpp `base` | <= 0.5 |
| G4 | Text readable as a live preview | manual rating of the example transcripts; WER as a guide | rating "yes"; WER guide <= 35 % |

`tools/coral/gate_report.py` calculates G1 to G3 from the result files and
writes the tables in section 7. G4 needs a person: read the example
transcripts in the live results and give `--readable yes` or `--readable no`.

Extra conditions that the gate also needs:

- The models used for the gate have a license that permits redistribution
  of the compiled model with TalkOps (AGPL-3.0). See section 5.
- The Rust service can run the model without `unsafe` in TalkOps code.
  See section 6.

## 3. Method

### 3.1 Test audio

1. German and English test utterances with references. Recommended:
   Mozilla Common Voice test sets (CC0), at least 200 utterances for each
   language, from speakers not used for language ID training.
2. `prepare_audio.py` sends them through a simulated telephone channel:
   300-3400 Hz band, 8 kHz, G.711 A-law (`--codec pcma`, usual in Europe)
   or mu-law, then back to 16 kHz.
3. Recommended in addition: at least 10 minutes of real TalkOps call
   recordings (stereo, 8 kHz) with manual references, prepared with
   `--codec none` and a `channel` for each line. Do not commit this audio
   or its text to Git.

### 3.2 Models

1. `export_nemo.py` exports a NeMo CTC checkpoint with a fixed window
   (default 256 frames = 2.56 s, 64 frames overlap) to an int8 TFLite model.
   The calibration uses telephone audio.
2. `compile_model.py` runs `edgetpu_compiler` and records the share of
   operations on the TPU and the on-chip and off-chip parameter memory.
3. `train_langid.py` trains a small German/English classifier on one
   window. The alternative without a classifier is `--lang-mode confidence`:
   both CTC models run, and the text with the higher CTC confidence wins.
   This doubles the TPU load.

### 3.3 Measurements

| Measurement | Tool | Devices |
| --- | --- | --- |
| Device and runtime facts | `hostinfo.py` | each host |
| Rust runtime path, invoke latency | `probe/` (`coral-probe`) | USB, M.2/PCIe |
| WER, CER, real-time factor (RTF), CPU per audio second, language ID accuracy | `bench_file.py` | USB, M.2/PCIe, CPU reference; whisper.cpp `base` |
| Segment latency, backlog, CPU cores, TPU busy share, WER | `bench_live.py` with 1, 2 and 3 calls | USB, M.2/PCIe; whisper.cpp `base` with 2 threads |

`bench_live.py` plays each channel at real-time speed and uses the turn
detector of the TalkOps realtime client (`coraltools/turns.py`, a port of
`crates/talkops-media-worker/src/realtime.rs` with the same unit test
scenarios). The Coral run processes each window as soon as its audio is
complete; at the end of a turn only the rest remains. The whisper.cpp run
transcribes each turn after the end of the turn, with at most 2 threads,
as the worker does for the live preview.

### 3.4 Known limits of the method

- The benchmark harness is Python. Its overhead adds to the measured
  Coral CPU load. The Rust service will use less CPU, so G3 is
  conservative.
- The WER normalisation does not expand numbers. The CTC models write
  numbers as words, Whisper often writes digits. This makes the Whisper
  WER look worse. Use references that write numbers as words.
- The live preview of TalkOps with local Whisper uses 8-second parts
  (ADR 0022), not turns. The turn-based Whisper run is a fair CPU
  comparison per audio second, but not the same latency as today.
- Feature normalisation is done for each window, not for the full turn.
  This can increase the WER a little compared with the NeMo reference.

## 4. Candidate models

The full list with sources is in
[`tools/coral/candidates.json`](../../tools/coral/candidates.json).

| ID | Language | Architecture | Parameters | int8 size compared with 8 MB cache | License |
| --- | --- | --- | --- | --- | --- |
| `en-citrinet-256` | en | Citrinet, CTC, sentencepiece | about 10 M | slightly larger | CC-BY-4.0 (model card, checked 2026-10-24) |
| `en-quartznet-15x5` | en | QuartzNet 15x5, CTC, characters | 18.9 M | about 2.4 times larger | to verify |
| `de-quartznet-15x5` | de | QuartzNet 15x5, CTC, characters | 18.9 M | about 2.4 times larger | to verify |
| `de-citrinet-256-ft` | de | own fine-tune of `en-citrinet-256` on Common Voice German | about 10 M | slightly larger | CC-BY-4.0 base + CC0 data |
| `langid-de-en-cnn` | de/en | own small CNN | about 0.2 M | fits | own model, CC0 data |

Not evaluated, with reasons:

- Whisper (all sizes): transformer attention, dynamic shapes and size.
- wav2vec2, HuBERT, Conformer CTC: attention, LayerNorm and GELU map
  badly, and the models are much larger than the parameter cache.
- Coqui STT / DeepSpeech: LSTM layers stay mostly on the CPU; the project
  is archived.
- Vosk / Kaldi: not TensorFlow Lite models.
- Silero STT: the license of the STT models is not clearly compatible with
  redistribution under AGPL-3.0.

Expected risks before the measurement:

- German has no known small conv-only CTC model below the parameter cache
  size. `de-quartznet-15x5` streams about 11 MB of parameters for each
  window. On USB this can increase the latency per window. Compile with
  `--segments 2` to measure a split over two TPUs. If German fails, the
  fallback is the fine-tune `de-citrinet-256-ft` (needs a GPU for
  training; not part of this phase).
- `de-quartznet-15x5` has only read speech (Common Voice) in its training
  data. Expect a high WER on telephone calls.

## 5. Licenses

- TalkOps is AGPL-3.0-only. A compiled model is a data file that the
  `coral-stt` image distributes. CC-BY-4.0 permits redistribution and
  modification with attribution. The image must then contain the
  attribution and the license text in `THIRD_PARTY_NOTICES.md`.
- `license_verified` in `candidates.json` must be `true` before a model is
  put in an image. `export_nemo.py` copies the value to the model metadata.
- The language ID model is trained by TalkOps on CC0 data.
- Runtime libraries: `libedgetpu` (Apache-2.0), TensorFlow Lite / LiteRT
  (Apache-2.0), `edgefirst-tflite` (Apache-2.0).
- Open: confirm the license of the NGC QuartzNet checkpoints
  (`en-quartznet-15x5`, `de-quartznet-15x5`) on the NGC pages and record
  the date in `candidates.json`.

## 6. Runtime binding without `unsafe`

The workspace forbids `unsafe` code. Result of the search on 2026-10-24:

| Crate | Version | License | External delegate (libedgetpu) | Assessment |
| --- | --- | --- | --- | --- |
| `edgefirst-tflite` | 0.10.3 (2026-10-01) | Apache-2.0 | yes: `Delegate::load_with_options(path, options)` uses the `tflite_plugin_create_delegate` entry points that `libedgetpu` exports | maintained; selected for the probe |
| `tflite-c-rs` | 0.0.1 | MIT or Apache-2.0 | yes (`ExternalDelegate`) | too early |
| `tflitec` | 0.5.x | MIT | no external delegate in the documented API | not suitable |

`edgefirst-tflite` loads `libtensorflowlite_c.so` at run time
(`libloading`). It needs no build script and no TensorFlow source. The
probe in `tools/coral/probe/` uses it with `unsafe_code = "forbid"`. It
builds and passes `cargo clippy -- -D warnings`. Without a TFLite library
it stops with a clear load error. A run on Edge TPU hardware is pending.

Thus no FFI exception ADR is necessary if the probe works on the hardware.
Risk: `libedgetpu` must be built for the same TensorFlow Lite version as
`libtensorflowlite_c.so`. The official `libedgetpu` packages are old;
community builds exist for newer TensorFlow versions. The probe run on
the hardware must record both versions (`hostinfo.py`).

## 7. Results

Replace the tables with `tools/coral/gate_report.py`. Do not edit the
generated parts by hand.

### 7.1 Hosts and devices

Paste the essential `hostinfo.py` facts here (CPU, kernel, USB speed,
driver, libedgetpu and TFLite versions, virtualisation).

| Host | CPU | Device | Connection | libedgetpu | TFLite runtime |
| --- | --- | --- | --- | --- | --- |
| pending | | | | | |

### 7.2 Compilation

<!-- results:compile:begin -->
Pending.
<!-- results:compile:end -->

### 7.3 Rust probe

| Model | Device | Invoke p50 ms | Invoke p95 ms | CPU cores during invoke |
| --- | --- | --- | --- | --- |
| pending | | | | |

### 7.4 File transcription

<!-- results:file:begin -->
Pending.
<!-- results:file:end -->

### 7.5 Live preview

<!-- results:live:begin -->
Pending.
<!-- results:live:end -->

### 7.6 Gate

<!-- results:gate:begin -->
Pending.
<!-- results:gate:end -->

## 8. Decision

Gate result: pending. The decision needs the measured values from section 7.

The user decided to implement the service and the integration before the
measurements (2026-10-24). Thus these parts exist now and need only the
hardware values to confirm or tune:

- service `coral-stt` (`crates/talkops-coral-stt`, image
  `ghcr.io/itsh-neumeier/talkops-coral-stt`), ADR 0046;
- the connection kind `coral`, the local fallback and the UI.

If the gate fails, use the alternatives in section 9. The local fallback
keeps TalkOps working in all cases.

The service uses the same front end, window and CTC rules as the tools
(`features.rs`, `ctc.rs`, `turns.rs` are ports of the Python modules).
Compile the German, English and language ID models in one
`edgetpu_compiler` call (co-compilation), so the TPU can cache the
parameters of all three models together.

Runtime libraries of the image: TensorFlow Lite C v2.19.1 (built from the
pinned commit `f4247ebb6f9e7421f38c3f01a9a5d5cd54bd24fd`, XNNPACK off) and
libedgetpu 16.0 for TF 2.19.1 (feranick community build for trixie,
SHA-256 pinned). A CPU test with this TensorFlow Lite library and small
int8 test models passed through `edgefirst-tflite` (2026-10-24).

## 9. Alternatives if the gate fails

1. **Coral for voice activity detection (VAD) and language ID only.** The
   TPU finds turns and the language; whisper.cpp transcribes only speech
   in the correct language. This decreases the CPU load but does not give
   faster text.
2. **Coral for the live preview only in English**, if only German fails.
3. **No Coral.** Use the existing OpenAI-compatible connection to a GPU
   server in the LAN (ADR 0024). Audio stays in the LAN.
4. **Another accelerator** with transformer support (for example a GPU or
   an NPU that runs Whisper). This is a separate task.

## 10. Open work

- Measure on USB and M.2/PCIe hardware (section 3). The user supplies the
  values or the result files.
- Verify the NGC licenses (section 5).
- Run the Rust probe on the hardware (section 6).
- Full ASD-STE100 language review: the standard was not available. The
  text follows `docs/documentation-style.md`.
