# Running with Portainer

[Handbook](index.md) · [Deutsch](../de/portainer.md)

## Before you start

You need: Portainer stack administration and a Linux Docker endpoint.

For complete recovery backups, connect TalkOps to the Portainer API in
**Settings → Backup**. TalkOps then reads the stack file and the stack
environment before each backup. Supply the backup password separately. See
[Backup and restore](backup.md#portainer) for the connection, manual imports
and new-host recovery.

`docker-compose.yml` only uses prebuilt images from GHCR and can therefore be
used directly as a Portainer stack.

1. **Stacks → Add stack**, name `talkops`.
2. Build method **Repository** (repository URL
   `https://github.com/itsh-neumeier/talkops`, compose path `docker-compose.yml`)
   with release reference `refs/tags/v1.8.21`, or **Web editor** and paste the
   contents of `docker-compose.yml` from that release.
3. Under **Environment variables** → **Advanced mode** paste the contents of
   `.env.example` and fill in the required values (`POSTGRES_PASSWORD`,
   `TALKOPS_SECRET_KEY`, `TALKOPS_ESL_PASSWORD`, `TALKOPS_XMLCURL_PASSWORD`).
   Generate random values e.g. with `openssl rand -hex 32`.
   Set `TALKOPS_VERSION=1.8.21` to match the configuration release. Keep existing
   secrets when updating a stack. Use [updates](updates.md) for a later version.
4. **Deploy the stack**. After about a minute all containers should be healthy.

Notes:

- The Portainer endpoint must be a Linux Docker host (host networking).
- Updates: open the stack → **Pull and redeploy** (for repository stacks,
  enable "Re-pull image").
- Back up `TALKOPS_SECRET_KEY` outside of Portainer as well.

## Check the result

Check image versions, stack mounts and backup configuration capture.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
