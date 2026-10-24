# 0046 - Speech recognition on Coral Edge TPUs

- Status: Accepted. The hardware measurements of the Phase 0 gate are
  pending (see [the benchmark report](../research/coral-stt-benchmark.md)).
- Date: 2026-10-24
- Supplements [0024](0024-transcription-engines-per-purpose.md). Also
  applies [0019](0019-transkription-in-zwei-durchgaengen-und-ki-api.md)
  (two passes) and [0022](0022-active-call-identities-and-live-previews.md)
  (live preview).

## Context

The live preview and the first pass of recordings and voicemails use
whisper.cpp on the CPU or a cloud connection. A Google Coral Edge TPU (USB
Accelerator or M.2/Mini PCIe card) is a small, low-power accelerator that
some operators already have in their server. The operator wants German and
English speech recognition on this TPU, mainly for the live preview, with
local Whisper for the accurate second pass (hybrid).

Whisper cannot run on the Edge TPU. The Edge TPU runs only full-integer
TensorFlow Lite models with fixed tensor shapes and has about 8 MB of
parameter cache. Small CTC models with fixed windows and a small language
identification (language ID) classifier can run on it.

The workspace forbids `unsafe` code. The TPU runtime is a C library.

## Options

1. Run the TPU inside the media worker.
2. **A separate service `coral-stt`** with its own image, and the
   OpenAI-compatible interface that the worker already uses (ADR 0024).
3. Our own FFI bindings to TensorFlow Lite and libedgetpu.
4. **The maintained safe wrapper crate `edgefirst-tflite`** (Apache-2.0),
   which loads `libtensorflowlite_c.so` and external delegates at run time.
5. A Python inference helper (pycoral, tflite_runtime).

## Decision

- **Options 2 and 4.**
- The crate `crates/talkops-coral-stt` and the image
  `ghcr.io/itsh-neumeier/talkops-coral-stt` hold the TPU libraries and
  the models. The media worker image does not change.
- The service listens on `127.0.0.1:8095` by default (configurable; a
  warning is logged for other addresses). The Compose service uses the
  host network and the profile `coral`. Audio does not leave the server.
- Interface (a subset of the OpenAI interface):
  - `GET /health`: status, TPUs (kind, state, load, loaded models), queue.
  - `GET /v1/models`: `coral-ctc-de`, `coral-ctc-en`, `coral-auto`.
  - `POST /v1/audio/transcriptions`: multipart; `json`, `text` and
    `verbose_json` with segments, times and the detected `language`
    (ISO 639-1, also for each segment). `language` is the preferred
    language for `coral-auto`.
  - `GET /v1/realtime?intent=transcription`: the events of the TalkOps
    realtime client (`session.update` with PCM16, `turn_detection: null`
    and `transcription.languages` in order of preference;
    `input_audio_buffer.append`, `.commit`, `.clear`; answers `committed`,
    `delta`, `completed` with the extra field `language`, `failed`,
    `error`).
  - Overload: HTTP 503 with `Retry-After`, or an `error` event with the
    code `overloaded`.
- Each TPU has its own thread and loads all models. One queue with a fixed
  capacity feeds the TPUs; live windows come before file windows. The
  number of realtime sessions is limited per ready TPU. A supervisor finds
  new, replugged and failed devices every 15 seconds. USB devices are
  identified by index (`usb:0`), so the renumbering after the firmware
  load does not change their identity.
- The host computes the log-mel features and decodes the CTC output. The
  language of a turn comes from the language ID model on the first window;
  without that model, all language models run and the higher CTC
  confidence wins.
- TalkOps stores the connection as the kind `coral` in
  `transcription_providers` (migration `20261112000001`): a loopback URL
  (default `http://127.0.0.1:8095/v1`), an optional key, no costs and no
  cloud privacy note. A later external CoralAPI server can replace the
  service with no change to the worker; it needs a new decision about the
  loopback rule and the privacy note.
- Fallback: when the Coral service is not reachable, overloaded or the
  stream breaks, the live preview uses local Whisper `base` in 8-second
  parts, and recordings and voicemails use the local model of their
  profile. `transcripts.engine` holds the model that made the text and
  `transcripts.fallback_from` the engine it replaced. The live preview
  shows the fallback (`live_transcript_previews.fallback_from`). Coral is
  tried again after the 30-second backoff of ADR 0024.
- The detected language is stored in `transcripts.language` and for each
  line (`segments[].language`) and is shown in the UI.
- The button "Apply hybrid" sets `live` to Coral Realtime, `recording` and
  `voicemail` to Coral, and the second passes to local Whisper.
- Runtime libraries in the image: TensorFlow Lite C v2.19.1 (built from
  the pinned commit) and libedgetpu 16.0 for TensorFlow 2.19.1 (community
  build for Debian trixie, pinned by checksum).

## Consequences

- No `unsafe` code in TalkOps; the FFI stays in a maintained dependency.
- Without the profile `coral` and without a Coral connection, nothing
  changes. Local Whisper remains the default.
- The text quality of small CTC models on telephone audio is lower than
  Whisper's. The hybrid setup replaces the text in the second pass.
- The image does not contain models. The operator compiles them with
  `tools/coral/` and puts them into the models volume. Model licenses must
  permit the redistribution (see the report).
- libedgetpu and TensorFlow Lite must have matching versions. An update of
  one needs an update of the other.
- The USB setup binds `/dev/bus/usb` and allows USB character devices
  (major 189) in the container, so the renumbered device works without a
  restart.
- The quality and capacity numbers depend on the Phase 0 measurements on
  real hardware. If the gate fails, the report proposes alternatives (for
  example Coral only for voice activity detection and language ID).
