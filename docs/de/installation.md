# Installation

> Stand Phase 1: Nebenstellen, Geräte, SIP-Trunks (Provider-Vorlagen),
> interne/externe Gespräche und Anrufliste. Danach weiter mit
> [Erste Schritte](erste-schritte.md).

## Voraussetzungen

- Linux-Host (x86_64 oder arm64), z. B. Debian/Ubuntu, Proxmox-VM, NAS mit Docker
- Docker Engine ≥ 24 mit Compose-Plugin
- Freie Ports auf dem Host (TalkOps nutzt Host-Networking):

| Port | Protokoll | Zweck |
|---|---|---|
| 5060 | UDP/TCP | SIP für Telefone |
| 5080 | UDP/TCP | SIP für Trunks (Provider) |
| 16384–16999 | UDP | RTP (Sprache/Video), konfigurierbar |
| 8080 | TCP | Web-UI / API |
| 8021, 5432 | TCP | nur lokal (127.0.0.1): FreeSWITCH-ESL, PostgreSQL |

Docker Desktop (macOS/Windows) wird nicht unterstützt, da Host-Networking
dort nicht vollständig funktioniert.

## Installation mit Docker Compose

```sh
mkdir talkops && cd talkops
curl -LO https://raw.githubusercontent.com/itsh-neumeier/talkops/main/docker-compose.yml
curl -L -o .env https://raw.githubusercontent.com/itsh-neumeier/talkops/main/.env.example
```

In `.env` die Pflichtwerte setzen, z. B.:

```sh
sed -i "s/^POSTGRES_PASSWORD=$/POSTGRES_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_SECRET_KEY=$/TALKOPS_SECRET_KEY=$(openssl rand -hex 32)/" .env
sed -i "s/^TALKOPS_ESL_PASSWORD=$/TALKOPS_ESL_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_XMLCURL_PASSWORD=$/TALKOPS_XMLCURL_PASSWORD=$(openssl rand -hex 16)/" .env
```

> **Wichtig:** `TALKOPS_SECRET_KEY` verschlüsselt die gespeicherten SIP- und
> Trunk-Zugangsdaten. Sichern Sie den Wert getrennt von der Datenbank – ohne
> ihn sind die Zugangsdaten nach einem Restore nicht mehr lesbar.

Starten:

```sh
docker compose up -d
docker compose ps        # alle Dienste sollten "healthy" sein
```

Das Web-UI ist unter `http://<host>:8080` erreichbar, der Komponentenstatus
unter `http://<host>:8080/api/v1/status`, die API-Beschreibung (OpenAPI) unter
`http://<host>:8080/api/v1/openapi.json`. Weiter mit [Erste Schritte](erste-schritte.md).

## HTTPS mit Caddy (optional)

Für ein öffentlich erreichbares Web-UI mit Let's-Encrypt-Zertifikat in `.env`:

```sh
COMPOSE_PROFILES=caddy
TALKOPS_DOMAIN=pbx.example.com
```

Caddy belegt die Ports 80 und 443 des Hosts.

## Updates

```sh
docker compose pull && docker compose up -d
```

Datenbankmigrationen laufen beim Start von `talkops` automatisch. Für feste
Versionen `TALKOPS_VERSION` in `.env` setzen (z. B. `0.1`).

## Alternative: macvlan

Ist Port 5060 auf dem Host belegt (z. B. durch eine andere Anlage), kann
FreeSWITCH eine eigene IP im LAN bekommen. Dann müssen `talkops` und
`freeswitch` sich gegenseitig erreichen; dafür `TALKOPS_ESL_ADDR`,
`TALKOPS_XMLCURL_URL` und `TALKOPS_ESL_LISTEN_IP` auf die jeweiligen IPs setzen
und das ESL-Passwort besonders stark wählen. Eine fertige Beispielkonfiguration
folgt in Phase 8.

## Fehlersuche

```sh
docker compose logs -f talkops freeswitch
docker compose exec freeswitch fs_cli -p "$TALKOPS_ESL_PASSWORD" -x status
```
