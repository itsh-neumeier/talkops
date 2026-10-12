# Guided backup and restore

## Task state

- Task ID: 20261012-codex-guided-backup-restore
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: Completed. Code verification passed; final documentation transfer.
- Branch: codex/guided-backup-restore
- Base commit: 2fbe3195517b509d2afd3d57247c05a98437f064
- Feature commit: ef39d2d
- Related PR: https://github.com/itsh-neumeier/talkops/pull/4
- Application head checked: 90bebe0. Use the PR for final head and readiness.

## Goal

Add guided recovery to first setup. Include deployment files and application
secrets in encrypted recovery backups. The user approved the proposal before
implementation. Commit, push and PR creation are authorized.

## Result

Implemented the setup assistant, encrypted backups, deployment capture and
Portainer import, standalone configuration export and an offline host tool.
Existing format-1 archives remain readable. Restored sessions are cleared.
No release metadata, tag, merge or publication was changed.

The worktree name is guided-backup-restore. Use git worktree list for its path.
Merged reviewed target commit 4503fb3 (release 1.7.6) into the feature branch.
Preserved both features when resolving translation conflicts. Other worktrees
were preserved. App registration failed, but the Git worktree is usable.

## Changes

- Backup and recovery modules: age envelope, file inventory, private staging,
  bounds, original-key check, writer exclusion and restore phase records.
- CLI and scripts/recover.sh: inspect, export and confirmed offline restore.
- Setup and backup UI: restore choice, inspection, configuration download,
  missing-file status and encrypted Portainer import. English/German strings.
- Compose backup override, environment examples and CI recovery checks.
- English/German user guides, ADR 0021 and the accepted proposal.
- Fixed initial auth/me navigation so unauthenticated setup stays visible.
- Simplified existing identity URL validation to pass current clippy.

## Decisions

- The user approved the separate backup password and offline host step.
- No Docker socket or privileged web restore. The host stops all three writers.
- Imported deployment secrets use the existing application key at rest.
- No schema migration. Dependencies: age 0.11.5, fs2 0.4.3, tempfile 3.27.0.
- ADR 0021 avoids ADR 0020 in parallel PR #3.
- Release versions, tags, merges and publication remain with the release chat.

## Checks

- Passed: Archive tests for encryption, export, corruption, limits and raw paths.
- Passed: Database advisory lock test with isolated PostgreSQL 17.
- Passed: Web lint, Svelte/TypeScript check and production build.
- Passed: Browser setup flow using headless Edge and API fixtures.
- Passed: Compose production, development and backup-override validation.
- Passed: Host-script export, confirmation, options and failure-state tests.
- Passed: Final API/core clippy with warnings denied on target 1.7.6.
- Passed: Final encrypted backup/restore and setup/import integration tests (4).
  Includes wrong-key rejection without data changes and non-admin import denial.
- The wider local API/core run stopped on three phone template CRLF assertions.
  Normalized embedded template line endings without Git content changes.
- Passed: Linux CI run 38261126875 at application head 90bebe0. Includes
  full Rust workspace tests, clippy, fmt, web/browser tests, Compose and both
  server/worker image build and smoke tests.
- Passed: Linux SIPp and browser-softphone E2E run 38261126999 at 90bebe0.
- Passed: Documentation links and whitespace (13 changed Markdown files).
- Earlier failures: Missing GNU compiler, full system disk and CRLF lint noise.
  Used MSVC, isolated test storage and normalized worktree text line endings.
- Not run locally: Linux image builds, SIPp and worker Unix-only tests.
  Linux CI covers the images and Unix-only tests.
- CodeRabbit CLI is unavailable. Manual diff review is used.
- STE review: Clear English review. Full standard unavailable; pending.

## Dependencies and open work

- No dependency on PR #3. The reviewed PR #2 release is already included. Integration overlaps include worker/main.rs,
  web/api.ts, language catalogs, CI and the ADR index. Preserve both behaviors.
- Database migration IDs: None.
- Implementation is committed and pushed. PR #4 is attached. The final transfer
  changes documentation only. Use the PR for current verification and readiness.
- The isolated PostgreSQL test service and active task preview were stopped.

## Next action

The release chat selects scope and version, integrates the PR, runs combined
checks and controls target merges, tags and publication. No feature work remains.

## References

- [Accepted proposal](../plans/guided-backup-restore.md)
- [ADR 0021](../adr/0021-guided-recovery-and-encrypted-deployment-backups.md)
- [Backup guide](../en/backup.md)
- [Release workflow](../release-workflow.md)
