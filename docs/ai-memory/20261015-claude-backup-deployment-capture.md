# Automatic deployment capture and Portainer API

## Task state

- Task ID: 20261015-claude-backup-deployment-capture
- Date: 2026-10-15
- Tool: Claude Code
- Status: Completed (PR 2 of the backup target task).
- Branch: claude/backup-deployment-capture
- Base commit: 702507b39fb1cf80db4ce5a0f9dd3d339b9da68c (main, release 1.7.13)
- Related commits or PR: https://github.com/itsh-neumeier/talkops/pull/34 (part 2). Part 1: https://github.com/itsh-neumeier/talkops/pull/33

## Goal

Part 2 of the backup target task (see the summary
`20261015-claude-backup-targets.md` on the part 1 branch and
`docs/plans/backup-targets.md` there, section 6): capture the Docker Compose
deployment files automatically when possible. The user approved a default
read-only bind of the Compose directory and the Portainer API connection.

## Result

- `TALKOPS_DEPLOYMENT_FILES=auto` (new default) discovers `.env`, `*.env`,
  `docker-compose*`/`compose*` YAML files; reports left-out files.
- `docker-compose.yml` mounts the Compose directory read-only by default.
  `docker-compose.backup.yml` only adds the password file.
- An empty mounted directory (Portainer) does not count as mounted.
- Portainer connection: URL, token (encrypted), stack; certificate pinning
  for self-signed certificates; stack list; read before each backup; status
  and last error in the UI. Manual import stays as fallback.

## Changes

- `migrations/20261102000002_portainer_connection.sql`
- `crates/talkops-core/src/portainer.rs`: connection storage and validation.
- `crates/talkops-api/src/portainer.rs`: client, pinning verifier, sync, refresh.
- `crates/talkops-api/src/recovery.rs`: discovery, `mounted_active`,
  `store_import`, `origin`, `excluded`.
- `crates/talkops-api/src/routes/backups.rs`: Portainer routes, overview fields.
- `crates/talkops-api/src/backup.rs`: refresh from Portainer before a backup.
- `crates/talkops-api/src/config.rs`: default `auto`.
- `docker-compose.yml`, `docker-compose.backup.yml`, `.env.example`.
- `web/src/lib/components/BackupSettings.svelte`, i18n en/de.
- Docs: ADR 0032 (partly supersedes ADR 0021), backup guides, Portainer guides.

## Decisions

- No Docker socket. Portainer through its API with an access token.
- `http://` only to 127.0.0.1; self-signed certificates by pinned SHA-256.
- `TALKOPS_BACKUP_PASSWORD` is never written into the captured `.env`.
- ADR 0032 and migration 20261102000002: 0029/0030 and 20261101000001/2 are
  used by PRs #30/#31; part 1 (#33) uses ADR 0031 and 20261102000001.

## Checks

- Passed: `cargo fmt --all --check`, clippy with `-D warnings`, `cargo test --workspace --locked`.
- Passed: tests with a fake Portainer server, discovery and precedence, and
  certificate pinning against `openssl s_server` with a self-signed certificate.
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`.
- Passed: Playwright check of the Portainer form against a local server and a
  fake Portainer; no sideways scroll at 360 px.
- Passed: Compose config for production, development and backup override.
- Not run: real Portainer instance; Docker image build (no Docker daemon).
- STE review: Clear English review. Full standard unavailable; review pending.

## Dependencies and open work

- Integration with PR #33: both PRs change `backup.rs` and the backup guides.
  After the merge of both, `crate::portainer::refresh` must run for every
  archive: in #33, `create()` delegates to `create_in()`. Move the refresh
  call to the start of `create_in()` so archives for backup targets also
  get the current Portainer stack.
- Not included: additional files of Git-based Portainer stacks.

## Next action

The release chat integrates PR #33 and this PR.

## References

- [ADR 0032](../adr/0032-automatic-deployment-capture.md)
- [ADR 0021](../adr/0021-guided-recovery-and-encrypted-deployment-backups.md)
- [Guided backup and restore](20261012-codex-guided-backup-restore.md)
