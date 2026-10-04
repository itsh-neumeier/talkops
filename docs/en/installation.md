# Installation

> Phase 0 status: the stack starts and every service becomes healthy;
> telephony features arrive from phase 1 on.

## Requirements

- Linux host (x86_64 or arm64), e.g. Debian/Ubuntu, Proxmox VM, NAS with Docker
- Docker Engine ≥ 24 with the Compose plugin
- Free ports on the host (TalkOps uses host networking):

| Port | Protocol | Purpose |
|---|---|---|
| 5060, 5061, 5080 | UDP/TCP | SIP (from phase 1) |
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
`http://<host>:8080/api/v1/status`.

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
setting `TALKOPS_VERSION` in `.env` (e.g. `0.1`).

## Alternative: macvlan

If port 5060 is already used on the host (e.g. by another PBX), FreeSWITCH can
get its own LAN IP. `talkops` and `freeswitch` must then reach each other: set
`TALKOPS_ESL_ADDR`, `TALKOPS_XMLCURL_URL` and `TALKOPS_ESL_LISTEN_IP` to the
respective IPs and use a strong ESL password. A ready-made example follows in
phase 1.

## Troubleshooting

```sh
docker compose logs -f talkops freeswitch
docker compose exec freeswitch fs_cli -p "$TALKOPS_ESL_PASSWORD" -x status
```
