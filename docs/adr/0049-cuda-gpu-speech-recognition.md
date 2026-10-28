# 0049 - Speech recognition on NVIDIA GPUs (`cuda-stt`)

- Status: Proposed on 2026-10-28. Implementation waits for the GPU and
  driver version of the production host (see "Open questions").
- Supplements: ADR 0024 (transcription engines per purpose), ADR 0046
  (Coral Edge TPU speech recognition), ADR 0048 (Coral model library).
- Related: ADR 0009 (Debian 13 base images).

## Context

`coral-stt` (ADR 0046) runs small CTC models with fixed windows on an Edge
TPU. The TPU has about 8 MB of parameter cache. Larger models with a
better German word error rate cannot run on it. Whisper cannot run on it.

The operator wants a service with the same functions on an NVIDIA GPU:

- German and English telephone speech, file transcription and live preview.
- Live preview latency of 3 to 5 seconds.
- The same OpenAI-compatible interface, so the media worker, the fallback
  and the UI work as with Coral.
- The same administrator model library with license and source statements
  (ADR 0048). TalkOps must not ship models without a redistribution grant
  (for example NVIDIA NGC `stt_de_quartznet15x5`).

Known hardware:

- Test desktop: Windows, Docker Desktop with WSL2, two NVIDIA T600 with
  4 GB each (Turing, compute capability 7.5).
- Production host: unknown.

Constraints from the project:

- Robustness and maintainability come before own implementations. Use
  proven components.
- `unsafe` is forbidden in the workspace. FFI must stay in a maintained
  dependency (as `edgefirst-tflite` for Coral).
- Images are based on Debian 13 "trixie" (ADR 0009) and run as UID 10001.
- `docker-compose.yml` must not contain `build:` (Portainer).

Candidate models. The license was checked on the model card. Check the
license again for each checkpoint before it goes into a preset:

| Model | Parameters | Languages | License |
| --- | --- | --- | --- |
| `nvidia/stt_de_fastconformer_hybrid_large_pc` | 115M | de | CC-BY-4.0 |
| `nvidia/parakeet-tdt-0.6b-v3` | 600M | 25 European, incl. de, en | CC-BY-4.0 |
| `nvidia/canary-180m-flash` | 182M | de, en, es, fr | CC-BY-4.0 |
| OpenAI Whisper (all sizes) | 39M to 1.55B | multilingual | MIT |

This ADR makes no word error rate or latency claim for these models.
The numbers come from measurements on the target hardware (see
"Acceptance measurements").

## Options

### Inference runtime

1. **sherpa-onnx** (k2-fsa, Apache-2.0) with the ONNX Runtime CUDA
   execution provider, through the official Rust crate `sherpa-onnx`
   (safe wrapper over the C API, FFI in `sherpa-onnx-sys`).
   - Supports NeMo CTC, NeMo transducer and TDT (Parakeet), Canary,
     Whisper, Silero VAD and Whisper language identification in one
     library and one model format (ONNX plus `tokens.txt`).
   - Publishes ready ONNX exports of Parakeet TDT 0.6B v3 and
     Canary 180M Flash, and export scripts for NeMo models.
   - Publishes prebuilt GPU libraries for each release, for example
     `sherpa-onnx-v2.0.1-cuda-13.x-cudnn-9.x-onnxruntime1.28.2-linux-x64-gpu.tar.bz2`
     and a `cuda-12.x` variant.
   - The C API selects the execution provider by name (`"cuda"`). It has no
     GPU index. One process uses one GPU (`CUDA_VISIBLE_DEVICES`).
2. **ONNX Runtime from Rust (`ort`)** with own pre- and post-processing.
   - Proven runtime, GPU index available.
   - TalkOps must implement the log-mel front end, the transducer/TDT
     search, the Canary and Whisper decoders and the tokenizers for each
     model family. This is a large amount of own code that sherpa-onnx
     already maintains.
