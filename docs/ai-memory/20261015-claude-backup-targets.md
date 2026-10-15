# Backup targets

## Task state

- Task ID: 20261015-claude-backup-targets
- Date: 2026-10-15
- Tool: Claude Code
- Status: In progress. Proposal waits for user review.
- Branch: claude/lucid-volta-54t5lq
- Base commit: 702507b39fb1cf80db4ce5a0f9dd3d339b9da68c (main, release 1.7.13)
- Related commits or PR: Pending

## Goal

Add backup targets: SMB, NFS, local, S3, FTP, WebDAV, with a folder browser
where the protocol permits it. Restore from a target during first setup.
Capture Docker Compose deployment files automatically when possible.
The user asked for a proposal and review before implementation.

## Result

Wrote the proposal `docs/plans/backup-targets.md`. No code changes.

## Changes

- `docs/plans/backup-targets.md`: proposal with open questions.
- `docs/ai-memory/20261015-claude-backup-targets.md`: this summary.

## Decisions

- Proposed rclone as the single transfer tool (user-space SMB, no extra
  container capabilities). NFS through a host Docker volume.
- Proposed migration ID `20261101000001_backup_targets.sql`.
- Docker socket stays excluded (ADR 0021). Compose capture through a default
  read-only bind; Portainer through an optional Portainer API connection.
- Implementation waits for user approval.

## Checks

- Not run: No code changes yet.
- STE review: Clear English review. Full standard unavailable; review pending.

## Dependencies and open work

- Open questions 1 to 7 in the proposal.
- Migration ID to reserve: 20261101000001. Check for collisions before use.

## Next action

Get user feedback on the proposal. Then implement per the delivery plan.

## References

- [Proposal](../plans/backup-targets.md)
- [Guided backup and restore](20261012-codex-guided-backup-restore.md)
- [ADR 0021](../adr/0021-guided-recovery-and-encrypted-deployment-backups.md)
