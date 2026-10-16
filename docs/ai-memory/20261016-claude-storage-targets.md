# Storage targets as a shared area

## Task state

- Task ID: 20261016-claude-storage-targets
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed
- Branch: claude/storage-targets (stacked on claude/settings-system-structure)
- Base commit: 0ea197d (head of claude/settings-system-structure, PR #37)
- Related commits or PR: Pending

## Goal

Make storage its own area (PR 7a). Before, `backup_targets` mixed the
connection (protocol, settings, secret, folder, insecure switch, bandwidth
limit) with the backup schedule and retention. Move the connections into
storage targets that several features can use. Backup jobs only select a
storage target. The audit export (PR 7b) uses the same storage targets.

## Result

- Migration `20261104000001_storage_targets.sql`: table `storage_targets`;
  each backup target becomes a storage target with the same ID and name;
  `backup_targets` gets `storage_target_id` (NOT NULL, tenant-safe FK,
  `ON DELETE NO ACTION`) and `folder`, `UNIQUE (storage_target_id, folder)`,
  and loses the moved columns. Roles with `backups.manage` get
  `storage.manage`.
- Core: `talkops_core::storage_targets` (connection types moved from
  `backup_targets`, CRUD, usage, test state, purposes). `backup_targets`
  re-exports the connection types and keeps schedule and retention.
- API: `/api/v1/storage-targets` list, create, update, delete (409 when in
  use), `POST /test` (draft), `POST /{id}/test` (saved, recorded),
  `POST /browse`, `POST /folders`. Backup jobs take `storage_target_id` and
  `folder`. Permission `storage.manage`.
- UI: new settings section "Storage"/"Speicher" (`?section=storage`) with
  list, used-by, test state, editor with folder browser. The Backup section
  shows "Backup jobs" with a storage target select and a sub-folder.
- ADR 0036 supersedes the storage parts of ADR 0031.

## Changes

- `migrations/20261104000001_storage_targets.sql`
- `crates/talkops-core/src/storage_targets.rs` (new), `backup_targets.rs`,
  `roles.rs` (`StorageManage`), `lib.rs`.
- `crates/talkops-api/src/routes/storage_targets.rs` (new),
  `routes/backup_targets.rs`, `routes/recovery.rs`, `routes/mod.rs`, `lib.rs`.
- `crates/talkops-api/tests/backup_targets.rs`: new API flow, used-target
  delete, saved test, folder conflict, permissions.
- `web/src/lib/components/StorageTargets.svelte` (new),
  `BackupTargets.svelte`, `SettingsNav.svelte`, settings page,
  `web/src/lib/backupTargets.ts`, `api.ts`, i18n en/de.
- `tests/browser/settings-system.mjs`: storage section and job editor.
- Docs: ADR 0036, ADR index, ADR 0031 status, `docs/architecture.md`,
  `docs/en/backup.md`, `docs/de/datensicherung.md`.

## Decisions

- Same ID and name for migrated storage targets: the jobs keep their IDs,
  history and archives; old links stay valid.
- `ON DELETE NO ACTION` instead of `RESTRICT`: same protection for a direct
  delete, but a tenant deletion can still cascade. The API answers 409 with
  the user names first.
- A sub-folder per job and `UNIQUE (storage_target_id, folder)`: retention
  of one job would otherwise delete the archives of another job.
- Purpose concept: `StoragePurpose` enum plus one `UNION ALL` part per
  feature in `USAGE_SQL`. Features write into their own sub-folder.
- New permission `storage.manage` (sensitive). `backups.manage` can still
  read the storage list (without secrets) to select a target.
- API compatibility: job responses keep the connection fields read-only.
  `POST`/`PUT /backup-targets` now need `storage_target_id`; browse, folders
  and draft test moved to `/storage-targets/*`. The UI is the only known
  client, so no aliases.
- Remote storage targets do not need the backup password. Enabled backup
  jobs on remote targets still need it (ADR 0031).

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with PostgreSQL 16,
  pg_dump 16 and rclone 1.75.2: 35 test binaries, 327 tests. One earlier
  run had a single failure in `roles::user_managers_cannot_give_or_take_more_rights`
  that passed on rerun and in the full rerun (timing, unrelated code).
- Passed: migration test with rows from before the migration
  (`backup_targets::tests::migration_moves_connections_to_storage_targets`).
- Passed: `npm run lint`, `npm run check`, `npm test` (16), `npm run build`.
- Passed: Playwright `settings-system.mjs`, `layout.mjs`, `roles.mjs`.
- Not run: Docker image build, E2E SIPp, `tests/recovery/browser.mjs`
  (restore assistant unchanged).
- Note: a private `CARGO_TARGET_DIR` was used. The shared target directory
  mixed artifacts of other worktrees for workspace crates.
- STE review: Clear English review. Full standard unavailable; review pending.

## Dependencies and open work

- Stacked on PR #37 (settings structure). Merge after #37.
- Migration ID 20261104000001 (reserved). ADR 0036 (reserved).
- PR 7b (audit export, branch claude/audit-export) adds
  `StoragePurpose::AuditExport`.

## Next action

The release manager merges PR #37, then this PR.

## References

- [ADR 0036](../adr/0036-storage-targets.md)
- [ADR 0031](../adr/0031-backup-targets-with-rclone.md)
- [Backup targets](20261015-claude-backup-targets.md)
- [Settings structure](20261016-claude-settings-system-structure.md)
