# 0037 - Audit log filters, export and retention

- Status: Accepted
- Date: 2026-10-16
- Uses: [ADR 0036](0036-storage-targets.md) (storage targets).

## Context

Before this decision, the audit log showed the newest 200 entries without
filters. Administrators must find changes of one user, one object or one
period. Many organizations must keep audit records outside the system for a
long time, and must be able to show that a file was not changed. The table
`audit_log` grows without limit.

## Options

1. Filters only. Administrators copy the data with database tools.
2. Filters, a download, and a periodic export of new entries to a storage
   target with checksums. A retention period for the database.
3. Send entries to a syslog or SIEM server.

## Decision

Option 2. Option 3 can follow as a second purpose of the same settings.

- `GET /api/v1/audit` takes filters (period, user, action, object type,
  text), sort and paging. `X-Total-Count` has the number of matches. The
  text search excludes `LIKE` wildcards.
- `GET /api/v1/audit/export?format=csv|json` returns all matches (at most
  100,000) as a file. The download writes its own audit entry. CSV cells
  that start with `=`, `+`, `-` or `@` get a leading `'` (CSV injection).
- Migration `20261104000002`: table `audit_settings` (one row per tenant)
  with the retention period, the storage target (tenant-safe foreign key,
  `ON DELETE NO ACTION`), sub-folder, format, schedule, file retention and
  `last_exported_id`. Table `audit_exports` records each file with its
  SHA-256 checksum.
- The export is incremental: each run writes the entries with an ID above
  `last_exported_id` into a new file (`talkops-audit-<time>-<first>-<last>`,
  JSON Lines or CSV, at most 50,000 entries), uploads it with a `.sha256`
  file in `sha256sum` format, then records the file and moves
  `last_exported_id` in one transaction. Files are never changed after the
  upload. An interrupted run writes the same entries again on the next run;
  no entry is lost.
- The retention runs hourly. While the export is on, it deletes only
  entries with an ID up to `last_exported_id`. A broken export therefore
  stops the deletion instead of losing entries.
- The export uses the storage target purpose `audit_export`. It writes into
  its own sub-folder and its file retention deletes only its own file
  names.
- Permission `audit.manage` (sensitive) changes the settings and starts a
  run. `audit.view` reads and downloads. The migration grants
  `audit.manage` to no role; administrators have it.
- The export does not encrypt the files. Audit details contain no secrets.
  The backup password is not needed.

## Consequences

- Positive: complete, verifiable copies outside TalkOps; a bounded table.
- Positive: the API and the files are stable inputs for other tools.
- Negative: the checksum file lies next to the export file. It detects
  damage, but an attacker with write access to the target can change both.
  The checksums in the TalkOps database give a second copy.
- IDs are assigned at insert time, not at commit. An entry of a transaction
  that commits late can have a lower ID than an exported one. The export
  therefore takes only entries older than 10 seconds. Audit inserts are
  short, so this delay is sufficient.

Language review: Clear English review only. The official ASD-STE100 standard
was not available. Full language review is pending.
