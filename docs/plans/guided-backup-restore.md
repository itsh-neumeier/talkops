# Guided backup and restore proposal

Status: Accepted by the user on 2026-10-12. Implementation is complete. See the task summary for results.
Date: 2026-10-12.
Base: `2fbe3195517b509d2afd3d57247c05a98437f064`.

## Current behavior

- `/setup` creates the first administrator. It has no restore option.
- Backups contain `manifest.json`, `database.dump` and five data volumes.
  The user can exclude recordings.
- Backups do not contain deployment files or `TALKOPS_SECRET_KEY`.
- `talkops restore --yes` replaces the database and included volume contents.
  It requires a separate command and stopped application services.
- The server container has a read-only root filesystem. It cannot change
  host deployment files or stop the Docker stack.
- Portainer environment values do not necessarily have a source `.env` file.

Sources: `crates/talkops-api/src/backup.rs`,
`crates/talkops-api/src/main.rs`, `web/src/routes/setup/+page.svelte`,
`docker-compose.yml`, `docs/en/backup.md` and `docs/en/portainer.md`.

## Proposed user flow

1. Show two choices on first setup: create a new instance or restore a backup.
2. In the restore assistant, select an archive and enter its backup password.
   For an old archive, request the original application key when necessary.
3. Show the backup date, application version, schema version, included data
   and deployment files. Show missing files and compatibility problems.
4. Prepare deployment files for download. Ask the user to check host-specific
   values: IP addresses, domain, ports, profiles, paths and volume storage.
   Keep the original application key so stored credentials remain readable.
5. Show commands for Docker Compose and instructions for Portainer.
   Explain the service stop, restore, restart and verification steps.
6. Run the restore offline through a host tool. Stop TalkOps, the media worker
   and FreeSWITCH before replacing data. Keep PostgreSQL running.
7. After restart, show the normal login. Use the restored accounts and MFA.
   Verify service health and report the restore result.

The web assistant prepares the operation. It does not replace its live
database. It does not need a Docker socket or privileged container access.
Protect archive inspection and configuration download with a local bootstrap
token. Close this access after setup. Apply request, storage and extraction
limits. Prevent races between administrator creation and restore preparation.
Do not show secret values in the review screen or record them in logs.

## Recovery before the first container start

Add a host recovery tool that can inspect a backup and export deployment
files without a database connection or a running TalkOps server. Supply a
container-based command for hosts without the TalkOps binary.

The tool exports into a new directory. It does not overwrite existing `.env`
or Compose files. The user checks these files before deployment. The tool
then guides the offline restore and service restart. It must use the selected
Compose files, profiles, environment files and project name.

This entry point is necessary when a new host has no passwords, key or stack.
The web assistant alone cannot solve that case because normal server startup
already requires a valid database connection and application secrets.

## Backup contents and configuration capture

Keep the current database and media contents. Add a versioned deployment
section with:

- The source `docker-compose.yml`, selected override files and environment
  files, including `.env` and `TALKOPS_SECRET_KEY`.
- Deployment metadata: application and PostgreSQL versions, image references,
  Compose project name, enabled profiles and configuration completeness.
- A file inventory and checksums for validation.

Mount a selected deployment directory read-only into TalkOps for scheduled
and manual backups. Read an explicit file list. Do not scan the host or copy
unrelated files. Capture the same information for CLI backups.

For Portainer, provide an administrator import for stack YAML and environment
values. Store imported deployment secrets encrypted at rest with the existing
application key. Report when imported configuration can be out of date.
The application cannot infer the original stack from its own environment.
No Portainer API integration is required for this proposal.

Show whether each backup contains a complete recovery configuration. A data
backup can still run when deployment files are absent, but the UI must clearly
report the missing recovery material. A backup with omitted recordings must
also report that omission.

Models, FreeSWITCH runtime databases, Docker image layers and previous backup
archives remain outside the backup. External mounts and services need explicit
recovery instructions. Do not describe an archive as a complete host backup.

## Encryption and compatibility

Encrypt the complete new archive when it includes deployment secrets. Use
an established password-based format and maintained implementation. Select
the dependency during implementation; do not create a custom crypto format.

Use a backup password separate from `TALKOPS_SECRET_KEY`. Scheduled backups
need this password supplied at runtime. Keep it outside the archive. Do not
use the application key as the recovery password: that key is itself part
of the recovery material. Document password loss and recovery limitations.

Continue to read existing format-1 `.tar.gz` archives. Their deployment
configuration and key must come from the user. Report this limitation before
restore. Reject unsupported formats and schema versions before data changes.
Older application versions cannot read the new encrypted format.

Record the format change in a new ADR. Keep ADR 0008's application encryption
rule. Update ADR 0016's archive and restore sections through the new ADR.

## Restore validation and failure behavior

Before changing data, decrypt and validate the complete archive. Check its
manifest, file inventory, checksums, database dump, schema compatibility,
paths and available storage. Reject unsafe links and paths. Stage files before
replacing the database or volumes. Remove temporary secret files on exit.

Stop application writers through the host workflow. A process-local backup
mutex alone does not coordinate separate server and CLI processes. Provide
a restore lock and state record for the supported workflow. Block competing
restore attempts. Do not claim the web process can verify arbitrary host
services without a host tool.

Report a failed phase and a retry procedure. A database restore and filesystem
replacement are not one transaction. Do not promise automatic rollback of
both. Preserve the source archive and configuration exports after failure.
Invalidate restored web sessions and login challenges; use a new login.

## Implementation areas

- Backup format, configuration capture and preflight in `backup.rs`.
- CLI inspection, configuration export and restore commands in `main.rs`.
- First-setup restore preparation API and setup state coordination.
- Setup assistant, backup completeness display and English/German UI strings.
- A host recovery tool, Compose mounts and `.env.example` options.
- Database migration only if imported deployment configuration or restore
  coordination needs persistent storage. No migration ID is reserved yet.
- English user documentation and German translations for installation,
  Portainer and backup recovery. A new ADR and task summary.

## Planned verification

- Round trip: deployment files, application key, database, media and readable
  restored credentials. Include a different target host configuration.
- Legacy archive recovery with supplied key and missing-configuration notices.
- Wrong password, corrupted or truncated archive, unsafe paths and links,
  unsupported schema, storage limits and failed restore phases.
- First-setup authorization, bootstrap token, CSRF, setup/restore races,
  closed access after setup and restored-session invalidation.
- Scheduled and manual configuration capture, missing files, stale imported
  configuration notices and optional recordings.
- Host workflow: selected Compose files and project, service stop/start,
  configuration export without overwrite and failure recovery.
- Browser flow from first setup to restore guidance and normal login.
- CI contract: Rust fmt, clippy and workspace tests with PostgreSQL 17 and
  its client tools; web lint, check and build; Compose validation and image
  smoke tests. Run relevant Linux integration checks where available.

## Review decision

The user accepted this scope before implementation. The accepted decisions
include a separate backup password and a guided host step for offline restore.
See the task summary for implementation and verification results.

Language review: Clear English review. The official ASD-STE100 standard was
not available. Full language review is pending.
