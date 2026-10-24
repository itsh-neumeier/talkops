# Coral Edge TPU speech recognition (German/English)

## Task state

- Task ID: 20261024-claude-coral-stt
- Date: 2026-10-24
- Tool: Claude Code
- Status: Blocked at the Phase 0 gate. Measurements on Edge TPU hardware are pending.
- Branch: claude/lucid-wright-lhtgr1
- Base commit: 8f32ec7778ec7a147c09ac98c97c5f1a434e22cc (origin/main, release 1.8.10)
- Related commits or PR: Pending (draft PR)

## Goal

Let TalkOps transcribe German and English speech on a Google Coral Edge TPU
(USB Accelerator or M.2/Mini PCIe) in the TalkOps server. The Coral does the
live preview and the first pass of recordings and voicemails. Local Whisper
does the second pass (hybrid, ADR 0019). The user specified 7 phases:
Phase 0 feasibility gate, then service `coral-stt`, integration (ADR,
migration, worker), fallback, UI, deployment and documentation.

## Result

Phase 0 tools and the report frame are complete. The session had no Edge TPU
hardware, so no gate measurements exist. As the user instructed, the work
stops at the gate and asks the user for the measured values. Phases 2 to 7
did not start.

## Changes

- `tools/coral/`: offline tools (Python) for model export (NeMo to int8
  TFLite with fixed windows), language ID training, Edge TPU compilation and
  op mapping, telephone channel simulation (G.711), file and live
  benchmarks against whisper.cpp `base`, host/device discovery, gate
  evaluation and report tables. `candidates.json` lists models and licenses.
- `tools/coral/probe/`: standalone Rust probe (not a workspace member) that
  runs a compiled model through `edgefirst-tflite` 0.10.3 with
  `unsafe_code = "forbid"`.
- `tools/coral/tests/`: 25 unit tests (stdlib tests in CI; numpy tests skip
  without numpy).
- `docs/research/coral-stt-benchmark.md`: method, gate, candidates,
  licenses, runtime binding, result sections with markers, alternatives.
- `.github/workflows/ci.yml`: job `coral-tools` runs the unit tests.
- `CLAUDE.md`: `tools/coral/` in the repository layout.

## Decisions

- Phase 0 is a hard gate (user). Without hardware: build tools and report,
  then ask the user for values. Do not start the service before the gate.
- Gate limits: backlog <= 1 s for one call (2 channels); segment latency
  p95 < 1.5 s; host CPU per channel <= 0.5 x whisper.cpp `base`; manual
  readability rating (WER <= 35 % is a guide only).
- Runtime binding: `edgefirst-tflite` (Apache-2.0) loads libedgetpu as an
  external delegate through safe API. No FFI exception ADR is necessary if
  the probe runs on hardware. `tflitec` has no external delegate API;
  `tflite-c-rs` is 0.0.1.
- Candidates: `en-citrinet-256` (CC-BY-4.0, verified), `en-quartznet-15x5`
  and `de-quartznet-15x5` (NGC license to verify), fallback
  `de-citrinet-256-ft` (own fine-tune). Language ID: own small CNN on CC0
  data, or CTC confidence of both models.
- The live benchmark uses a Python port of the worker turn detector
  (`realtime.rs`), checked with the same test scenarios.
- CPU comparison is per channel, so runs with a different number of calls
  compare correctly.

## Checks

- Passed: `python3 -m unittest discover -s tools/coral/tests -v` (25 tests).
- Passed: CPU smoke run in a scratch virtual environment (TensorFlow 2.21,
  ai-edge-litert, Python 3.13) with a tiny synthetic int8 model:
  `prepare_audio.py`, `train_langid.py`, `bench_file.py` (fixed,
  classifier, confidence), `bench_live.py --calls 2`, `gate_report.py` on a
  copy of the report.
- Passed: `cargo build` and `cargo clippy -- -D warnings` in
  `tools/coral/probe`. Without a TFLite library the probe stops with a load
  error, as expected.
- Passed: `node tests/docs/wiki.mjs`; `node scripts/docs/build-wiki.mjs --check`.
- Not run: `export_nemo.py` with a real NeMo checkpoint (NeMo not installed;
  large download). `compile_model.py` with `edgetpu_compiler` (not
  installed). All hardware measurements (no Edge TPU). Rust workspace checks
  (no workspace code changed).
- STE review: repository rules only. Full standard unavailable; review pending.

## Dependencies and open work

- No migration. No change to server, worker or images.
- Open: user measurements on USB and M.2/PCIe (report section 3 and
  `tools/coral/README.md`), NGC license check, probe run on hardware.
- After a passed gate: Phases 2 to 7 (service `coral-stt`, new ADR that
  supplements ADR 0024, migration for the connection kind `coral`, worker
  fallback, UI, Compose profile `coral`, CI image, documentation).

## Next action

The user runs the tools on the hardware and supplies the result files or
values. Then run `gate_report.py`, record the gate decision in the report,
and continue with Phase 2 only if the gate passes.

## References

- ../research/coral-stt-benchmark.md
- ../../tools/coral/README.md
- 20261013-claude-transcription-engines.md
- ../adr/0024-transcription-engines-per-purpose.md
