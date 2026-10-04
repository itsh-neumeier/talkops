# 0007 – Deployment: Docker Compose mit Host-Networking

- Status: Angenommen
- Datum: 2026-10-04

## Kontext

Installationen laufen auf einem Linux-Host (NUC, NAS, VM, Proxmox-LXC), oft
verwaltet über **Portainer**. SIP und RTP vertragen Docker-NAT schlecht:
RTP braucht hunderte UDP-Ports, SIP-Header enthalten IP-Adressen, und das
Mapping großer Portbereiche über `docker-proxy` ist langsam bzw. fehleranfällig.

## Optionen

1. **Host-Networking** für FreeSWITCH (und die eng gekoppelten Dienste).
2. **Bridge + veröffentlichte Ports** – RTP-Bereich mappen, `ext-rtp-ip`
   setzen; fehleranfällig, langsamer Start, schlecht für SIP-ALG-freie Setups.
3. **macvlan** – Container bekommt eigene IP im LAN; sauber, aber der Host
   selbst erreicht den Container ohne Zusatzkonfiguration nicht.
4. **Kubernetes/Helm** – für die Zielgruppe überdimensioniert.

## Entscheidung

- **Docker Compose**, Images auf GHCR (`ghcr.io/itsh-neumeier/talkops-server`,
  `-freeswitch`, `-media-worker`), Multi-Arch amd64 + arm64.
- `freeswitch`, `talkops`, `media-worker` (und optional `caddy`) laufen mit
  **`network_mode: host`**. Dadurch bleiben ESL (`127.0.0.1:8021`) und das
  xml_curl-Ziel (`127.0.0.1:8080`) auf Loopback, ohne gemeinsames
  Docker-Netz. Postgres läuft im Bridge-Netz und ist nur auf
  `127.0.0.1:${POSTGRES_PORT}` veröffentlicht.
- `docker-compose.yml` enthält **nur** fertige Images (`image:`), keine
  `build:`-Abschnitte, und ist damit direkt als Portainer-Stack nutzbar.
  Lokale Builds über das Override `docker-compose.dev.yml`.
- Alle Images laufen als UID/GID 10001; gemeinsame Volumes für Aufnahmen,
  Voicemail und Ansagen.
- **macvlan** wird als Alternative dokumentiert (z. B. wenn Port 5060 auf dem
  Host belegt ist).

## Konsequenzen

- Nur Linux-Hosts (Docker Desktop auf macOS/Windows unterstützt
  Host-Networking nicht vollständig) – für eine Telefonanlage akzeptabel.
- Ports der Dienste dürfen auf dem Host nicht belegt sein (Doku listet sie).
- Die Firewall des Hosts schützt die Dienste; Doku enthält Beispielregeln.
