# Installation

[Handbook](index.md) · [Deutsch](../de/installation.md)

## Before you start

You need: Host administration access and a new instance or a recovery plan.

## Requirements

- Linux host (x86_64 or arm64), e.g. Debian/Ubuntu, Proxmox VM, NAS with Docker
- Docker Engine ≥ 24 with the Compose plugin
- Free ports on the host (TalkOps uses host networking):

| Port | Protocol | Purpose |
|---|---|---|
| 5060 | UDP/TCP | SIP for phones |
| 5080 | UDP/TCP | SIP for trunks (providers) |
| 16384–16999 | UDP | RTP (voice/video), configurable |
| 8080 | TCP | Web UI / API |
| 8021, 5432 | TCP | local only (127.0.0.1): FreeSWITCH ESL, PostgreSQL |

Docker Desktop (macOS/Windows) is not supported because host networking does
not fully work there.

## Install with Docker Compose

For recovery on a new host, export the backup configuration before creating
new credentials. See [Backup and restore](backup.md). First setup also has
a restore assistant.

```sh
mkdir talkops && cd talkops
curl -fLO https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.11/docker-compose.yml
curl -fL -o .env https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.11/.env.example
sed -i 's/^TALKOPS_VERSION=.*/TALKOPS_VERSION=1.8.11/' .env
```

Set the required values in `.env`, for example:

```sh
sed -i "s/^POSTGRES_PASSWORD=$/POSTGRES_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_SECRET_KEY=$/TALKOPS_SECRET_KEY=$(openssl rand -hex 32)/" .env
sed -i "s/^TALKOPS_ESL_PASSWORD=$/TALKOPS_ESL_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_XMLCURL_PASSWORD=$/TALKOPS_XMLCURL_PASSWORD=$(openssl rand -hex 16)/" .env
```

> **Important:** `TALKOPS_SECRET_KEY` encrypts stored SIP and trunk
> credentials. Back it up separately from the database – without it, the
> credentials cannot be decrypted after a restore.

These files and images use the existing published release 1.8.11. For a later
release, select matching configuration and image versions. `latest` is a moving
stable image tag; it does not pin the configuration files.
Protect `.env` after generating secrets with `chmod 600 .env`. If you enable
complete configuration backups, use the group-read setup in [backup](backup.md).

Start:

```sh
docker compose up -d
docker compose ps        # every service should be "healthy"
```

The web UI is available at `http://<host>:8080`, component status at
`http://<host>:8080/api/v1/status`, the API description (OpenAPI) at
`http://<host>:8080/api/v1/openapi.json`. Continue with the [First steps](first-steps.md).

## HTTPS with Caddy (optional)

For a publicly reachable web UI with a Let's Encrypt certificate, add to `.env`:

```sh
COMPOSE_PROFILES=caddy
TALKOPS_DOMAIN=pbx.example.com
```

Caddy uses ports 80 and 443 on the host.

## Updates

Follow [updates](updates.md). Create a backup before updating and review target
release notes, migrations and mount changes. Keep all active Compose options.
Database migrations run at startup. A FreeSWITCH restart ends active calls.
An image-only downgrade is not a safe substitute for a compatible restore.

## Alternative: macvlan

If port 5060 is already used on the host (e.g. by another PBX), FreeSWITCH can
get its own LAN IP. `talkops` and `freeswitch` must then reach each other: set
`TALKOPS_ESL_ADDR`, `TALKOPS_XMLCURL_URL` and `TALKOPS_ESL_LISTEN_IP` to the
respective IPs, allow FreeSWITCH's address with `TALKOPS_FS_PEERS` (TalkOps
otherwise only answers configuration requests from the same host) and use a
strong ESL password. This setup is not part of the tested standard installation.

## Security

- **SIP from the internet:** only open 5060 if remote phones need it; a VPN
  (WireGuard, Tailscale) is safer. Trunks register outbound and need no
  inbound port forwarding with most providers.
- **SIP login protection** (*Settings → SIP login protection*): an address
  with 10 failed logins (wrong password or unknown user) within 10 minutes is
  banned for an hour – every login from it fails, even with the right
  password. Add your LAN as trusted network if many phones share one address
  behind NAT; admins can lift bans there.
- The web UI should only be reachable via HTTPS (Caddy) from outside; turn on
  two-factor login for admins.
- The containers run without root, without Linux capabilities and – server and
  media worker – with read-only file systems.
- Monitoring: see [Monitoring](monitoring.md).

## Troubleshooting

```sh
docker compose logs -f talkops freeswitch
docker compose exec freeswitch fs_cli -p "$TALKOPS_ESL_PASSWORD" -x status
```

### Calls are missing from the call log

FreeSWITCH sends the record of a call to TalkOps when the call ends.
If TalkOps does not accept the record, FreeSWITCH tries again for about
one minute. Then FreeSWITCH writes the record to the `cdr-spool` volume.
TalkOps imports the records from this volume every minute. A call
recording appears in the call log together with its record.

To find the cause, examine the logs:

```sh
docker compose logs talkops | grep -iE "call record|CDR|event socket"
docker compose exec freeswitch ls -la /var/lib/talkops/cdr-spool
```

- `call record stored`: TalkOps stored the record.
- `cannot store CDR`: the database did not accept the record.
  FreeSWITCH spools it.
- `malformed CDR`: TalkOps cannot read the record. After the import,
  the file is in `cdr-spool/rejected/`. Keep the file for an error report.
- `lost connection to FreeSWITCH event socket`: TalkOps cannot see calls
  in progress until the connection is available again.

Versions before the `cdr-spool` volume kept undelivered records inside
the FreeSWITCH container. A new container does not have them. To keep
these records, do these steps before the update:

1. Copy the records out of the old container:
   `docker compose cp freeswitch:/usr/local/freeswitch/var/log/freeswitch/xml_cdr ./xml_cdr-old`
2. Update TalkOps.
3. Copy the records into the spool:
   `docker compose cp ./xml_cdr-old/. freeswitch:/var/lib/talkops/cdr-spool/`

TalkOps imports the records within one minute. Records that TalkOps has
already stored are not stored again.

## Check the result

Check container health, first login and the first internal call.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
