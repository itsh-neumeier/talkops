# Backup and restore

## Backup contents

A backup contains a PostgreSQL dump and the volumes `voicemail`, `sounds`,
`provisioning`, `snapshots`, `cdr-spool` and optionally `recordings`. The spool
contains pending and rejected call records. A complete recovery
backup also contains selected `.env`, Compose and override files, including
`TALKOPS_SECRET_KEY`. The manifest records file checksums, schema, application
and PostgreSQL versions, project name, profiles and configuration completeness.

New encrypted archives are named `talkops-YYYYMMDD-HHMMSS.tar.gz.age` and use
the age password format. Keep the separate backup password outside captured
files and the archive. Password loss prevents recovery of an encrypted backup.
Data-only backups without a password remain unencrypted.

Models, Docker images, the FreeSWITCH runtime database and previous archives
are excluded. External mounts and services need their own recovery procedure.
Copy archives to another machine. Same-disk backups cannot survive disk failure.
[Backup targets](#backup-targets) copy archives automatically.

## Complete backups with Compose

`docker-compose.yml` mounts the Compose directory read-only at
`/var/lib/talkops/deployment`. With `TALKOPS_DEPLOYMENT_FILES=auto` (the
default), TalkOps captures these files of the directory automatically:
`.env`, `*.env`, `docker-compose*.yml`/`.yaml` and `compose*.yml`/`.yaml`.
TalkOps leaves out links, files larger than 1 MiB, files that UID 10001
cannot read and files that contain the backup password. **Settings → Backup** shows the captured and the left-out files.

1. Create a separate password file. The operator owns the file; group 10001
   gives the container read access:

   ```sh
   openssl rand -base64 32 > backup-password
   sudo chown "$(id -u)":10001 backup-password
   sudo chmod 640 backup-password
   ```

2. Give the container read access to `.env`. Keep it private otherwise:

   ```sh
   sudo chown "$(id -u)":10001 .env && sudo chmod 640 .env
   ```

3. Start with the backup override. It mounts the password file only:

   ```sh
   docker compose -f docker-compose.yml -f docker-compose.backup.yml up -d
   ```

Include all other active `-f`, `--env-file`, `--project-name` and `--profile`
options each time. Set `COMPOSE_PROJECT_NAME` and `COMPOSE_PROFILES` for captured
metadata. Check metadata when selecting these values through CLI options.

To select files explicitly, set `TALKOPS_DEPLOYMENT_FILES` to a
comma-separated list, for example `.env,docker-compose.yml`. Set
`TALKOPS_DEPLOYMENT_SOURCE` and `TALKOPS_BACKUP_PASSWORD_SOURCE` for different
host paths. The Compose directory must not contain other private data that
UID 10001 can read.

In **Settings → Backups**, set daily hour, archive retention and recording
inclusion. Missing files and credentials are shown. Manual, scheduled
and CLI backups use the same capture and encryption settings:

```sh
docker compose -f docker-compose.yml -f docker-compose.backup.yml exec talkops talkops backup
```

Use `--without-recordings` to exclude recordings. Without deployment files,
data-only backups remain available. Deployment files require encryption.

## Portainer

Portainer keeps the stack in its own data volume. The relative bind of
`docker-compose.yml` then mounts an empty directory, and TalkOps uses the
Portainer API instead:

1. In Portainer, open **My account → Access tokens** and add a token. Use a
   user that can read the TalkOps stack.
2. In TalkOps, open **Settings → Backup** and click
   **Connect Portainer**.
3. Enter the Portainer URL, for example `https://127.0.0.1:9443`.
   `http://` is allowed only for `127.0.0.1`.
4. Click **Check certificate**. For a self-signed certificate, compare the
   SHA-256 fingerprint with the certificate of Portainer and select
   **Pin this certificate**. Later connections refuse other certificates.
5. Enter the token, click **Load stacks** and select the TalkOps stack.
6. Click **Save and read the stack**.

Before each backup, TalkOps reads the stack file (as `docker-compose.yml`) and
the stack environment variables (as `.env`) and stores them encrypted with the
application key. TalkOps leaves out `TALKOPS_BACKUP_PASSWORD`. When Portainer
is not available, the backup uses the last stored copy and the screen shows
the error. The token is stored encrypted and is never shown.

Without a Portainer connection, import the files manually:

1. Copy the actual stack YAML into `docker-compose.yml`.
2. Copy stack environment values into a local `.env`.
3. Select these files and overrides in **Settings → Backups**.
4. Enter the actual project name and profiles.
5. Store the configuration.

Imported files are stored encrypted with the application key. Update them after
every stack change. Mounted files take precedence over the Portainer connection
and imports. CLI capture of imports also needs `TALKOPS_SECRET_KEY`.

Supply `TALKOPS_BACKUP_PASSWORD` separately as a container environment value,
or mount a private file and set `TALKOPS_BACKUP_PASSWORD_FILE`. Do not include
that password in imported `.env` files. Store a separate recovery copy.

## Backup targets

Backup targets receive copies of the backup archives. A backup target has
two parts (ADR 0036):

- A **storage target** holds the connection: protocol, server, folder,
  password or key and upload limit. Open **Settings → Storage** and click
  **Add storage target**. This needs the permission `storage.manage`.
  Other features can use the same storage targets later.
- A **backup job** selects a storage target and an optional sub-folder. It
  has the schedule, the retention and the recordings setting. Open
  **Settings → Backup → Backup jobs** and click **Add backup job**. This
  needs the permission `backups.manage`.

TalkOps copies the archives with [rclone](https://rclone.org). The server
image contains a pinned rclone release.

| Protocol | Folder field | Notes |
| --- | --- | --- |
| SMB | `share/folder` | SMB 2 or 3. TalkOps does not mount the share. |
| S3 | `bucket/folder` | AWS or a compatible service. Enter the endpoint URL for all providers except AWS. Self-hosted servers usually need path-style URLs. |
| WebDAV | folder below the URL | Nextcloud: `https://SERVER/remote.php/dav/files/USER` and an app password. |
| SFTP | folder; `/path` starts at the root | Password or private key without a passphrase. |
| FTP | folder | Explicit TLS is the default. Plain FTP needs the insecure-transport switch. |
| Local | absolute path | Below `TALKOPS_BACKUP_TARGET_ROOTS` (default `/var/lib/talkops/targets`). |

Rules:

- Remote targets receive only encrypted archives. Set
  `TALKOPS_BACKUP_PASSWORD` or `TALKOPS_BACKUP_PASSWORD_FILE` first. See
  [Complete backups with Compose](#complete-backups-with-compose).
- Plain FTP and `http://` URLs send data without encryption. They need the
  switch **Allow transport without encryption**.
- TalkOps stores passwords and keys encrypted with `TALKOPS_SECRET_KEY`. The
  API and the audit log do not show them. Leave a password field empty to keep
  the stored value.
- Use a separate folder for each TalkOps server. Retention deletes TalkOps
  archives in the folder. It does not delete other files.
- Each backup job needs its own folder. TalkOps refuses a second job with
  the same storage target and sub-folder.
- A change of a storage target applies to all jobs that use it. The list
  **Used by** shows these jobs. You cannot delete a storage target that a
  job uses. Delete the job or move it to another storage target first.

### Folder browser and connection test

The storage target editor has the folder browser and the connection test.
**Browse** shows one folder level. Select a folder and click **Use this
folder**. **Create folder** makes a new folder. S3 shows the buckets and SMB
shows the shares when the server permits it. Otherwise, type the bucket or
share name into the folder field.

**Test connection** creates the folder, writes a test file, reads it back and
deletes it. SFTP: the first test or browse accepts the host key of the server
and shows its SHA-256 fingerprint. Compare the fingerprint with the server
(`ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub`) before you save the
target. Later connections refuse a different host key.

**Test** in the storage target list tests the saved connection. The list
shows the result and the time of the last test.

### Schedule

Each backup job has its own schedule in the time zone of the container (`TZ`):

- Select the weekdays. A job with no weekday runs only with **Back up now**.
- Set the time of the first run of the day.
- Optional: repeat the run every 1 to 12 hours until midnight.

A job that is due in the same minute as the daily local backup receives
that archive. For other runs, TalkOps writes an archive into a temporary
folder in the backups volume, copies it to all due jobs and deletes it.
The backups volume needs free space for one archive. **Include call
recordings** is a setting of each job.

### Smart retention

After each upload, TalkOps keeps an archive in the folder of the job when one
of these rules selects it:

- **Latest archives**: the newest N archives (at least 1).
- **Daily**: the newest archive of each day for N days.
- **Weekly**: the newest archive of each week (Monday to Sunday) for N weeks.
- **Monthly**: the newest archive of each month for N months.
- **Yearly**: the newest archive of each year for N years.

The default is 3 latest archives, 7 days, 4 weeks and 12 months: one archive
for each day of the last week, then one for each week of the last month,
then one for each month of the last year. Set all periods to 0 to keep only
the latest N archives. Upload leftovers (`*.partial`) older than six hours are
also deleted.

### NFS, USB disks and SMB mounts

The server container has no Linux capabilities. It cannot mount shares.
Let the Docker daemon of the host mount them and use a local target:

```sh
# .env: TALKOPS_NFS_HOST=nas.local and TALKOPS_NFS_EXPORT=/export/talkops
docker compose -f docker-compose.yml -f docker-compose.targets.yml up -d
```

Then add a local storage target with the folder `/var/lib/talkops/targets/nfs`. The
export must allow writes for UID/GID 10001. A USB disk or another host folder
works the same way with a bind mount (see the comment in
`docker-compose.targets.yml`). In Portainer, add the same volume definition to
the stack.

### Status and monitoring

The job list shows the status, the last successful run, the last error and
the next run of each job. A failed job does not stop the other jobs.
Check the list after changes to the network, the password or the storage
target.

### Update from TalkOps 1.7

The update moves the connection of each backup target into a storage target
with the same name. The backup job keeps its schedule, retention, last run
and archives. Roles with `backups.manage` also get `storage.manage`. No
action is necessary.

## Restore from a backup target

In operation:

1. Click **Archives** at the backup job.
2. Click **To this server** for the archive. TalkOps downloads the archive
   into the local backup list. Existing archives are never replaced.
3. Restore the archive offline:

   ```sh
   bash recover.sh restore --backup talkops-YYYYMMDD-HHMMSS.tar.gz.age \
     --password-file backup-password -f docker-compose.yml --env-file .env
   ```

   `--backup` uses an archive in the backups volume. Add all active
   overrides and options as described below.

During first setup:

1. Choose **Restore from backup** and enter the setup token.
2. Select **From a backup target**. Enter the connection of the target.
   TalkOps does not store these values.
3. Click **Browse**, go to the folder and select an archive.
4. Enter the backup password and click **Inspect backup**. TalkOps downloads
   the archive into the backups volume and shows the review.
5. Use the shown `recover.sh restore --backup …` command on the host.

After the restore, the saved targets of the restored database are active
again.

On a host without a running TalkOps server, `recover.sh fetch` lists and
downloads archives. Put the connection into a private JSON file:

```json
{
  "config": {"kind": "s3", "provider": "Hetzner", "endpoint": "https://fsn1.your-objectstorage.com",
             "region": "", "access_key_id": "KEY", "path_style": false},
  "path": "backups/talkops",
  "secret": {"secret_access_key": "SECRET"}
}
```

```sh
chmod 600 target.json
bash recover.sh fetch --target-file target.json --list
bash recover.sh fetch --target-file target.json --name talkops-YYYYMMDD-HHMMSS.tar.gz.age --output .
```

Other `config` examples: `{"kind": "smb", "host": "nas.local", "user": "backup"}`,
`{"kind": "webdav", "url": "https://cloud.example.com/remote.php/dav/files/u", "vendor": "nextcloud", "user": "u"}`,
`{"kind": "sftp", "host": "backup.example.com", "user": "u", "host_keys": "ssh-ed25519 AAAA…"}`
and `{"kind": "ftp", "host": "ftp.example.com", "user": "u", "tls": "explicit"}`.
Put passwords into `"secret": {"password": "…"}`. For SFTP without `host_keys`,
the command shows the fingerprint; add `--trust-host-key` after you compare it.
Delete the target file after the recovery.

## Recover before the first container start

Use an image that supports format 2. Pin a suitable release tag in production.
Inspection needs no database or application secrets.

1. Save the archive on the target Linux host as `backup.archive`.
2. Put its password in a private file named `backup-password` readable by the
   operator. The host tool supplies it through the container environment.
3. Export the host tool:

   ```sh
   docker run --rm ghcr.io/itsh-neumeier/talkops-server:latest recovery-script > recover.sh
   ```

4. Export deployment files into a new directory:

   ```sh
   bash recover.sh prepare --archive backup.archive --password-file backup-password --output recovered
   ```

   Existing files are never overwritten. The export includes `recovery.json`.
   Exported secrets are plaintext. Keep them private.
5. Review IP addresses, domain, ports, paths, volumes, profiles, project name
   and image versions. Keep the original `TALKOPS_SECRET_KEY`. Supply missing
   configuration. Do not execute untrusted Compose files.
6. Put reviewed files in the target deployment directory. Supply the password
   file needed by the backup override. Give UID 10001 the required read access.
7. Run with all active deployment options:

   ```sh
   bash recover.sh restore --archive /path/to/backup.archive \
     --password-file /path/to/backup-password \
     -f docker-compose.yml -f docker-compose.backup.yml \
     --env-file .env --project-name talkops
   ```

The tool validates the archive and Compose configuration. It asks you to type
`RESTORE`. It stops TalkOps, the media worker and FreeSWITCH. It starts only
PostgreSQL, restores data, then starts the stack and waits for healthy services.
Use `--yes` only after target review. `--image` selects the inspection image.
The application does not need a Docker socket.

## Guided first setup

Choose **Restore from backup**. For browser inspection, set a random
`TALKOPS_SETUP_TOKEN` of at least 32 characters on the host and restart TalkOps.
Enter the token, archive and password. The assistant shows dates, versions,
volumes and missing material. It offers deployment downloads after explicit
acknowledgement about plaintext secrets and gives host/Portainer instructions.
After offline restore, it checks for an account and opens normal login.

Browser limits are 512 MiB uploaded and 8 GiB extracted. Larger backups use
the host tool. CLI staging defaults to 64 GiB; change `inspect --max-bytes` or
`TALKOPS_RESTORE_MAX_BYTES` when needed. Temporary storage must hold compressed
and extracted data. Limits are 100,000 entries, 32 MiB for the manifest and
1 MiB of deployment data. Creation uses the same limits as recovery.

Inspection requires the local token and CSRF request header. Inspection and
administrator creation share a database lock. Access closes when an account
exists. Remove the setup token after setup.

## Legacy backups and manual restore

Format-1 `.tar.gz` archives remain readable. Supply the original `.env`, Compose
files and application key. Without the key, stored credentials are unreadable.
Omit `--password-file` for old archives. Configuration export reports an error
when no deployment files exist.

Stop the three writers and keep PostgreSQL running. Use a temporary container
with the reviewed environment and the same volumes:

```sh
docker compose stop freeswitch talkops media-worker
docker compose up -d --wait postgres
docker compose run --rm --no-deps \
  -v "$PWD/backup.archive:/restore.archive:ro" \
  talkops restore --yes /restore.archive
docker compose up -d --wait
```

Include all active overrides and environment options in each command. Supply
the password for encrypted archives. In Portainer, import reviewed YAML and
environment values, stop the writers, and run the equivalent container on the
host. Match actual volume names. Do not start a competing stack with empty data.

## Validation and failure recovery

Restore validates the complete archive before changes. It rejects wrong
passwords, corrupt data, newer schemas, checksum mismatches, unsafe paths,
links and devices. It stages files and checks the dump with PostgreSQL tools.
Writers and restore processes use a database lock. Backup and restore commands
also use a filesystem lock. Verify lock support for NAS backup storage.
Use real volume directories; backup and restore reject volume-root symlinks.

Older schemas are migrated. Restored sessions and login challenges are cleared.
Accounts, passwords and MFA remain in effect. After login, check service health,
phone registrations and a test call. Enable deployment capture on the target
stack or import the actual Portainer configuration again.

If restore fails, keep writers stopped. Read `restore-state.json` in the backups
volume. Phases are `database`, `volumes` and `complete`. Correct the problem and
retry with the original archive. Preflight failures do not write a phase record.
Database and filesystem replacement are not one transaction. Automatic rollback
of both is not provided.