3. **faster-whisper / CTranslate2** (MIT), for example as the server
   `speaches`.
   - Proven and fast for Whisper. Python service, Whisper only. No NeMo
     models, so no Parakeet, Canary or FastConformer.
   - The admin library, diagnostics and the TalkOps realtime events must be
     built again in Python.
4. **NVIDIA NeMo** (PyTorch) or **NVIDIA Riva / NIM**.
   - NeMo: reference implementation, but a Python/PyTorch image of many
     gigabytes and a high VRAM use. Not suitable for a 4 GB card with
     several models.
   - Riva/NIM: proprietary license and NGC terms. Not redistributable.

### Image base

1. `nvidia/cuda:*-runtime-ubuntu24.04` as the base image.
   - Contradicts ADR 0009. A second base distribution must be patched
     and scanned.
2. **`debian:trixie-slim` plus the NVIDIA CUDA apt repository for
   Debian 13** (`developer.download.nvidia.com/compute/cuda/repos/debian13`).
   - The repository has CUDA 13 runtime packages and cuDNN 9
     (`libcudnn9-cuda-13`). It has no CUDA 12 runtime (`cuda-cudart-12-*`)
     for Debian 13 (checked on 2026-10-28).
3. `debian:trixie-slim` plus the CUDA 12 libraries from the NVIDIA Debian 12
   repository or from the NVIDIA Python wheels.
   - Needed only if the production host driver cannot run CUDA 13.

### GPU access in Compose

1. `runtime: nvidia` (old nvidia-docker2 style).
2. **`deploy.resources.reservations.devices` with `driver: nvidia`** (Compose
   specification; needs the NVIDIA Container Toolkit on the host).
3. Bind device nodes (`/dev/nvidia*`) and driver libraries by hand.

## Decision

### Runtime and service

1. A new crate `crates/talkops-cuda-stt` and the image
   `ghcr.io/itsh-neumeier/talkops-cuda-stt`. The media worker image does
   not change.
2. The service uses **sherpa-onnx** through the official `sherpa-onnx` crate
   with the `shared` feature. The image build sets `SHERPA_ONNX_LIB_DIR` to
   the `lib` directory of the pinned GPU release archive (checked by
   SHA-256). The crate version and the archive version must be the same.
   Use a release that is at least two weeks old.
3. Backends, selected by `TALKOPS_CUDA_BACKEND`:
   - `cuda`: execution provider `cuda` (production).
   - `cpu`: execution provider `cpu` (reference, and model checks without a
     GPU; not for production).
   - `mock`: deterministic fake recognizer without libraries or models
     (CI and UI tests).
   CI builds and tests with the CPU shared library that `sherpa-onnx-sys`
   downloads. CI has no GPU.
4. Model families in the first version:
   - `nemo_ctc` (for example the CTC head of
     `stt_de_fastconformer_hybrid_large_pc`),
   - `nemo_transducer` (FastConformer hybrid transducer head, Parakeet TDT),
   - `canary`,
   - `whisper`.
   Silero VAD (MIT) and Whisper language identification are support models.
5. One process uses one GPU. Compose selects the GPU with
   `TALKOPS_CUDA_DEVICE` (default `0`). A host with two GPUs can run a
   second instance on a second port and add a second connection, for
   example one GPU for the live profile and one GPU for recordings.
6. All models of the active selection are loaded at startup and stay in
   VRAM. A device thread owns the recognizers. The queue, the priority of
   live jobs, the fixed capacity, HTTP 503 with `Retry-After` and the
   session limit work as in ADR 0046. Several waiting jobs of the same
   model go to the GPU in one batch (`decode_streams`).

### Interface

7. The interface is the same as `coral-stt` (ADR 0046, 0048). Only the
   names differ:
   - Listen address `127.0.0.1:8096` by default (loopback; a warning is
     logged for other addresses).
   - `GET /health`: status, GPU (name, VRAM total and used, use, temperature
     through NVML), loaded models with measured VRAM, queue.
   - `GET /v1/models`: `cuda-de`, `cuda-en`, `cuda-auto`.
   - `POST /v1/audio/transcriptions`, `GET /v1/realtime?intent=transcription`,
     `/v1/diagnostics/*`: the same requests, events and fields as Coral,
     including `language` per turn and segment and `stable_preview`.
   - `/v1/admin/models`, `/v1/admin/library[/{id}]`,
     `/v1/admin/slots/{slot}`, `/v1/admin/restart`: the model library.
   - NVML is read through the safe crate `nvml-wrapper`. Without NVML,
     `/health` shows no GPU values but the service works.
