# 0032 - Automatic deployment capture and Portainer API

- Status: Accepted
- Date: 2026-10-15
- Supersedes: ADR 0021, "capture explicitly selected deployment files" and
  the manual Portainer import as the only Portainer method.
- Keeps: ADR 0021 encryption, separate backup password and no Docker socket.

## Context

ADR 0021 captures deployment files only from an explicit list, and only with
the `docker-compose.backup.yml` override. Portainer users must import the
stack file and the environment by hand after each stack change. Many users
forget one of the files or the update. Then a recovery backup is incomplete.
The container has no Docker socket and cannot read host files that are not
mounted. Portainer keeps stacks in its own data volume.

## Options

1. Keep the explicit list and the manual import.
2. Mount the Docker socket and read the Compose labels. ADR 0021 rejects this.
3. Mount the Compose directory by default and discover the files. Read
   Portainer stacks through the Portainer API with an access token.

## Decision

Option 3.

- `docker-compose.yml` mounts `${TALKOPS_DEPLOYMENT_SOURCE:-.}` read-only at
  `/var/lib/talkops/deployment`. `TALKOPS_DEPLOYMENT_FILES=auto` is the
  default. Discovery takes `.env`, `*.env`, `docker-compose*.yml|yaml` and
  `compose*.yml|yaml`. It leaves out links, files above 1 MiB, unreadable
  files and files with the backup password, and reports them. An explicit
  list keeps the ADR 0021 behavior.
- A mounted directory without a Compose file (the empty bind of a Portainer
  stack) does not count. Then the Portainer connection or an import applies.
- Portainer connection (table `portainer_connections`, migration
  `20261102000002`): URL, stack ID and access token, encrypted with
  `TALKOPS_SECRET_KEY`. Before each backup, TalkOps reads
  `GET /api/stacks/{id}` and `GET /api/stacks/{id}/file` with `X-API-Key`.
  It stores the stack file as `docker-compose.yml` and the environment as
  `.env` in the encrypted import file. `TALKOPS_BACKUP_PASSWORD` is left out.
  A failure keeps the last copy and is shown.
- `https://` is required, except `http://` to `127.0.0.1`. A self-signed
  certificate is pinned by its SHA-256 fingerprint after the administrator
  compares it. The certificate probe sends no token.

## Consequences

- The container can read all files in the Compose directory that UID 10001
  can read. The documentation tells users to keep other private data out.
- `.env` needs read access for UID 10001 for automatic capture.
- A Portainer token has the rights of its user. The documentation recommends
  a user that can read the TalkOps stack only.
- Additional files of Git-based Portainer stacks are not read; only the main
  stack file.
- Existing installations with an explicit `TALKOPS_DEPLOYMENT_FILES` list
  keep their behavior.

Language review: Clear English review only. The official ASD-STE100 standard
was not available. Full language review is pending.
