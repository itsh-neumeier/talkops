# Audit log filters, export and retention

## Task state

- Task ID: 20261016-claude-audit-export
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed
- Branch: claude/audit-export (stacked on claude/storage-targets)
- Base commit: 138335c (head of claude/storage-targets, PR #41)
- Related commits or PR: https://github.com/itsh-neumeier/talkops/pull/43 (stacked on #41)

## Goal

PR 7b. Make the audit log usable for audits: filters, sort, paging and
downloads (CSV/JSON). Add an automatic, incremental export of new entries
to a storage target (ADR 0036) with SHA-256 checksums, and a retention
period for the database.

## Result

- Migration `20261104000002_audit_export.sql`: `audit_settings` (one row per
  tenant, FK to `storage_targets`, `ON DELETE NO ACTION`), `audit_exports`
  (files with SHA-256), index `audit_log (tenant_id, id)`.
- Core: `audit.rs` (filtered search, count, export up to 100,000 rows, CSV
  with formula guard, JSON, JSON Lines, prune, facets, incremental `after`
  with a 10 s settle delay); `audit_export.rs` (settings, files, file names).
- API: `GET /api/v1/audit` (filters, `X-Total-Count`), `/audit/facets`,
  `/audit/export?format=csv|json` (recorded as an audit entry),
  `GET/PUT /audit/settings`, `POST /audit/export/run`. New permission
  `audit.manage` (sensitive, granted to no role by the migration).
  `audit.manage` can read the storage target list.
- Background task `talkops_api::audit_export::spawn`: export every minute
  when due; retention hourly. With the export on, retention deletes only
  exported entries.
- Storage purpose `audit_export` in `USAGE_SQL`; a storage target used by
  the export cannot be deleted.
- UI: `/audit` with filters, sortable columns, pages of 50, CSV/JSON links;
  tab "Retention and export" (`AuditExportSettings.svelte`) for
  `audit.manage`.
- ADR 0037. User docs `docs/en/audit-log.md`, `docs/de/audit-log.md`; role
  tables updated (also `storage.manage`, missing from PR 7a).

## Decisions

- Incremental export by ID with `last_exported_id`, moved in the same
  transaction that records the file. A failed run repeats entries; it never
  loses them. Entries younger than 10 s wait (ID order is not commit order).
- One new file per run, never appended; `.sha256` next to it in
  `sha256sum` format; checksum also stored in `audit_exports`.
- Export files are not encrypted (no secrets in audit details); the backup
  password is not needed. Docs recommend encrypted transport.
- File retention reuses `Retention::keep` of backup jobs and deletes only
  `talkops-audit-*` files and their checksum files.
- Download limit 100,000 rows (422 when more match).

## Checks

- Passed: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` (PostgreSQL 16, pg_dump 16,
  rclone 1.75.2): 36 binaries, 334 tests, incl. `tests/audit.rs` (filters,
  paging, downloads, permissions, retention, export to a local target with
  checksum and file retention) and core unit and DB tests.
- Passed: `npm run lint`, `npm run check`, `npm test` (16), `npm run build`.
- Passed: Playwright `audit.mjs` (new, added to CI), `settings-system.mjs`,
  `layout.mjs`, `roles.mjs`.
- Not run: Docker image build, E2E SIPp.
- STE review: Clear English review. Full standard unavailable; review pending.

## Dependencies and open work

- Stacked on PR #41 (storage targets), which is stacked on PR #37.
- Migration ID 20261104000002, ADR 0037.
- Possible follow-up: syslog/SIEM forwarding as a second export purpose.

## Next action

The release manager merges #37, #41, then this PR.

## References

- [ADR 0037](../adr/0037-audit-export-and-retention.md)
- [ADR 0036](../adr/0036-storage-targets.md)
- [Storage targets](20261016-claude-storage-targets.md)
