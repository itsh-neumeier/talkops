# Backup targets proposal

Status: Accepted by the user on 2026-10-15 with changes (see "Accepted
decisions"). Part 1 (targets, schedules, retention, restore) is implemented.
Part 2 (automatic deployment capture, Portainer API) follows in a second PR.
Date: 2026-10-15.
Base: `702507b39fb1cf80db4ce5a0f9dd3d339b9da68c` (`main`, release 1.7.13).

## Accepted decisions

The user answered the open questions on 2026-10-15:

1. rclone in the server image: yes (pinned v1.75.2 with SHA-256 checks).
2. Remote targets require encrypted archives: yes.
3. NFS through a Docker volume of the host: yes.
4. SFTP as an extra protocol: yes.
5. Default read-only bind of the Compose directory and the Portainer API
   connection: yes. This is part 2.
6. Changed: each target has a flexible schedule (weekdays, first run,
   repeat every N hours) and a smart retention similar to Synology:
   latest N, then one archive per day, week, month and year for given
   periods. This replaces the single `keep` count in section 2.
7. Two PRs: part 1 = sections 1 to 5 and 7 to 9; part 2 = section 6.

Implementation changes against the proposal:

- SFTP host keys: rclone pins the key on the first test or browse (trust on
  first use). The UI shows the SHA-256 fingerprint. A saved SFTP target
  needs a pinned key.
- The setup token rate limit counts wrong tokens only. A failed setup
  request releases the setup lock at once.
- E-mail alerts for failed targets are not part of this work.

## Requested result

- Store backup targets: SMB, NFS, local path, S3 bucket, FTP and WebDAV.
- Browse the remote location when the protocol and the target permit it.
- Restore from a target during first setup (target as restore source).
- Capture the Docker Compose deployment files automatically when possible.

## Current behavior

- `backup::create` writes one archive into `TALKOPS_BACKUP_DIR`
  (volume `backups`). `prune` keeps the last N archives there.
- The daily schedule, the retention count and the recordings switch are in
  `backup_settings`. There is no remote copy. The documentation tells the
  user to copy archives to another machine.
- The server container uses `cap_drop: ALL`, `no-new-privileges` and a
  read-only root filesystem. It cannot mount SMB or NFS shares.
- Deployment files are captured only with the `docker-compose.backup.yml`
  override (read-only bind of the Compose directory) or by a manual import
  for Portainer.
- The restore assistant in first setup accepts an uploaded archive only.

Sources: `crates/talkops-api/src/backup.rs`, `crates/talkops-api/src/recovery.rs`,
`crates/talkops-core/src/backups.rs`, `docker-compose.yml`,
`docker-compose.backup.yml`, ADR 0016 and ADR 0021.

## Proposed approach

### 1. Transfer engine: rclone

Use [rclone](https://rclone.org) as the single transfer tool. The server
runs it as a child process, as it runs `pg_dump` now.

| Target   | rclone backend          | Browse                              |
|----------|-------------------------|-------------------------------------|
| Local    | `local`                 | Yes, only below allowed roots       |
| SMB      | `smb` (SMB2/3, no mount) | Shares and folders                 |
| NFS      | `local` on a Docker NFS volume | Yes, inside the mount       |
| S3       | `s3` (AWS, MinIO, Wasabi, Hetzner, Backblaze, Garage, ...) | Buckets if permitted, then prefixes |
| FTP/FTPS | `ftp` (explicit TLS default) | Folders                        |
| WebDAV   | `webdav` (Nextcloud, ownCloud, generic) | Folders             |
| SFTP     | `sftp` (optional extra) | Folders                             |

Reasons:

- One proven, maintained component covers all protocols. This agrees with
  the project rule "use proven components".
- rclone supplies listing (`lsjson`), upload with checksum check (`copyto`),
  deletion, retries, timeouts and a bandwidth limit (`--bwlimit`).
- SMB works in user space. No kernel mount, no extra capability.
- Configuration goes through environment variables of the child process
  (`RCLONE_CONFIG_<NAME>_*`). Secrets never appear in a process list or in
  a configuration file on disk.

Rejected alternatives:

- Native Rust crates per protocol (`aws-sdk-s3`, `suppaftp`, a WebDAV client,
  the young `smb` crate): five code paths, five error models, more upkeep.
- Apache OpenDAL: native Rust and good for S3/WebDAV/FTP/SFTP, but no SMB.
- Kernel mounts from the container: need `CAP_SYS_ADMIN`. This breaks the
  hardening in ADR 0016.

Image: install a pinned official rclone release for amd64 and arm64 with a
SHA-256 check in `docker/talkops/Dockerfile`. The Debian trixie package is
an older release line. Cost: the server image grows by about 25 MB
(compressed).

NFS: rclone has no NFS client. The host mounts the share through a Docker
volume (`driver_opts: type: nfs`). TalkOps uses it as a local target below
`/var/lib/talkops/targets/`. The documentation supplies a Compose override
and Portainer steps. SMB can use the same method as an alternative.

### 2. Data model (one migration)

Migration ID: `20261102000001_backup_targets.sql` (after the current last
ID `20261031000001`).

```sql
CREATE TABLE backup_targets (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     UUID NOT NULL REFERENCES tenants (id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('local','smb','s3','ftp','webdav','sftp')),
    enabled       BOOLEAN NOT NULL DEFAULT true,
    settings      JSONB NOT NULL DEFAULT '{}',  -- host, port, share, bucket, region, endpoint, user, TLS ...
    secret        TEXT,                         -- ADR 0008 ciphertext: password, secret key, SSH key
    path          TEXT NOT NULL DEFAULT '',     -- folder or prefix on the target
    keep          SMALLINT NOT NULL DEFAULT 14 CHECK (keep BETWEEN 1 AND 365),
    bwlimit_kbps  INTEGER CHECK (bwlimit_kbps > 0),
    last_upload_at TIMESTAMPTZ,
    last_file     TEXT,
    last_error    TEXT,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name)
);
```

- `settings` holds no secret. A typed Rust enum validates it per kind.
- The API never returns `secret`. It returns `has_secret: true` only.
  An empty secret field in an update keeps the stored value.
- The existing `backup_settings.keep` stays the local retention count.

### 3. Backup flow

1. The schedule writes the archive into the local backup directory as now.
2. After success, the server uploads the archive to each enabled target:
   `rclone copyto <archive> <target>:<path>/<name>.partial`, check the size
   and hash, then rename to `<name>`. An interrupted upload leaves no
   archive with a valid name.
3. Retention per target: list `<path>`, select only names that pass
   `backup::valid_name`, delete the oldest beyond `keep`. Other files are
   never deleted.
4. Record the result per target (`last_upload_at`, `last_file`,
   `last_error`). One failed target does not stop the other targets.
5. Optional local mode "keep only the latest archive locally" for small
   disks (local `keep = 1`). The local copy remains the staging area.
6. Failures show on the dashboard. Optional e-mail to administrators with
   the existing mailer.

Encryption: a remote copy leaves the host. I propose that targets other than
`local` require the backup password (encrypted `.tar.gz.age` archives). The
UI shows the reason and the setup steps. See question 2.

### 4. Target browser

- `POST /api/v1/backup-targets/browse` with a saved target ID or an unsaved
  draft, and a path. It returns folders (and archives, as information).
- The browser goes one level per request (`rclone lsjson --max-depth 1`),
  with a timeout of 15 s and a limit of 1,000 entries.
- S3: the root lists buckets if the key permits `ListAllMyBuckets`.
  Otherwise the user types the bucket name and browses its prefixes.
- SMB: the root lists shares if the server permits it. Otherwise the user
  types the share name.
- Local and NFS: browse only below `TALKOPS_BACKUP_TARGET_ROOTS`
  (default `/var/lib/talkops/targets`). Paths with `..` or links outside a
  root are rejected.
- "Create folder" in the browser, where the protocol supports it.
- "Test connection": write, read back and delete a small test file. Show a
  clear error (DNS, TLS, authentication, permission, free space if known).
- SFTP: show the host key fingerprint on the first test. The administrator
  accepts it. TalkOps stores it and checks it on each connection.

### 5. Restore from a target

In operation (Settings -> Backups):

- Each target shows its archives (name, size, date).
- "Download to this server" copies one archive into the local backup
  directory. The existing offline restore (`scripts/recover.sh`,
  `talkops restore`) then uses it as now. No live restore in the web UI
  (ADR 0016/0021 rule stays).

During first setup (restore assistant):

- A new source choice: "Upload archive" (as now) or "Get from backup target".
- The user enters the target connection data. The database of a new host has
  no targets. The setup token protects the step as it protects inspection now.
- The assistant uses the same browser, shows the archives, downloads the
  selected archive to the local backup directory and starts the existing
  inspection. The entered connection data is not stored. After the restore,
  the targets of the restored database are active again.
- The host tool gets the same function for a host with no running server:
  `talkops fetch --target <kind> ...` and an option in `scripts/recover.sh`.
  Credentials come from an environment file or a prompt, never from the
  command line.

### 6. Automatic capture of deployment files

Docker socket access stays excluded (ADR 0021). Without the socket, the
container cannot read host files that are not mounted. So:

- Docker Compose: move the read-only bind of the Compose directory
  (`${TALKOPS_DEPLOYMENT_SOURCE:-.}`) from `docker-compose.backup.yml` into
  `docker-compose.yml`. Then each Compose installation captures its files
  without an override.
- Auto discovery in that directory when `TALKOPS_DEPLOYMENT_FILES` is
  `auto` (new default): `.env`, `*.env`, `docker-compose*.yml`,
  `docker-compose*.yaml`, `compose*.yml`, `compose*.yaml`. Excluded: the
  backup password file, files that contain the backup password, files above
  1 MiB and links. An explicit list still has priority.
- The backup password file stays a separate, optional bind. Without the
  password, the archive is data-only and the UI tells the user that the
  deployment files are not included (as now).
- Portainer: Portainer stores stacks in its own data volume. A relative bind
  resolves to a path that is normally not useful on the host. Proposal:
  optional Portainer API connection (URL, access token, stack ID; token
  encrypted per ADR 0008). Before each backup, TalkOps reads the stack file
  (`GET /api/stacks/{id}/file`) and the stack environment values
  (`GET /api/stacks/{id}`). This replaces the manual import. The manual
  import remains as a fallback.

### 7. API, UI and permissions

- New routes in `crates/talkops-api/src/routes/backup_targets.rs`:
  list, create, update, delete, test, browse, list archives, fetch archive.
  Admin role only. Each mutation writes an audit entry without secrets.
- UI: a "Backup targets" section in the backup settings: list with status,
  modal with protocol choice, form fields per protocol, folder browser,
  "Test connection". All strings in `en.ts` and `de.ts`.
- Setup: new source choice in `RestoreAssistant.svelte`.

### 8. Security

- Secrets: ADR 0008 encryption, never logged, never in rclone arguments.
  Child process with a cleared environment, timeouts and `kill_on_drop`.
- Server-side requests to administrator-supplied hosts: admin only, audited,
  setup step only with the setup token. Plain FTP and HTTP WebDAV need an
  explicit "insecure transport" switch with a warning.
- Restore downloads use the existing size limit
  (`TALKOPS_RESTORE_MAX_BYTES`) and the existing archive validation.

### 9. Tests and documentation

- Unit tests: settings validation per kind, rclone environment building
  (no secret in arguments), retention selection, path checks for local roots.
- `#[sqlx::test]` tests for the target queries and the secret handling.
- Integration tests with rclone against a local target and against local
  test servers in CI where practical (MinIO for S3, rclone `serve webdav`
  and `serve ftp`). SMB and SFTP: rclone serve where available, else
  documented manual tests.
- Web: lint, check, build.
- New ADR 0031 "Backup targets with rclone". It extends ADR 0016 and 0021
  and does not contradict them.
- User guides `docs/en/backup.md` and `docs/de/backup.md`: targets per
  protocol, NFS/SMB volume example, Portainer API connection, restore from a
  target. `docs/en/portainer.md` and the German translation.

## Delivery plan

Implement in one PR, with focused commits, unless the review asks for a split:

1. Migration, core types and queries, rclone wrapper, upload and retention.
2. API routes, browser, test connection, archive list and fetch.
3. UI: target management and browser, dashboard status.
4. Setup restore from a target, host tool `talkops fetch`.
5. Automatic deployment capture and the optional Portainer API connection.
6. ADR, user documentation, tests, task summary.

Possible split: steps 1 to 4 as the first PR, step 5 as a second PR.

## Open questions for the user

1. Do you accept rclone as an additional component in the server image?
2. Must remote targets require encrypted archives (backup password)?
3. NFS through a Docker volume on the host (proposal). Is that acceptable,
   or do you need a different way?
4. Add SFTP as an extra protocol (no additional cost with rclone)?
5. Compose: may `docker-compose.yml` mount the Compose directory read-only by
   default? Portainer: implement the optional Portainer API connection?
6. Upload to all targets after each backup (proposal), or a target
   selection or a separate schedule per target?
7. One PR or two PRs (see delivery plan)?

Language review: Clear English review only. The official ASD-STE100 standard
was not available.
