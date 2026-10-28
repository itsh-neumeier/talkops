# CUDA GPU speech service `cuda-stt`

## Task state

- Task ID: 20261028-claude-cuda-stt
- Date: 2026-10-28
- Tool: Claude Code
- Status: Completed (implementation); acceptance measurements pending
- Branch: claude/cuda-stt (based on `claude/coral-model-upload`, PR #91)
- Base commit: 039284c (head of `claude/coral-model-upload`)
- Related commits or PR: Pending

## Goal

Add a service `cuda-stt` with the same functions as `coral-stt` on an NVIDIA
GPU: OpenAI-compatible transcription, realtime live preview (target 3 to 5 s
latency for German calls), diagnostics, and the administrator model library
with license and source statements. Add the connection kind `cuda` to
TalkOps (API, media worker fallback, UI).

## Result

- ADR 0049 (proposed, implemented; acceptance measurements pending).
- New crate `talkops-stt-service`: audio, turn split, diagnostics, the
  OpenAI-compatible routes, the realtime protocol and library helpers,
  generic over the traits `SpeechEngine` and `SpeechTurn`. `coral-stt`
  uses it; its interface and tests did not change.
- New crate `talkops-cuda-stt`: sherpa-onnx backend (`cuda`, `cpu`), mock
  backend, engine with batching, turn recognizer (re-decode per interval,
  cut of long turns), model library (archive upload, HTTPS download with
  SHA-256, child-process load check, slot selection), NVML GPU status.
- Image `docker/cuda-stt/Dockerfile`: Debian trixie, NVIDIA Debian 13
  repository, CUDA 13.1 runtime, cuBLAS, cuRAND, cuDNN 9.25, sherpa-onnx
  1.13.8 GPU archive (ONNX Runtime 1.28.2). Size about 3.2 GB.
- TalkOps: migration `20261114000001` (kind `cuda`), core `is_local_kind`,
  API `/api/v1/speech-services/{id}/...` (replaces the unreleased Coral
  routes of PR #91), diagnostics list both kinds, UI `SpeechModels.svelte`
  replaces `CoralModels.svelte`, settings with preset, GPU status, hybrid.
- Compose profile `cuda`, `.env.example`, CI image build and test
  (`tests/cuda/image.sh`), release workflow (amd64 only).
- Presets `presets/stt-models/cuda.json`: 7 models with GitHub-digest
  checked SHA-256 (Whisper tiny digest computed locally).
- Docs: `docs/en/recording.md`, `docs/de/aufzeichnung.md` section
  "NVIDIA GPU (this server)".

## Decisions

- Runtime sherpa-onnx through the official `sherpa-onnx` crate (`shared`
  feature, `SHERPA_ONNX_LIB_DIR` = GPU archive in the image; CI downloads
  the CPU archive). Reason: one maintained library for all model families.
- The user confirmed on 2026-10-28: production host driver 580+ (CUDA 13).
  The production GPU model is still unknown.
- Debian 13 NVIDIA repository has CUDA 13.1+ only (no 13.0, no CUDA 12
  runtime). Pinned 13.1 for the widest driver 580 compatibility.
- No VAD model slot: long live turns are cut at the quietest 100 ms of the
  last 5 s (energy). Files use the shared energy turn split.
- One process per GPU (sherpa-onnx C API has no device index).
- Library keeps full-precision files by default: with the CUDA provider some
  int8 operators run on the CPU.
- Engine field `coral` in `transcription_profiles::Engine::Cloud` and the
  column `coral_stable_preview` keep their names; they now mean "local
  speech service" (coral or cuda). Renaming them is possible follow-up work.

## Checks

Rust in `rust:1-trixie`, web in `node:22-trixie` (no local toolchains).

- Passed: `cargo fmt --all`; `cargo clippy --workspace --all-targets
  --locked -- -D warnings`.
- Passed: `cargo test -p talkops-coral-stt -p talkops-stt-service -p
  talkops-cuda-stt` (coral 17 unit + 8 service, shared 12, cuda 13 unit +
  9 service).
- Passed: `cargo test --workspace --locked --no-fail-fast` with Postgres 17:
  582 tests passed; 1 failed (the fax test below, root in the container).
- Passed (ignored test, manual): real models on the CPU with sherpa-onnx:
  FastConformer DE transducer int8, Whisper tiny (8 kHz input), Canary 180M
  int8 (German) gave the expected text of the sherpa-onnx test files.
- Passed (manual, T600 4 GB, Docker Desktop WSL2, driver 595.79): image
  with `--gpus device=0`, upload of Whisper tiny and FastConformer DE int8,
  selection, restart through the restart policy, transcription
  "Alles hat ein Ende, nur die Wurst hat zwei."; HTTPS download of the
  fp32 transducer preset with SHA-256 check (about 1 min), loaded with 698
  MiB VRAM (NVML, other processes on the GPU), 2.75 s file in about 0.11 s
  after warm-up. One file only: not a WER or latency result.
- Passed: `tests/cuda/image.sh` library and user checks; the mock service
  health was checked inside the container network (Docker Desktop host
  network is not reachable from Windows).
- Passed: `npm run check` (0 errors), `npm test` (91), Prettier for changed
  files, `node tests/docs/wiki.mjs` (13), wiki build with explicit revision.
- Passed: `docker compose config -q` with `COMPOSE_PROFILES=cuda`.
- Not run: realtime latency with a real model on the GPU, browser tests
  (`tests/browser/*.mjs`), SIPp end-to-end, the release workflow.
- Known local failure: `read_only_fax_directory_rejects_upload_without_queueing`
  fails when the test container runs as root (root ignores mode 0555). Not
  related to this change; CI runs as a normal user.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- PR #91 (Coral model library) must merge first or together; this branch
  contains its commits.
- Acceptance measurements (ADR 0049): WER/CER on 200 German telephone clips,
  p50/p95 live latency at 1, 2, 4 sessions, VRAM per model, on the T600 and
  the production GPU.
- Production GPU model and VRAM (ask the operator).
- Optional: rename the `coral` engine field to a neutral name.

## Next action

Review and merge the PR; then run the acceptance measurements on the T600.

## References

- ADR 0046, ADR 0048, ADR 0049.
- `20261028-claude-coral-model-upload.md`,
  `20261028-claude-coral-german-model-evaluation.md`.
