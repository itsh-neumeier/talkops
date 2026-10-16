# 0036 - Storage targets as a shared area

- Status: Accepted
- Date: 2026-10-16
- Supersedes: [ADR 0031](0031-backup-targets-with-rclone.md), the storage
  parts: the table `backup_targets` as the store of connections and the
  connection API below `/api/v1/backup-targets`.
- Keeps: ADR 0031 rclone transfers, secret handling, encryption rule,
  schedule, retention and restore.

## Context

ADR 0031 stores the connection of a backup target (protocol, settings,
secrets, folder, insecure-transport switch, bandwidth limit) together with
the schedule and the retention in one row. Other features also need to write
files to a NAS, S3 or a cloud: the audit export, and later a recording
archive. Each feature must not keep its own copy of the connections and the
secrets. The administrator wants one place to add and test storage.

## Options

1. Keep the connections in `backup_targets`. Each new feature adds its own
   connection columns.
2. One table `storage_targets` for connections. Features refer to a storage
   target by a foreign key and keep their own job settings.
3. One generic job table for all features.

## Decision

Option 2.

- Table `storage_targets` (migration `20261104000001`): name, protocol,
  settings, encrypted secret, base folder, insecure-transport switch,
  bandwidth limit and the result of the last saved connection test.
- The migration copies the connection of each backup target into a storage
  target with the same ID and name. `backup_targets` gets
  `storage_target_id` (NOT NULL) and `folder`, and loses the moved columns.
  No data is lost.
- The foreign key `(tenant_id, storage_target_id)` refers to
  `storage_targets (tenant_id, id)`. A job cannot refer to a storage target
  of another tenant. The rule is `ON DELETE NO ACTION`: like `RESTRICT`, the
  database rejects the deletion of a used storage target, but a tenant
  deletion can cascade to both tables in one statement. The API checks the
  users first and answers `409` with their names.
- A backup job writes to the base folder plus its optional sub-folder.
  `UNIQUE (storage_target_id, folder)` prevents two jobs in one folder,
  because retention would delete the archives of the other job.
- Purposes: `StoragePurpose` in `talkops_core::storage_targets` names the
  features that use storage targets. Each feature adds a value and one
  `UNION ALL` part to the usage query. The API returns the users of each
  target as `used_by`. A feature writes into its own sub-folder, so file
  names of different features do not mix.
- API: `/api/v1/storage-targets` (list, create, update, delete, draft test,
  saved test, folder browser, folder creation). The folder browser and the
  test moved from `/api/v1/backup-targets/*`. Backup jobs stay at
  `/api/v1/backup-targets` and take `storage_target_id` and `folder`
  instead of the connection fields. The job response still contains the
  connection fields of its storage target (read-only).
- Permission `storage.manage` (sensitive) manages storage targets. The
  migration gives it to every role with `backups.manage`, so no access
  changes. A user with `backups.manage` can read the list without secrets to
  select a target.
- Encryption: remote backup jobs still need the backup password (ADR 0031).
  A storage target alone does not need it. A change of a storage target that
  an enabled backup job uses checks the rule again.
- The setup restore assistant still takes a full connection without a saved
  storage target.

## Consequences

- Positive: one connection serves several jobs and features. The
  administrator tests and changes it in one place.
- Positive: new features plug in with a foreign key and a purpose value.
- Negative: API change for clients of `POST`/`PUT /api/v1/backup-targets`
  and the moved browse, folder and test endpoints. The web UI is the only
  known client.
- Negative: a change of a storage target affects all its users. The editor
  shows the number of users.
- Storage targets still need the backup configuration of the server (rclone
  and the local roots).

Language review: Clear English review only. The official ASD-STE100 standard
was not available. Full language review is pending.
