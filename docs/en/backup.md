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

## Complete backups with Compose

1. Create a separate password file. The operator owns the file; group 10001
   gives the container read access:

   ```sh
   openssl rand -base64 32 > backup-password
   sudo chown "$(id -u)":10001 backup-password
   sudo chmod 640 backup-password
   ```

2. Set `TALKOPS_DEPLOYMENT_FILES` in `.env` to every active source file, for
   example `.env,docker-compose.yml,docker-compose.backup.yml`. Add each active
   override and selected `*.env` file. Remove unused filenames. Do not include
   a file that contains the backup password.
3. Start with the backup override:

   ```sh
   docker compose -f docker-compose.yml -f docker-compose.backup.yml up -d
   ```

Include all other active `-f`, `--env-file`, `--project-name` and `--profile`
options each time. Set `COMPOSE_PROJECT_NAME` and `COMPOSE_PROFILES` for captured
metadata. Check metadata when selecting these values through CLI options.

The override mounts the deployment directory read-only and reads selected files
only. The password is mounted separately. Set `TALKOPS_DEPLOYMENT_SOURCE` and
`TALKOPS_BACKUP_PASSWORD_SOURCE` for different host paths. Keep `.env` private
and give UID 10001 read access to mounted configuration and password files.

In **Settings → Backups**, set daily hour, archive retention and recording
inclusion. Missing selected files and credentials are shown. Manual, scheduled
and CLI backups use the same capture and encryption settings:

```sh
docker compose -f docker-compose.yml -f docker-compose.backup.yml exec talkops talkops backup
```

Use `--without-recordings` to exclude recordings. Without deployment files,
data-only backups remain available. Deployment files require encryption.

## Portainer

TalkOps cannot infer the original Portainer stack from its own environment.

1. Copy the actual stack YAML into `docker-compose.yml`.
2. Copy stack environment values into a local `.env`.
3. Select these files and overrides in **Settings → Backups**.
4. Enter the actual project name and profiles.
5. Store the configuration.

Imported files are stored encrypted with the application key. Update them after
every stack change. The screen and manifest show the import date. Mounted files
take precedence. CLI capture of imports also needs `TALKOPS_SECRET_KEY`.

Supply `TALKOPS_BACKUP_PASSWORD` separately as a container environment value,
or mount a private file and set `TALKOPS_BACKUP_PASSWORD_FILE`. Do not include
that password in imported `.env` files. Store a separate recovery copy.

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
