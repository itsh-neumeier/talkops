# Backup targets

## Task state

- Task ID: 20261015-claude-backup-targets
- Date: 2026-10-15
- Tool: Claude Code
- Status: Part 1 completed (PR 1). Part 2 (deployment capture) open.
- Branch: claude/lucid-volta-54t5lq (part 1)
- Base commit: 702507b39fb1cf80db4ce5a0f9dd3d339b9da68c (main, release 1.7.13)
- Related commits or PR: Pending

## Goal

Add backup targets: SMB, NFS, local, S3, FTP, WebDAV and SFTP, with a folder
browser where the protocol permits it. Restore from a target, also during
first setup. Capture Docker Compose deployment files automatically when
possible. The user approved the proposal with changes: a flexible schedule
and a smart retention per target, and two PRs.

## Result

Part 1 is implemented, tested and documented:

- Targets with encrypted secrets, schedule, smart retention, bandwidth limit
  and per-target recordings setting. rclone transfers.
- Folder browser, folder creation, connection test, SFTP host key pinning.
- Archive list, download to the server, delete. Restore assistant source
  "From a backup target". `talkops fetch`, `recover.sh fetch`,
  `recover.sh restore --backup NAME`.
- UI in settings > system and in the setup restore assistant (en/de).
- Docker image with pinned rclone, CI installs the same rclone.
- `docker-compose.targets.yml` example for an NFS volume.

## Changes

- `migrations/20261102000001_backup_targets.sql`: table `backup_targets`.
- `crates/talkops-core/src/backup_targets.rs`: types, validation, schedule,
  retention, queries.
- `crates/talkops-api/src/rclone.rs`: rclone process wrapper.
- `crates/talkops-api/src/backup_targets.rs`: runs, scheduler tick, prune, fetch.
- `crates/talkops-api/src/routes/backup_targets.rs`: admin API.
- `crates/talkops-api/src/routes/recovery.rs`: setup browse/inspect from a
  target; shared token check; explicit rollback of the setup lock.
- `crates/talkops-api/src/backup.rs`: `create_in`, `stamp`, scheduler calls targets.
- `crates/talkops-api/src/main.rs`, `config.rs`: `fetch` command,
  `TALKOPS_RCLONE`, `TALKOPS_BACKUP_TARGET_ROOTS`.
- `scripts/recover.sh`, `tests/recovery/host.sh`: fetch and `--backup`.
- `web/src/lib/backupTargets.ts`, components `BackupTargets`,
  `TargetConnectionFields`, `FolderBrowser`, `RestoreAssistant`, i18n en/de.
- `docker/talkops/Dockerfile`, `.github/workflows/ci.yml`,
  `docker-compose.targets.yml`, `.env.example`.
- Docs: ADR 0029, `docs/en/backup.md`, `docs/de/datensicherung.md`,
  `docs/architecture.md`, `docs/plans/backup-targets.md`.

## Decisions

- rclone v1.75.2 as the only transfer tool; secrets only in its environment.
- NFS and kernel SMB mounts through host Docker volumes (no capabilities).
- Remote targets require the backup password (encrypted archives).
- Retention is time-window based (Synology-like), plus keep_last >= 1.
  Only TalkOps archive names are deleted.
- A due target reuses the daily local archive of the same minute; other runs
  use one staging archive in the backups volume.
- SFTP: trust on first use with fingerprint display; saved targets need keys.
- Setup token rate limit counts only wrong tokens (browser needs many calls).
- No Docker socket (ADR 0021 stays).

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Passed: `cargo test --workspace --locked` with local PostgreSQL 16, pg_dump 16
  and rclone 1.75.2 (see the PR for counts).
- Passed: rclone round trips against `rclone serve` WebDAV, FTP, SFTP and S3.
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`.
- Passed: browser checks with Playwright against a local server: setup
  restore from a local target, target editor with WebDAV, browse, create
  folder, test, save, run now, archive list; no sideways scroll at 360/768 px.
- Passed: `bash tests/recovery/host.sh`; Compose config with
  `docker-compose.targets.yml`.
- Not run: Docker image build (no Docker daemon). The rclone stage download
  and checksums were checked in a shell for amd64 and arm64. CI builds images.
- Not run: SMB against a real server (no SMB test server); covered by unit
  tests of the rclone environment. E2E SIPp tests (unchanged areas).
- STE review: Clear English review. Full standard unavailable; review pending.

## Dependencies and open work

- Migration ID: 20261102000001. Renamed from 20261101000001 because open
  PR #30 (roles_permissions) uses 20261101000001 and PR #31 uses 20261101000002.
- Part 2 (separate PR): default read-only bind of the Compose directory,
  `TALKOPS_DEPLOYMENT_FILES=auto` discovery, optional Portainer API
  connection that reads the stack file and environment.
- Not included: e-mail alerts for failed targets.

## Next action

Open PR 1. Then implement part 2 on its own branch from the target branch.

## References

- [Proposal](../plans/backup-targets.md)
- [ADR 0029](../adr/0029-backup-targets-with-rclone.md)
- [Guided backup and restore](20261012-codex-guided-backup-restore.md)
