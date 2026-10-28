# Installation

[Handbuch](index.md) · [English](../en/installation.md)

## Voraussetzungen

Du benötigst: Host-Administration und eine neue Installation oder einen Wiederherstellungsplan.

Für die Wiederherstellung auf einem neuen Host zuerst die gesicherte Konfiguration
exportieren, bevor neue Zugangsdaten erzeugt werden. Siehe
[Datensicherung und Wiederherstellung](datensicherung.md). Die Ersteinrichtung
bietet ebenfalls einen Wiederherstellungsassistenten.

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
curl -fLO https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.13/docker-compose.yml
curl -fL -o .env https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.13/.env.example
sed -i 's/^TALKOPS_VERSION=.*/TALKOPS_VERSION=1.8.13/' .env
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

Dateien und Images nutzen den veröffentlichten Stand 1.8.13. Wähle für spätere
Releases passende Konfiguration und Image-Version gemeinsam. `latest` ist ein
beweglicher stabiler Image-Tag und legt die Konfigurationsdateien nicht fest.
Schütze `.env` nach Erzeugung der Geheimnisse mit `chmod 600 .env`.
Für vollständige Konfigurationsbackups gilt stattdessen die Gruppenlese-
Einrichtung unter [Datensicherung](datensicherung.md).

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

Folge der [Aktualisierungsanleitung](aktualisierung.md). Erstelle vorher ein
Backup und prüfe Release-Hinweise, Migrationen und Mount-Änderungen.
Erhalte alle aktiven Compose-Optionen. Migrationen laufen beim Start.
Ein FreeSWITCH-Neustart beendet Gespräche. Ein reines Image-Downgrade ersetzt
keine kompatible Wiederherstellung.

## Alternative: macvlan

Ist Port 5060 auf dem Host belegt (z. B. durch eine andere Anlage), kann
FreeSWITCH eine eigene IP im LAN bekommen. Dann müssen `talkops` und
`freeswitch` sich gegenseitig erreichen; dafür `TALKOPS_ESL_ADDR`,
`TALKOPS_XMLCURL_URL` und `TALKOPS_ESL_LISTEN_IP` auf die jeweiligen IPs setzen,
die Adresse von FreeSWITCH mit `TALKOPS_FS_PEERS` freigeben (sonst beantwortet
TalkOps Konfigurationsabfragen nur vom selben Host) und das ESL-Passwort
besonders stark wählen. Dieser Aufbau gehört nicht zur getesteten
Standardinstallation.

## Sicherheit

- **SIP aus dem Internet:** Port 5060 nur öffnen, wenn externe Telefone ihn
  brauchen; ein VPN (WireGuard, Tailscale) ist sicherer. Trunks registrieren
  sich ausgehend und brauchen bei den meisten Anbietern keine Portweiterleitung.
- **SIP-Anmeldeschutz** (*Einstellungen → SIP-Anmeldeschutz*): Eine Adresse mit
  10 fehlgeschlagenen Anmeldungen (falsches Passwort oder unbekannter
  Benutzer) innerhalb von 10 Minuten wird für eine Stunde gesperrt – jede
  Anmeldung von dort scheitert, auch mit dem richtigen Passwort. Das eigene LAN
  als vertrauenswürdiges Netz eintragen, wenn sich viele Telefone hinter NAT
  eine Adresse teilen; Admins können Sperren dort aufheben.
- Das Web-UI von außen nur per HTTPS (Caddy) erreichbar machen und für Admins
  die Zwei-Faktor-Anmeldung einschalten.
- Die Container laufen ohne root, ohne Linux-Capabilities und – Server und
  Media-Worker – mit schreibgeschütztem Dateisystem.
- Überwachung: siehe [Monitoring](monitoring.md).

## Fehlersuche

```sh
docker compose logs -f talkops freeswitch
docker compose exec freeswitch fs_cli -p "$TALKOPS_ESL_PASSWORD" -x status
```

### Anrufe fehlen in der Anrufliste

FreeSWITCH sendet den Datensatz eines Anrufs am Gesprächsende an TalkOps.
Nimmt TalkOps ihn nicht an, versucht FreeSWITCH es etwa eine Minute lang
erneut und legt den Datensatz dann im Volume `cdr-spool` ab. TalkOps liest
dieses Volume jede Minute ein. Eine Gesprächsaufnahme erscheint zusammen mit
ihrem Datensatz in der Anrufliste.

Ursache in den Logs suchen:

```sh
docker compose logs talkops | grep -iE "call record|CDR|event socket"
docker compose exec freeswitch ls -la /var/lib/talkops/cdr-spool
```

- `call record stored`: TalkOps hat den Datensatz gespeichert.
- `cannot store CDR`: Die Datenbank hat den Datensatz nicht angenommen.
  FreeSWITCH legt ihn im Spool ab.
- `malformed CDR`: TalkOps kann den Datensatz nicht lesen. Nach dem Einlesen
  liegt die Datei in `cdr-spool/rejected/`. Für eine Fehlermeldung aufheben.
- `lost connection to FreeSWITCH event socket`: TalkOps sieht laufende
  Gespräche erst wieder, wenn die Verbindung steht.

Versionen vor dem Volume `cdr-spool` haben nicht zugestellte Datensätze im
FreeSWITCH-Container abgelegt. Ein neuer Container hat sie nicht mehr. Um
diese Datensätze zu behalten, **vor** dem Update:

1. Datensätze aus dem alten Container kopieren:
   `docker compose cp freeswitch:/usr/local/freeswitch/var/log/freeswitch/xml_cdr ./xml_cdr-alt`
2. TalkOps aktualisieren.
3. Datensätze in den Spool kopieren:
   `docker compose cp ./xml_cdr-alt/. freeswitch:/var/lib/talkops/cdr-spool/`

TalkOps liest sie innerhalb einer Minute ein. Bereits gespeicherte Anrufe
werden nicht doppelt angelegt.

## Ergebnis prüfen

Prüfe Containerzustand, erste Anmeldung und den ersten internen Anruf.

Bei Fehlern: [Fehlerbehebung](fehlerbehebung.md). Erhalte bisherige Daten und Konfiguration während der Diagnose.
