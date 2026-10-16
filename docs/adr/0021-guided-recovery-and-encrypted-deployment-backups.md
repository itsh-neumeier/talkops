# 0021 - Guided recovery and encrypted deployment backups

- Status: Accepted
- Date: 2026-10-12
- Supersedes: ADR 0016 archive format and restore validation details.
- Keeps: ADR 0016 offline restore and ADR 0008 application encryption.
- Partly superseded by [ADR 0032](0032-automatic-deployment-capture.md): automatic
  file discovery and the Portainer API connection.

## Context

First setup only creates an administrator. Old backups contain application
data but no deployment files or application key. A new host needs those files
before its first container start. Portainer can store environment values
without a source `.env`. The application cannot change host files or stop
Docker services without additional privileges.

## Decision

Add a restore choice to first setup. The web assistant inspects archives,
reports recovery material and exports deployment files. A local setup token
protects inspection. The setup database lock prevents concurrent setup work.
Inspection closes after an account exists.

Provide standalone CLI inspection and configuration export without a database.
Provide a host script for offline Compose recovery. Stop TalkOps, the media
worker and FreeSWITCH before restoring. Keep PostgreSQL running. Do not mount
the Docker socket into the application.

Capture explicitly selected deployment files from a read-only directory.
Support administrator imports for Portainer. Encrypt imported files at rest
with the existing application key. Show the import date and require updates
after stack changes. Report missing selected files and credentials.

Write format-2 archives with SHA-256 file inventories. Use the age password
format when deployment secrets are included. A separate recovery password
must come from a runtime value or separate file. Do not capture that password.
Keep format-1 readers for older data-only archives.

Authenticate and validate archives through EOF before data changes. Stage
files privately. Reject unsafe paths, links, duplicate entries and unsupported
schemas. Apply extraction limits. Check dumps with PostgreSQL client tools.
Use database and filesystem locks for supported writers and recovery tools.
Clear restored sessions and login challenges. Record restore phases.

## Consequences

Full recovery requires the encrypted archive and its separate password.
Password loss prevents recovery. Exported deployment files contain plaintext
secrets and need private storage. Existing format-1 archives still need the
original deployment configuration and application key.

Temporary storage must hold the compressed payload and extracted files.
Database and filesystem replacement do not form one transaction. A failed
restore keeps writers stopped and needs correction and retry from the archive.
Existing binaries that predate these locks must be stopped explicitly.

Dependencies are `age`, `fs2` and `tempfile`. No schema migration is required.

Language review: Clear English review only. The official ASD-STE100 standard
was not available. Full language review is pending.
