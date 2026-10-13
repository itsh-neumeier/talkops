# Transcription engines per purpose, OpenAI cloud models and realtime previews

## Task state

- Task ID: 20261013-claude-transcription-engines
- Date: 2026-10-13
- Tool: Claude Code
- Status: Completed. The PR records the readiness.
- Branch: claude/cool-pasteur-8btt22
- Base commit: 21f5a5158978bfdddb86c293cdf4688c73223e5b (origin/main, release 1.7.7)
- Related commits or PR: [PR 14](https://github.com/itsh-neumeier/talkops/pull/14)

## Goal

Let the administrator connect OpenAI speech models with an API key. Let the
administrator select the model for each purpose: live preview, recording
(fast and slow), voicemail, local or cloud. The user approved the proposal
and also asked for two follow-up items in the same task: realtime streaming
for the live preview and a cost display per minute.

## Result

Implemented on the feature branch:

- Several cloud connections for each tenant (`openai` with a fixed URL, or
  `compatible` with an own URL). The key is encrypted.
- One engine profile for each purpose: `live`, `recording`,
  `recording_refine`, `voicemail`, `voicemail_refine`. Each profile is local
  (Whisper model) or cloud (connection and model). Second passes can be off.
- Cloud live preview in 8-second parts, or streamed over the Realtime API
  (WebSocket, one session for each channel, client-side turn detection).
- Usage of cloud audio for each day, purpose and model, with an estimated
  cost from the profile price or the OpenAI list price.
- Settings UI with engines, connections and usage. The old API component is
  removed.

## Changes

- `migrations/20261028000001_transcription_profiles.sql`: New tables
  `transcription_providers`, `transcription_profiles`, `transcription_usage`.
  Stream lease columns in `live_transcript_previews`. Copies the old settings.
- `crates/talkops-core/src/transcription_api.rs`: Connections (CRUD, key
  encryption, realtime URL, label).
- `crates/talkops-core/src/transcription_profiles.rs`: Purposes, profiles,
  engine selection, OpenAI list prices, usage and its cleanup.
- `crates/talkops-core/src/settings.rs`: Remove `transcription_quality` and
  `transcription_refine` from `TenantSettings`. Add the local model `base`.
- `crates/talkops-core/src/live_transcripts.rs`: Stream lease functions and the
  failed status of a stream.
- `crates/talkops-api/src/routes/transcription.rs`: New endpoints under
  `/api/v1/settings/transcription` (overview, profiles, providers, test).
  The old `/api/v1/settings/transcription-api` endpoints are removed.
- `crates/talkops-media-worker/src/main.rs`, `live.rs`, `api.rs`: Engine per
  purpose and pass. Usage recording. The live preview can use cloud models.
- `crates/talkops-media-worker/src/realtime.rs`: Realtime streaming.
- `web/src/lib/components/TranscriptionSettings.svelte`: New settings UI.
  `LiveTranscript.svelte` updates the text of a segment with the same times.
- `tests/browser/transcription-settings.mjs` and `.github/workflows/ci.yml`:
  New browser check in the web CI job.
- Docs: `docs/adr/0024-transcription-engines-per-purpose.md`,
  `docs/architecture.md`, `docs/en/recording.md`, `docs/en/active-calls.md`,
  `docs/de/aufzeichnung.md`, `docs/de/laufende-gespraeche.md`.

## Decisions

- Several connections (recommended in the proposal; the user did not object).
- Voicemail has its own optional second pass.
- Live local models are limited to `base` and `fast`.
- Realtime: `wss://…/v1/realtime?intent=transcription`, `session.type =
  transcription`, PCM16 24 kHz, `turn_detection: null`. `gpt-live-transcribe`
  does not support server VAD, so the worker detects turns (300 ms lead-in,
  700 ms silence ends a turn, 15 s maximum). Only speech is sent. Newer models
  get `languages` and `keywords`; older models get `language` and `prompt`.
- The stream runs as a background task with a database lease (20 s), not in
  the job lane. It stops when the viewer leaves, when the file does not grow
  for 20 s, or after 4 h. A failed stream pauses for 30 s.
- OpenAI list prices from https://developers.openai.com/api/docs/pricing,
  checked 2026-10-13. Costs are estimates in USD.
- The old `tenant_settings` columns stay (migrations are forward-only).
- A provider delete in use returns HTTP 409 with the purposes.

## Checks

- Passed: `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D
  warnings`.
- Passed: `DATABASE_URL=… cargo test --workspace` (all suites, local
  PostgreSQL 16). New: 5 core integration tests (including two migration
  data tests), stream lease test, 5 realtime unit tests, worker end-to-end
  test against a fake Realtime WebSocket server, updated API test.
- Passed: `npm run lint`, `npm run check`, `npm run build` in `web/`.
- Passed: `tests/browser/transcription-settings.mjs` and
  `tests/browser/active-calls.mjs` with Playwright 1.56.1 and the local preview.
- Not run: Docker/SIPp E2E (`tests/e2e/run.sh`). GitHub Actions runs it.
- Not run: A real OpenAI request. No API key was available. The realtime
  protocol follows the OpenAI documentation of 2026-10 and a fake server.
- STE review: Repository rules only. The full standard was unavailable;
  full language review is pending.

## Dependencies and open work

- Migration ID: 20261028000001_transcription_profiles. Update server and
  worker together.
- No new crates in `Cargo.lock`. The worker now uses `tokio-tungstenite`
  (rustls, webpki roots), `rustls`, `webpki-roots`, `futures-util`, `base64`.
- Overlap: PR 12 (media worker transcript times) and PR 13 (web design) can
  conflict in `api.rs`/`main.rs` and on the settings page.
- Open: The realtime WebSocket does not use `HTTPS_PROXY`.
- Open: Verify with a real OpenAI key (`gpt-transcribe`,
  `gpt-live-transcribe`).

## Next action

The release chat reviews the PR, integrates it and runs the combined checks.

## References

- ../adr/0024-transcription-engines-per-purpose.md
- ../adr/0019-transkription-in-zwei-durchgaengen-und-ki-api.md
- ../adr/0022-active-call-identities-and-live-previews.md
- 20261012-codex-active-call-overview.md