8. Live preview: the selected models are not streaming models. The service
   uses the turns that the client commits. While a turn grows, the service
   decodes the audio of the turn again every
   `TALKOPS_CUDA_PARTIAL_INTERVAL_MS` (default 1000 ms) and sends the new
   text as `delta`. A turn longer than 15 seconds is split at a pause found
   by Silero VAD, or else at 15 seconds. The expected latency is the interval
   plus the decode time of the turn. The acceptance measurement must show
   if this meets 3 to 5 seconds. Streaming models can be added later as a
   new family with no interface change.
9. Language: `cuda-de` and `cuda-en` decode in the given language (Canary
   and Whisper get the language as a parameter). `cuda-auto` uses the
   language ID model on the first seconds of a turn, or the language that a
   multilingual model reports, or else the first preferred language.

### Shared code with `coral-stt`

10. Code that does not depend on the accelerator moves into a new library
    crate `crates/talkops-stt-service`: audio decoding and resampling, turn
    split, diagnostics capture, API key check, error responses, the
    realtime event protocol, and the library store of ADR 0048 (ids,
    `install.json`, statements, usage, limits). `coral-stt` and `cuda-stt`
    implement a recognizer trait and a model check. The move does not
    change the Coral interface. The Coral tests stay and must pass.

### Model library

11. A library model is a directory `models/cuda/library/<id>/` with ONNX
    files, `tokens.txt` and a `model.json` manifest:
    `family`, `languages`, file names, `sample_rate`, and the statements
    `name`, `license`, `source`. The id is the first 12 hex digits of the
    SHA-256 of the sorted file digests.
12. Slots: `de`, `en`, `langid`, `vad`. One multilingual model can fill
    `de` and `en`. It is stored once and loaded once. The selection is in
    `models/cuda/selection.json`; the service does not copy model files
    into slot directories (the files can have several gigabytes).
13. Install methods:
    - **Download by URL**: the administrator gives an HTTPS URL, the
      expected SHA-256, the family, the languages, the license and the
      source. The service downloads the archive into a staging directory,
      checks the digest, unpacks it and checks it. This is the main method
      for large models.
    - **Upload**: an archive (`.tar`, `.tar.gz`, `.tar.bz2`) through the
      TalkOps UI. The TalkOps API and the service stream the body to a file.
      They do not keep it in memory.
    - Unpacking rejects absolute paths, `..`, links and device files.
14. The check before a model goes into the library:
    - Required files of the family exist; each `.onnx` file has a valid
      ONNX protobuf start; `tokens.txt` is text.
    - The service loads the model with the `cpu` provider in a child
      process with a time limit. A model that fails is rejected. The child
      process protects the service from a crash in native code.
15. A data file `presets/stt-models/cuda.yaml` lists suggested models with
    URL, SHA-256, family, languages, license and source (as the trunk
    presets). Only models with a license that permits redistribution and
    use go into this file. TalkOps does not host the files. The UI shows the
    presets as "Install"; the administrator still confirms the license.
16. Limits (environment, with defaults): one model at most 4 GiB unpacked,
    the library at most 20 GiB and 10 models. Selected models cannot be
    deleted. A model change needs a restart (ADR 0048).

### VRAM

17. The service does not estimate VRAM from model size. It measures the
    VRAM in use (NVML) before and after it loads each model at startup and
    shows the value per model in `/health` and in the UI.
18. If a model does not load (for example out of memory), the slot shows the
    state `error` with the message. The other models stay loaded and the
    service starts. The administrator selects a smaller model (for example
    an int8 export) in the UI.
