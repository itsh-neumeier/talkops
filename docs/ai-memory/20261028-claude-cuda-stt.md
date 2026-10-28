# CUDA GPU speech service `cuda-stt`

## Task state

- Task ID: 20261028-claude-cuda-stt
- Date: 2026-10-28
- Tool: Claude Code
- Status: In progress (Phase 1 complete; waits for the production GPU answer)
- Branch: claude/cuda-stt (based on `claude/coral-model-upload`, PR #91)
- Base commit: 039284c (head of `claude/coral-model-upload`)
- Related commits or PR: Pending

## Goal

Add a service `cuda-stt` with the same functions as `coral-stt` on an NVIDIA
GPU: OpenAI-compatible transcription, realtime live preview with 3 to 5 s
latency for German calls, diagnostics, and the administrator model library
with license and source statements. Add the connection kind `cuda` to
TalkOps (API, media worker fallback, UI).

## Result

- Phase 1: ADR 0049 (proposed) written. No code changed yet.

## Changes

- `docs/adr/0049-cuda-gpu-speech-recognition.md`: new ADR.
- `docs/adr/README.md`: index row.

## Decisions

- Runtime: sherpa-onnx with ONNX Runtime CUDA provider, through the official
  `sherpa-onnx` crate (`shared` feature, `SHERPA_ONNX_LIB_DIR` set to the
  pinned GPU release archive). Reason: one maintained library supports NeMo
  CTC, transducer/TDT, Canary, Whisper, Silero VAD and Whisper language ID;
  no FFI in TalkOps. Rejected: own `ort` pipeline (much own decoder code),
  faster-whisper (Whisper only, Python), NeMo (image size, VRAM), Riva/NIM
  (license).
- The sherpa-onnx C API has no GPU index: one process per GPU
  (`CUDA_VISIBLE_DEVICES` via Compose `device_ids`).
- Image: `debian:trixie-slim` plus the NVIDIA CUDA apt repository for
  Debian 13 (CUDA 13 runtime, cuDNN 9). The Debian 13 repository has no
  CUDA 12 runtime (checked 2026-10-28). CUDA 13 needs a host driver 580+.
- Live preview: offline models, re-decode of the growing turn every 1 s,
  15 s turn limit with Silero VAD split.
- Library: ONNX directory + `tokens.txt` + `model.json`; slots `de`, `en`,
  `langid`, `vad`; selection file instead of slot copies; URL download with
  SHA-256 as main install method, streamed upload as second method.
- Shared crate `talkops-stt-service` for code that both services use.
- Generic API routes `/api/v1/speech-services/{id}/...`; Coral routes stay as
  aliases.

## Checks

- Not run: no code yet.
- Verified by query: sherpa-onnx v2.0.1 (2026-10-26) publishes
  `cuda-12.x` and `cuda-13.x` `cudnn-9.x` `onnxruntime1.28.2` Linux GPU
  archives; v1.13.8 (2026-09-10) uses the same ORT version.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- PR #91 (Coral model library) must merge first or together; this branch
  is based on it.
- Ask the user: production host GPU model, VRAM and driver version.
- Phase 2+: shared crate, `cuda-stt` crate with mock/cpu/cuda backends,
  Dockerfile, Compose, migration for kind `cuda`, worker, API, UI, docs
  (en/de), tests, PR.
- Acceptance measurements on T600 and production GPU (ADR 0049).

## Next action

Get the production GPU answer, then implement Phase 2.

## References

- ADR 0046, ADR 0048, ADR 0049.
- `20261028-claude-coral-model-upload.md`,
  `20261028-claude-coral-german-model-evaluation.md`.
