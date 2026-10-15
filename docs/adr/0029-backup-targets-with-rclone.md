# 0029 - Backup targets with rclone

- Status: Accepted
- Date: 2026-10-15
- Extends: ADR 0016 (scheduled backups) and ADR 0021 (encrypted recovery backups).

## Context

ADR 0016 writes archives to the backups volume on the same disk. The user
must copy the archives to another place. Homelabs and small businesses use
a NAS (SMB or NFS), S3 storage, WebDAV clouds such as Nextcloud, SFTP or FTP
servers and USB disks. The server container has no Linux capabilities and a
read-only root filesystem (ADR 0016). It cannot mount shares. A new host
must be able to restore from such a place during first setup (ADR 0021).

## Options

1. One native Rust client for each protocol (S3 SDK, FTP, WebDAV, SMB crates).
2. Apache OpenDAL: native Rust for S3, WebDAV, FTP and SFTP, but no SMB.
3. Kernel mounts from the container. These need `CAP_SYS_ADMIN`.
4. rclone as a child process for all protocols.

## Decision

Option 4. The server image contains a pinned rclone release with a SHA-256
check. Each call starts one rclone process. The connection goes into
`RCLONE_CONFIG_T_*` environment variables of that process. Passwords never
appear in arguments, in the process list or in a file on disk. rclone gets
an empty configuration and a private temporary home folder.

- Targets: local folder, SMB, S3, WebDAV, SFTP and FTP in table
  `backup_targets` (migration `20261102000001`). Secrets are JSON encrypted
  with `TALKOPS_SECRET_KEY` (ADR 0008). The API returns `has_secret` only.
- NFS and kernel SMB mounts: the host Docker daemon mounts them as volumes.
  TalkOps uses them as local targets below `TALKOPS_BACKUP_TARGET_ROOTS`.
  Local paths are checked after links are resolved.
- Remote targets receive only age-encrypted archives (ADR 0021).
  Plain FTP and `http://` need an explicit insecure-transport switch.
- Each target has a schedule: weekdays, first run, repeat every N hours.
  A due target receives the daily local archive of the same minute.
  Otherwise one staging archive per recordings setting goes to all due
  targets and is deleted.
- Smart retention per target: latest N, plus the newest archive of each day,
  week, month and year in the given periods. Retention deletes only files
  with TalkOps archive names.
- Uploads go to `NAME.partial` and are renamed (S3 uploads are atomic), then
  checked by size.
- SFTP host keys are pinned on the first test or browse (trust on first use).
  The UI shows the SHA-256 fingerprint for comparison.
- Restore: download into the backups volume, then the offline restore of
  ADR 0021 (`recover.sh restore --backup NAME`). The setup assistant can
  browse and download with the setup token, without stored credentials.
  `talkops fetch` and `recover.sh fetch` work without a database.
- Setup token rate limit counts wrong tokens only, so the folder browser
  stays usable. A failed setup request releases the setup lock at once.

## Consequences

- The server image grows by the rclone binary (about 25 MB compressed).
- The backups volume needs free space for one staging archive.
- Several TalkOps servers in one target folder delete each other's archives.
  The documentation requires one folder per server.
- rclone updates are a deliberate change of the pinned version and checksums
  in the Dockerfile and CI.
- E-mail alerts for failed targets are not included.

Language review: Clear English review only. The official ASD-STE100 standard
was not available. Full language review is pending.
