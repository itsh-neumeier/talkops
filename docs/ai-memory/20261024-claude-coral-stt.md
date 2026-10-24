# Coral Edge TPU speech recognition (German/English)

## Task state

- Task ID: 20261024-claude-coral-stt
- Date: 2026-10-24
- Tool: Claude Code
- Status: Implemented (Phases 0 to 7). The hardware measurements of the
  Phase 0 gate are pending.
- Branch: claude/lucid-wright-lhtgr1
- Base commit: 8f32ec7778ec7a147c09ac98c97c5f1a434e22cc (origin/main, release 1.8.10)
- Related commits or PR: [PR 81](https://github.com/itsh-neumeier/talkops/pull/81)

## Goal

Let TalkOps transcribe German and English speech on a Google Coral Edge TPU
(USB Accelerator or M.2/Mini PCIe) in the TalkOps server. The Coral does the
live preview and the first pass of recordings and voicemails. Local Whisper
does the second pass (hybrid, ADR 0019). The user specified 7 phases:
Phase 0 feasibility gate, service `coral-stt`, integration (ADR,
migration, worker), fallback, UI, deployment and documentation.

## Result

- Phase 0: offline tools in `tools/coral/` and the report frame
  `docs/research/coral-stt-benchmark.md`. No hardware was available.
- The user decided (chat, 2026-10-24) to implement Phases 2 to 7 before the
  measurements ("make the PR complete; if not, we patch afterwards").
- Phases 2 to 7 are implemented, tested without hardware and pushed.
- A user question about a prerelease `v1.8.11-alpha` got a prompt for the
  release chat; no tag was set (the release chat owns tags).

## Changes

- `crates/talkops-coral-stt/`: new service. Device discovery (sysfs USB in
  both states, `/dev/apex_*`), one thread per TPU, shared queue with
  capacity and live priority, session limit per TPU, supervisor rescan.
  Log-mel front end, fixed CTC windows with stitching, language ID or CTC
  confidence. `/health`, `/v1/models`, `/v1/audio/transcriptions`
  (verbose_json with `language` per segment), `/v1/realtime`. Overload:
  HTTP 503 + `Retry-After`, `error` with code `overloaded`. Backends: `tpu`
  and `cpu` (edgefirst-tflite 0.10.3), `mock` (CI).
- `migrations/20261112000001_coral_transcription.sql`: kind `coral`,
  `transcripts.fallback_from`, `live_transcript_previews.engine` and
  `fallback_from`.
- `talkops-core`: `transcription_api` (kind `coral`, loopback rule,
  `CORAL_URL`, `config` with optional secrets), `transcription_profiles`
  (`Engine::Cloud { coral, fallback }`, `Profile.provider_kind`),
  `recordings` (`Segment.language`, `fallback_from`), `live_transcripts`
  (`set_engine`, `stream_failed_recently`), `transcription_jobs`.
- `talkops-media-worker`: `run_engine` returns `Transcribed` with engine,
  fallback and language; Coral fallback with a 30 s in-memory backoff;
  live fallback (base in parts) while a Coral stream failed in the last
  30 s; Coral session languages (tenant language first); language from
  `completed` and verbose_json; no usage for Coral.
- `talkops-api`: connection test reads `/health` and `/v1/models` for Coral.
- `web/`: connection kind "Coral (dieser Server)", TPU status, Coral models,
  "Hybrid übernehmen", fallback model select, no price or privacy note for
  Coral, fallback and language display in transcripts and the live popup.
- `docker/coral-stt/Dockerfile`, `docker-compose.yml` (profile `coral`),
  `docker-compose.dev.yml`, `.env.example`, `tests/coral/image.sh`,
  `.github/workflows/ci.yml` (image build, compose check, browser test,
  `coral-tools` job), `.github/workflows/release.yml` (fourth image).
- Docs: ADR 0046, `docs/architecture.md`, `docs/en/recording.md`,
  `docs/en/active-calls.md`, `docs/de/aufzeichnung.md`,
  `docs/de/laufende-gespraeche.md`, report, `tools/coral/README.md`.

## Decisions

- Safe binding: `edgefirst-tflite` loads `libtensorflowlite_c.so` and
  libedgetpu at run time; no FFI exception ADR.
- Image libraries: TensorFlow Lite C v2.19.1 built from commit
  `f4247ebb…` with XNNPACK off (its CMake build fails in 2.19); libedgetpu
  16.0 for TF 2.19.1 from feranick (trixie debs, SHA-256 pinned). Debian
  trixie has no packages for either.
- Coral connections must use a loopback URL. An external CoralAPI server
  needs a new decision (loopback rule, privacy note).
- Fallback: live = local `base` in parts; recording/voicemail = the
  profile's `local_model`. Any Coral error triggers the fallback.
- Language: ISO 639-1 codes; `transcripts.language` takes the detected main
  language only for Coral; other engines keep the tenant language
  (unchanged behaviour).
- USB in Docker: bind `/dev/bus/usb` plus cgroup rule `c 189:* rmw`, so the
  renumbered stick works without a restart.

## Checks

- Passed: `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D
  warnings`.
- Passed: `cargo test --workspace` against local PostgreSQL 16 except one
  test, see Failed. New: coral-stt unit (16) and service tests (5, mock),
  core sqlx tests `coral_connections_and_fallback`,
  `fallback_and_language_are_stored`, API test `coral_connection`, worker
  tests `coral_recordings_use_the_service_or_the_local_fallback` and
  `coral_live_preview_falls_back_and_retries` (all fallback paths:
  overloaded, broken stream, unreachable, backoff, retry).
- Passed: `crates/talkops-coral-stt/tests/tflite.rs` (ignored test) with a
  locally built `libtensorflowlite_c.so` 2.19.1 and small int8 test models
  on the CPU.
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`;
  browser tests `coral-transcription.mjs`, `transcription-settings.mjs`,
  `active-calls.mjs`, `transcriptions.mjs`, `call-presentation.mjs`,
  `layout.mjs`, `settings-search.mjs`, `call-list.mjs`,
  `dashboard-collapse.mjs`.
- Passed: `docker compose config -q` (base, profile coral, dev override);
  `node tests/docs/wiki.mjs`; `node scripts/docs/build-wiki.mjs --check`;
  `python3 -m unittest discover -s tools/coral/tests` (25).
- Failed (environment only): `talkops-api` test
  `read_only_fax_directory_rejects_upload_without_queueing`. The local tests
  ran as root, and root ignores the read-only directory mode. The change
  does not touch fax code. CI runs as a normal user.
- Not run: Docker image build (the build needs the session proxy CA in each
  stage; CI builds it); E2E SIPp; all hardware checks (Edge TPU USB/PCIe,
  gate measurements); a real NeMo export.
- STE review: repository rules only. Full standard unavailable; review pending.

## Dependencies and open work

- Migration ID: 20261112000001_coral_transcription. Update server, worker
  and web together.
- New crate dependency: edgefirst-tflite 0.10.3 (+ edgefirst-tflite-sys,
  libloading). The release workflow now publishes four images.
- Open: Phase 0 measurements on hardware and the gate decision; NGC license
  check of the QuartzNet checkpoints; models are not shipped.
- Open: verify the gasket-driver fork steps and the LXC lines on real
  hosts.

## Next action

The user runs `tools/coral/` on the hardware and reports the values. Then
fill in the report with `gate_report.py` and tune the defaults (queue,
sessions per TPU, window) or apply the alternatives of the report.

## References

- ../adr/0046-coral-edge-tpu-speech-recognition.md
- ../research/coral-stt-benchmark.md
- ../../tools/coral/README.md
- 20261013-claude-transcription-engines.md
