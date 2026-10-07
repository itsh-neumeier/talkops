# Installation

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

```sh
mkdir talkops && cd talkops
curl -LO https://raw.githubusercontent.com/itsh-neumeier/talkops/main/docker-compose.yml
curl -L -o .env https://raw.githubusercontent.com/itsh-neumeier/talkops/main/.env.example
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

```sh
docker compose pull && docker compose up -d
```

Database migrations run automatically when `talkops` starts. Pin a version by
setting `TALKOPS_VERSION` in `.env` (e.g. `1.0`). Make a
[backup](backup.md) before major updates.

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
