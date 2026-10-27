# Coral UI, production trial and support diagnostics

## Task state

- Task ID: 20261026-codex-coral-production-readiness
- Date: 2026-10-26
- Tool: Codex
- Status: Implemented and locally validated; user-owned production trial pending
- Branch: codex/coral-stable-preview
- Base commit: 51e7df588ab028a435e79685ff43baeae5a52898
- Related PRs: #81 and #82

## Goal and current direction

The user wants the Coral feature ready, including UI control. The user will
perform the real test on the production host. The user also requests full
debug output that can be handed back for diagnosis. Do not change that host
or claim the unresolved quality gate passed.

## Current state

- Fetched main at f8bdfb3 (1.8.11).
- Created `codex/coral-base-readiness` from PR #81 head and merged main
  without conflict. Initial base publish was 0063ce2; combined concurrent
  test corrections are published at 5a1a531.
- Merged that updated base into `codex/coral-stable-preview`.
- Fixed the Coral package lock version to match inherited 1.8.11 metadata.
- Added pending migration `20261112000002_coral_stable_preview`.
- Added profile field, scoped validation, audit entry and worker propagation
  for a UI complete-word preview checkbox. The service setting becomes a
  default; each tenant profile sets the session mode explicitly.
- Added a bounded, tenant-scoped Coral trace buffer with explicit start/stop
  and report endpoints. Audio is never retained. Text is optional. The API
  requires diagnostics permission, and all-live-text permission for content.
- Added the System Diagnostics Coral card with capture and report download.
- Added technical live-preview state export, excluding call identity/files
  and transcript contents.
- Added SQL/API/worker, trace and browser tests. Local validation and the
  test/rollback documentation are complete.

## Result and remaining handoff

The implementation is complete and locally tested. The trial image workflow
only publishes `coral-test-<12-character-source-commit>` for three amd64
images on the owned feature branch, after the main CI checks pass. Stable
tags and production services are unchanged. The trial override keeps
FreeSWITCH at 1.8.11. The guide covers the port conflict at 8095, PCIe GID 0,
model installation, additive migrations and rollback through a matching
pre-trial backup. It also explains how to stop using Coral in the UI.

The report includes app/service/worker build revisions, model SHA-256,
model shapes, TPU state, queue, timing events and current fallback state.
The capture is bounded to 5,000 events/2 MiB and 1–30 minutes. Events expire
after one hour. Text capture/export needs explicit consent and all-live-text
permission. The technical report does not retain audio or credentials.
It is diagnostic data, not a WAV/TSV training export.
The download also works when Coral is unreachable, with application state
and `coral_service_error`. A regression test covers that failure case.

Local model package, not committed or publicly uploaded:
`build/coral-work/coral-production-trial/coral-de-candidate-20261025.tar.gz`.
Archive SHA-256: `d8f2924d6c826bd0d449b981084315195c18a2cd74f59524e97a186ff1133423`.
Compiled model SHA-256: `694931801d196734efd6b5ed5deeba2b0152462656325a611e8f33700095fa14`.
It contains the earlier candidate and metadata only, without audio or text.
Docker context now excludes local build data and test outputs.

Update PRs after checks, with the user-owned production trial still pending.
No merge, version bump, stable image publication or production deployment
has been requested. Do not mark quality/readability approved without data.

## Checks

- Passed: workspace Clippy with all targets and `-D warnings`, locked Cargo.
- Passed: 46 targeted Rust tests: 22 Coral units, seven service tests, eight
  core transcription SQL tests, four API integration tests, one export
  redaction unit, three worker Coral tests and one worker session test.
  Real-model TFLite test remains ignored; the user owns the real trial.
- Passed: frontend lint with Windows line-ending adaptation, type check
  with no warnings, all 83 web tests and production build.
- Passed: browser checks for Coral settings, capture/download and System
  settings; desktop/mobile screenshots inspected. No horizontal overflow.
- Passed: all 13 Wiki tests, 75 pages/686 links and trial Compose config.
- Resolved: old-schema migration fixtures now run the remaining upgrade
  path before using the current profile reader. API route registration now
  adds the two GET paths separately. The frontend build output is not a
  mounted directory, which avoids adapter EBUSY on removal.
- Resolved: initial browser DNS hostname failed Vite's host check. The
  checks passed on localhost in the UI container's network namespace.
- Resolved: CI 40461728658 failed the older overload timing assumption.
  The gated test now fills the queue causally and bounds the response wait.
  Preserve Claude's concurrent device-state wait and successful recovery
  assertion. Merged base passed all five service tests; merged feature
  passed all seven, both with two CPUs. Runtime behavior is unchanged.
- Not run locally: full workspace/SIP E2E and full production image builds.
  GitHub CI covers these. The physical Rust-service quality gate remains
  pending and is not replaced by mock tests or the prior Python results.
- USB remains attached at 15-1, runtime 18d1:9302. An unprivileged `usbipd
  list` showed it, then failed to query the Windows service manager. No USB
  mutation was performed.
- STE review: full standard unavailable; full language review pending.

## Next action

The base synchronization is published. The feature handoff is in #82;
GitHub checks and workflow summaries are the authority for current CI and
trial tags. Verify all three trial image jobs before deployment. The user
owns the production trial. Do not deploy or mark readability/quality
approved. Use `docs/research/coral-production-trial.md`.
