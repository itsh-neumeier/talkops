# Referenz und Glossar

[Handbuch](index.md) · [English](../en/reference.md)

Diese Seite verlinkt Konfigurationsreferenzen. Deployment-Variablen ändern
Administratoren; normale Benutzer benötigen keinen Docker- oder Datenbankzugriff.

## Netzwerkvorgaben

| Port | Protokoll | Zweck |
| --- | --- | --- |
| 5060 | UDP/TCP | SIP-Registrierung der Telefone. |
| 5080 | UDP/TCP | SIP-Profil für Provider. |
| 16384–16999 | UDP | RTP-Audio-/Videomedien von FreeSWITCH. |
| 8080 | TCP | Weboberfläche und HTTP-API. |
| 8021 | TCP, localhost | FreeSWITCH-Event-Socket. |
| 5432 | TCP, localhost | PostgreSQL. |
| 80, 443 | TCP, optional | Caddy-HTTPS und Zertifikatseinrichtung. |
| 5066 | TCP, localhost | FreeSWITCH-WebSocket hinter dem angemeldeten TalkOps-WebSocket-Proxy. |
| 3478 | UDP/TCP, optional | coturn-Listener; Relay-Einrichtung siehe Softphone-Anleitung. |

Diese Vorgaben sind keine Liste pauschaler Firewallfreigaben. Host-Netzwerk,
NAT und Proxy hängen vom Deployment ab. Datenbank und Event-Socket bleiben lokal.
Siehe [Installation](installation.md) und [Softphone-Netzwerk](softphone.md).

## Umgebung und Deployment-Dateien

[.env.example](../../.env.example) und [docker-compose.yml](../../docker-compose.yml)
sind die Deployment-Referenzen. Backup- und Mount-Optionen stehen in
[docker-compose.backup.yml](../../docker-compose.backup.yml) und
[docker-compose.targets.yml](../../docker-compose.targets.yml).
Nutze Dateien aus demselben Release wie die Images.

| Variablengruppe | Wichtige Werte |
| --- | --- |
| Image-Version | `TALKOPS_VERSION`; Image-Tag wie `1.8.20`, nicht Git-Tag `v1.8.20`. |
| Datenbank | `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB`, `POSTGRES_PORT`. |
| Pflichtgeheimnisse | `TALKOPS_SECRET_KEY` (64 Hexadezimalzeichen), `TALKOPS_ESL_PASSWORD`, `TALKOPS_XMLCURL_PASSWORD`. |
| Web- und SIP-Ports | `TALKOPS_LISTEN`, `TALKOPS_HTTP_PORT`, `TALKOPS_ESL_PORT`, `TALKOPS_SIP_PORT`, `TALKOPS_SIP_TRUNK_PORT`, `TALKOPS_RTP_START_PORT`, `TALKOPS_RTP_END_PORT`. |
| HTTPS | `COMPOSE_PROFILES=caddy`, `TALKOPS_DOMAIN`. Bei zusätzlichem TURN aktive Profile kombinieren. |
| Browser-Relay | `TALKOPS_TURN_SECRET`, `TALKOPS_TURN_URLS`, `TALKOPS_TURN_PEER_IP`, `TALKOPS_TURN_RELAY_ONLY`; optionales Profil `turn`. |
| Transkription | `TALKOPS_WHISPER_MODEL`, `TALKOPS_WHISPER_VAD`; Einstellungen je Zweck in der Oberfläche. |
| Betrieb | `TZ`, `TALKOPS_LOG_FORMAT`, `RUST_LOG`, `TALKOPS_FS_LOG_LEVEL`, `TALKOPS_METRICS_TOKEN`. |
| Wiederherstellung | `TALKOPS_SETUP_TOKEN`, `TALKOPS_DEPLOYMENT_FILES`, Deployment-/Passwortpfade und Zielwurzeln; siehe [Datensicherung](datensicherung.md). |

Halte `TALKOPS_HTTP_PORT` gleich dem Port in `TALKOPS_LISTEN`. Zugangsdaten
gehören nicht in Issues. Speichere das Backup-Passwort nie in erfassten `.env`-Dateien.
Direkte Server-/Worker-Variablen stehen zusätzlich in der
[Serverkonfiguration](../../crates/talkops-api/src/config.rs).

## Wahl, Rollen und API

- [Erste Schritte](erste-schritte.md#wählen) enthält interne, externe und SIP-Wahl.
- [Funktionscodes](yealink.md#4-kurzwahlen-am-telefon) enthält Nicht stören, Weiterleitung, Heranholen und Parken.
- [Benutzer und Rollen](benutzer-und-rollen.md#berechtigungen) enthält Berechtigungsschlüssel.
- Der laufende Server liefert OpenAPI unter `/api/v1/openapi.json` und Status
  unter `/api/v1/status`. Nutze konfigurierten Host und Port. API-Aufrufe
  benötigen weiterhin Anmeldung und passende Rechte; eine Beschreibung gewährt
  keinen Zugriff.

## Glossar

| Begriff | Bedeutung |
| --- | --- |
| Nebenstelle | Interne Nummer, Eigentümer und Anrufeinstellungen. |
| Gerät | SIP-Identität einer Nebenstelle. |
| Telefon | Provisionierte Hardware oder App-Profil mit gegebenenfalls mehreren Konten. |
| Trunk / Konto | Providerverbindung / Registrierungsdaten innerhalb dieser Verbindung. |
| DID / E.164 | Eingehende Providernummer / internationales Nummernformat wie `+49891234567`. |
| SIP / RTP | Anrufsignalisierung / Medienübertragung. |
| Registrar | Server für SIP-Registrierungen. |
| BLF / MWI | Besetztlampenfeld / Anzeige wartender Nachrichten. |
| IVR / Smart Attendant | Menü oder Anrufablauf für Anruferauswahl. |
| DND / CLIR | Nicht stören / Rufnummer unterdrücken. |
| WebRTC / WSS / TURN | Browsertelefonie / sicherer WebSocket / Medien-Relay. |
| TTS / Transkription | Text zu Sprache / Sprache zu Text. |
| CDR / Spool | Abgeschlossener Anrufdatensatz / dauerhafte Warteschlange noch nicht gespeicherter Datensätze. |
| Migration | Versionierte Datenbankschemaänderung. |

Nutze bei Fehlern die [Fehlerbehebung](fehlerbehebung.md) und deren
Supportbericht-Checkliste. Erwartete Konfiguration belegt keinen realen
Provider- oder Hardwaretest.