19. `TALKOPS_CUDA_VRAM_RESERVE_MB` (default 512) is VRAM that must stay free
    after startup. If less is free, `/health` shows a warning, so batches and
    long turns do not fail at run time.

### Image and Compose

20. Base image `debian:trixie-slim` (ADR 0009 stays valid). The image adds
    the NVIDIA CUDA apt repository for Debian 13, with the signing key
    pinned by fingerprint, and installs only the runtime libraries that the
    ONNX Runtime CUDA provider needs (cudart, cuBLAS, cuFFT, cuDNN 9 for
    CUDA 13). The build runs `ldd` on the provider library and fails on a
    missing library. No CUDA toolkit or compiler is in the runtime image.
21. Host requirements: NVIDIA driver that supports CUDA 13 (driver 580 or
    later) and the NVIDIA Container Toolkit. If the production host must keep
    an older driver, the image uses the CUDA 12 variant (option 3 of the
    image base). This decision changes only the image, not the code.
22. Compose service `cuda-stt` with the profile `cuda`, host network, the
    hardened defaults, a read-only root file system, the writable `models`
    volume, and:

    ```yaml
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              device_ids: ["${TALKOPS_CUDA_DEVICE:-0}"]
              capabilities: [gpu]
    ```

    The file has no `build:`.

### TalkOps

23. A new connection kind `cuda` in `transcription_providers` (new
    migration): loopback URL (default `http://127.0.0.1:8096/v1`),
    optional key, no costs, no cloud privacy note. Core gets one check for
    "local speech service" (`coral` or `cuda`), so the media worker uses the
    same model choice (`*-auto`), language labels, realtime client,
    30-second backoff and fallback to local Whisper for both kinds.
24. The "Apply hybrid" action works with a `cuda` connection as with Coral.
25. API: the model library routes become generic for local speech services
    (`/api/v1/speech-services/{id}/...`) and accept connections of kind
    `coral` and `cuda`. The Coral routes of ADR 0048 stay as aliases until a
    release removes them. Permission `transcription.settings`, audit entries
    as ADR 0048. The upload route streams the body.
26. UI: `CoralModels.svelte` becomes one component for both kinds. The kind
    selects the slots, the accepted file types, the URL install and the
    VRAM column. Diagnostics use the same component for both kinds.

## Consequences

- Larger models with a better expected text quality can run in the live
  preview and the first pass. Whether a model meets the latency target on a
  given GPU is a measurement, not a promise.
- The image is large (cuDNN and cuBLAS have several hundred megabytes each).
  It is only pulled with the profile `cuda`.
- The host needs a current NVIDIA driver and the NVIDIA Container Toolkit.
  Docker Desktop with WSL2 supports GPUs, but host networking in Docker
  Desktop needs a current version. The Windows desktop is a test system,
  not a supported production host.
- sherpa-onnx, ONNX Runtime and CUDA versions are coupled through the GPU
  release archive. An update changes all three together and needs the
  acceptance measurements again.
- One process per GPU keeps the code simple. Two GPUs need two connections.
- The refactor into `talkops-stt-service` touches `coral-stt`. It must keep
  the Coral interface and tests unchanged.
- Models that TalkOps may not redistribute can still be used: the
  administrator installs them and is responsible for the license.
- Without the profile `cuda` and without a `cuda` connection, nothing changes.

## Acceptance measurements (before "Accepted")

On the test desktop (T600, 4 GB) and on the production GPU, with the same
200 test clips, text normalization and turn settings as the Coral report:

- WER and CER for German telephone audio (8 kHz G.711 resampled to 16 kHz)
  per candidate model and precision (fp32, fp16, int8).
- Live: p50 and p95 latency from end of speech to final text and to the
  first `delta`, at 1, 2 and 4 parallel sessions.
- VRAM per model and maximum parallel sessions before overload.

## Open questions

- Which GPU (model, VRAM) and which NVIDIA driver version does the
  production host have? This decides CUDA 13 or CUDA 12, the default
  models and the session limit.
