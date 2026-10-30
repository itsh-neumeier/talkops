# Updates

[Handbook](index.md) · [Deutsch](../de/aktualisierung.md)

An update can restart FreeSWITCH and end active calls. Use a maintenance window.
The application runs database migrations at startup. Schema changes can prevent
a safe image-only downgrade. Read the target release notes before deployment.

## Before you start

You need host or Portainer administration access. Backup controls require
`backups.manage`; system diagnostics require `diagnostics.manage`.
Use the published [releases](https://github.com/itsh-neumeier/talkops/releases)
and [changelog](../../CHANGELOG.md), not an unmerged feature branch.

1. Record the current image tag, Compose files, project name and active profiles.
2. Create a complete encrypted [backup](backup.md) outside the host.
3. Check that the archive, password and `TALKOPS_SECRET_KEY` are recoverable.
4. Read every intervening release's migration and mount notes.
5. Check **Settings → System → Sessions** for active calls before stopping services.

## Update Docker Compose

The example selects the existing published release 1.8.18. For a later release,
replace the tag and image version together after reading its notes.
Use your existing deployment directory. Preserve `.env` and every active override.

1. Download the target configuration to a separate review directory:

   ```sh
   mkdir -p update-review
   curl -fL -o update-review/docker-compose.yml https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.18/docker-compose.yml
   curl -fL -o update-review/env.example https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.18/.env.example
   diff -u docker-compose.yml update-review/docker-compose.yml
   ```

2. Incorporate required changes into your deployment files. Preserve custom
   mounts, network values, secrets and data volumes. Review backup/target overrides
   from the same release if used. Do not replace your `.env` with the example.
3. Set `TALKOPS_VERSION=1.8.18` in `.env`. `latest` follows the moving stable
   image tag; a fixed version makes the intended update explicit.
4. Validate, pull and start with **all** your existing Compose options:

   ```sh
   docker compose config -q
   docker compose pull
   docker compose up -d
   docker compose ps
   ```

For example, add `-f docker-compose.yml -f docker-compose.backup.yml` to every
command when using that override. Keep the same `--project-name`, `--env-file`
and profiles. A different project name can create fresh empty volumes.

For 1.8.4/1.8.5, server, worker and FreeSWITCH need the same writable fax volume
and UID/GID 10001 access. An image update alone does not repair old stack mounts.
Run the administrator storage check in [fax](fax.md) after the update.

## Update Portainer

Review the target stack file and new environment keys before **Pull and redeploy**.
For repository stacks, select the intended Git release reference and enable image
pulling. Pin `TALKOPS_VERSION` to the corresponding image version. Preserve
environment values, stack identity and volume mappings. Check the stack capture
connection and backup password setup after edits. See [Portainer](portainer.md).

## Verify the result

1. Confirm expected containers are healthy and inspect startup/migration logs.
2. Check **Settings → System → Status**, then sign in with a normal user.
3. Confirm internal, incoming and outgoing test calls with two-way audio.
4. Check the browser phone, voicemail and any configured fax or door station.
5. Create and inspect a new backup and confirm remote copy jobs.

If startup fails, preserve logs and volumes. Follow [troubleshooting](troubleshooting.md).
Do not repeatedly switch images across a migrated schema. Restore the compatible
backup, configuration and image set through the offline [recovery procedure](backup.md)
when recovery is required. A restore replaces data created since the backup.
