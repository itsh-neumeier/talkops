# Coral model upload in the TalkOps UI

## Task state

- Date: 2026-10-28
- Tool: Claude Code
- Status: Implemented and checked locally; PR open, CI pending
- Branch: claude/coral-model-upload (worktree `.worktrees/claude-coral-model-upload`)
- PR: https://github.com/itsh-neumeier/talkops/pull/91
- Base commit: 977528a (origin/main, release 1.8.14)
- Related: `20261028-claude-coral-german-model-evaluation.md`,
  `20261028-codex-coral-missing-model.md`, ADR 0046, new ADR 0053

## Goal

The project cannot redistribute the NVIDIA NGC German model or weights
derived from it. Let an administrator install compiled Coral models that
they may use through the TalkOps UI. A separate project download of NGC
weights is still a redistribution and was not done.

## Decisions

- `coral-stt` stores and checks models, not the API server. It owns the
  model format and the startup loader. See ADR 0053.
- Staging directory, check with `models::check_slot` (startup code), TFLite
  `TFL3` identifier and `edgetpu-custom-op` marker, then swap. One backup
  level in `.backup/<slot>`.
- No hot reload. `POST /v1/admin/restart` stops the process; the compose
  restart policy starts it. Open connections close after at most 10 s.
- License and source statements are required and stored in `install.json`.
  TalkOps does not check them. The UI requires a confirmation checkbox.
- API: `/api/v1/coral/{id}/models[/{slot}[/restore]]`, `/api/v1/coral/{id}/restart`.
  Permission `transcription.settings`. Audit entries hold slot, digest,
  license and source. The API forwards the multipart body unchanged.
- `docker-compose.yml`: `models` volume is writable in `coral-stt`;
  `TALKOPS_CORAL_MODEL_ADMIN` (default true) disables the endpoints.

## Changes

- `crates/talkops-coral-stt/src/install.rs` (new), `http.rs`, `main.rs`,
  `models.rs`, `lib.rs`, `tests/service.rs`.
- `crates/talkops-api/src/routes/coral_models.rs` (new), `routes/mod.rs`, `lib.rs`.
- `web/src/lib/components/CoralModels.svelte` (new),
  `TranscriptionSettings.svelte`, `api.ts`, `i18n/en.ts`, `i18n/de.ts`.
- `docker-compose.yml`, `.env.example`, `docs/adr/0053-coral-model-upload.md`,
  `docs/adr/README.md`, `docs/en/recording.md`, `docs/de/aufzeichnung.md`.

## Checks

Rust ran in `rust:1-trixie` (no local cargo on this Windows host).

- Passed: `cargo fmt --all -- --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings` (before the
  last service test was added); `cargo clippy -p talkops-coral-stt` after it.
- Passed: `cargo test -p talkops-coral-stt` (24 unit, 8 service tests, new:
  install/backup/restore/reject and HTTP admin flow with key and restart).
- Passed: `cargo test -p talkops-api --test http` with Postgres 17 (8 tests,
  includes the OpenAPI document) and the `coral_models` unit test.
- Passed in `node:22-trixie`: Prettier check of changed web files,
  `npm run check` (0 errors), `npm test` (91 tests).
- `npm run lint` reports 235 files on this Windows checkout because of CRLF
  line endings (`core.autocrlf=true`); the changed files pass.
- Not run: full `cargo test --workspace`, SIPp end-to-end test, UI in a
  browser, real container restart, write access on a production volume,
  upload of a real compiled model to a physical TPU.
- STE review: full standard unavailable; not reviewed.

## Model library (follow-up request)

The user asked for a store of several models with a selection. Changed
before merge in the same PR:

- `install.rs` is now a library: `models/coral/library/<id>/` (id = first
  12 hex digits of the Edge TPU file digest), one selected model per slot,
  copied into the slot directory. Limits 10 models / 512 MiB. Selected
  models cannot be deleted. Hand-copied slot models are adopted into the
  library (empty license) before a selection replaces them.
- The single `.backup` level and the restore endpoint were removed.
- Endpoints: coral-stt `/v1/admin/models`, `/v1/admin/library[/{id}]`,
  `/v1/admin/slots/{slot}` (PUT/DELETE), `/v1/admin/restart`; API
  `/api/v1/coral/{id}/models`, `/library[/{model}]`, `/slots/{slot}`, `/restart`.
- UI: slot selectors, library table with usage, upload with optional
  immediate selection. Only one model per slot is active (TPU memory).
- Checks after the change: fmt and workspace clippy passed; coral-stt
  25 unit and 8 service tests passed; API HTTP tests (8) with Postgres and
  the `coral_models` unit test passed; svelte-check 0 errors; web tests 91
  passed; Prettier passed for changed files.

## Open work

- CI result and release-manager handoff.
- Check on a Docker host: upload a real package, restart, model active,
  restore. Existing installations need the new compose file (writable volume).
