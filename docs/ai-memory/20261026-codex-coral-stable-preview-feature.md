# Optional complete-word Coral live preview

## Task state

- Task ID: 20261026-codex-coral-stable-preview-feature
- Date: 2026-10-26
- Tool: Codex
- Status: Implemented and tested; PR publication pending
- Branch: codex/coral-stable-preview
- Base commit: 3d11790618ed99c70fc5b7864c5be363e8a969d7
- Related PR: Pending; depends on https://github.com/itsh-neumeier/talkops/pull/81

## Goal

The user requested implementation as a feature through a PR after the
actual Coral latency and quality measurements.

## Result

Implemented an optional complete-word preview in the Rust Coral service.
The setting `TALKOPS_CORAL_STABLE_PREVIEW` defaults to `false` and accepts
`true` to enable the feature. Compose forwards this setting. Session
configuration reports it; a client cannot change the operator setting.

The preview keeps immutable centre frames of the original model windows.
It holds the final open CTC word until a space closes it or the client
commits the turn. The existing worker still detects pauses and starts new
turns. The service checks that every later partial and final result keeps
all published complete words. A conflict returns the existing inference
error, which activates the existing worker fallback path.

Model weights, input shapes, final file decoding, retention, and fallback
policy are unchanged. Initial artificial silence is not implemented because
it traded recognition quality for earlier startup in the measured sample.

## Changes

- `crates/talkops-coral-stt/src/recognizer.rs`: complete-word stable prefix.
- `crates/talkops-coral-stt/src/realtime.rs`: optional mode and prefix checks.
- `crates/talkops-coral-stt/src/http.rs` and `src/main.rs`: service setting.
- `crates/talkops-coral-stt/tests/service.rs`: opt-in, WebSocket deltas before
  commit, final text parity, clear and short-turn completion.
- `.env.example` and `docker-compose.yml`: explicit opt-in configuration.
- `docs/en/active-calls.md` and `docs/de/laufende-gespraeche.md`: activation,
  timing limits and rollback.
- `docs/research/coral-results/20261026-preview-usb/`: aggregate records from
  the earlier local Python benchmark task, with no audio or transcripts.

## Decisions

- Use a focused branch based on the verified head of PR #81. The earlier
  local research branch `codex/coral-transcription` remains intact at 7f24f65.
  Its unrelated CUDA and training work is not included in this feature PR.
- Keep this feature opt-in. The measured Python prototype had 28.68% WER
  with pause resets, versus 22.26% for the matched independent clips.
  Four of 41 reference intervals lacked an associated output word.
- Treat about 4–5 s as a prototype budget. It is not a service guarantee.
  The Rust service has not repeated the hardware WER/latency measurement.
- Preserve ADR 0046 and the existing loopback service and worker protocol.
  No migration, API authorization or UI permission change is needed.

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo test --locked -p talkops-coral-stt`: 20 unit tests and six
  service tests. One real-model TFLite test was ignored because its required
  C runtime and model setup were not supplied to the Rust container.
- Passed: `cargo clippy --locked -p talkops-coral-stt --all-targets -- -D warnings`.
  Rust checks used the official image at digest
  `sha256:931c2c0531a8b143e044f0f3914dfe802f15bb3d1955f0b657e9359327ac2ffc`.
- Passed: Compose `coral` profile validation with test credentials, and
  assertion that `TALKOPS_CORAL_STABLE_PREVIEW=true` reaches the service.
- Passed: `node tests/docs/wiki.mjs`: all 13 tests in Linux.
- Passed: `node scripts/docs/build-wiki.mjs --check`: 75 pages and 684 links.
- Resolved local check limitation: Windows denied the Wiki symlink fixture;
  the same documentation test passed in the Linux Node 22 container.
- Not run: complete workspace tests, SIP stack and physical TPU Rust service
  replay. The local service tests use the mock backend.
- STE review: repository writing rules used; full standard unavailable,
  full language review pending.

## Dependencies and open work

PR #81 provides the Coral service and is still a draft. The feature PR
targets its branch and must be integrated after that dependency. Hardware
quality approval remains open. No merge, release or deployment is authorized.

## Next action

Publish this feature branch and open a PR with dependency and hardware
limits in its description. The implementation has not been deployed.
