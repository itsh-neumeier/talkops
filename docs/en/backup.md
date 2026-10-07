# Backup and restore

A TalkOps backup is a single file `talkops-YYYYMMDD-HHMMSS.tar.gz` in the
`backups` volume. It contains:

- the database (configuration, users, call log, voicemail and recording
  metadata, transcripts, audit log, encrypted SIP/trunk passwords),
- the volumes `voicemail`, `sounds`, `snapshots`, `provisioning` and,
  unless switched off, `recordings`.

The Whisper models and the FreeSWITCH runtime database are not backed up;
they are downloaded or rebuilt automatically.

> **Keep `TALKOPS_SECRET_KEY` safe, separately from the backups.** The SIP,
> trunk, LDAP and door station passwords in the backup are encrypted with
> it. Without the key a restore works, but every one of those passwords
> must be entered again. Also keep your `.env` file.

## Scheduled backups

**Settings → Backups** (admins):

- **Daily backup** at the given hour (time zone of the container, `TZ` in
  `.env`; default 03:00).
- **Backups kept**: older archives are deleted after each backup
  (default 7).
- **Include call recordings**: switch off when recordings are large and
  backed up otherwise.
- **Back up now** starts a backup immediately; **Download** saves an
  archive on your computer.

A backup on the same disk does not survive a disk failure. Copy the
archives elsewhere regularly, e.g. download them, or mount a NAS share as
the `backups` volume:

```yaml
# docker-compose.override.yml
volumes:
  backups:
    driver: local
    driver_opts:
      type: nfs
      o: addr=nas.local,rw,nfsvers=4
      device: ":/volume1/talkops-backups"
```

The share must be writable for UID/GID 10001.

## Backup from the command line

```sh
docker compose exec talkops talkops backup
# prints /var/lib/talkops/backups/talkops-20261012-030000.tar.gz
docker compose cp talkops:/var/lib/talkops/backups/talkops-20261012-030000.tar.gz .
```

`--without-recordings` leaves the recordings out.

## Restore

A restore **replaces** the database and the contents of the volumes in the
archive. An archive from an older TalkOps version can be restored on a
newer one (the database is migrated afterwards), not the other way round.

1. Stop TalkOps and the media worker (FreeSWITCH and Postgres keep running):

   ```sh
   docker compose stop talkops media-worker
   ```

2. Restore. For an archive in the `backups` volume:

   ```sh
   docker compose run --rm talkops restore --yes \
     /var/lib/talkops/backups/talkops-20261012-030000.tar.gz
   ```

   For a downloaded archive (e.g. on a new server, after `docker compose up -d`
   with the same `.env`):

   ```sh
   docker compose stop talkops media-worker
   docker compose run --rm -v "$PWD/talkops-20261012-030000.tar.gz:/restore.tar.gz:ro" \
     talkops restore --yes /restore.tar.gz
   ```

3. Start everything again; phones re-register on their own:

   ```sh
   docker compose up -d
   ```

In Portainer: stop the `talkops` and `media-worker` containers and run the
commands above on the host.
