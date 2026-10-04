# TalkOps – Architektur

> Status: Phase 0 (Fundament). Entscheidungen mit Begründung stehen in den
> [ADRs](adr/README.md); dieses Dokument beschreibt das Zusammenspiel.

## Überblick

```
                 SIP/RTP (UDP/TCP/TLS, SRTP)            SIP over WSS + DTLS-SRTP
 Trunks ─────────┐   Yealink / DECT / VTO ──┐            Browser-Softphone ──┐
                 ▼                          ▼                                ▼
          ┌──────────────────────────────────────────────────────────────────────┐
          │ freeswitch  (host network, „dumm“)                                    │
          │  mod_sofia · mod_rtc · mod_conference · mod_callcenter · mod_spandsp │
          │  mod_xml_curl ──────────┐                   mod_event_socket :8021    │
          └─────────────────────────┼───────────────────────────▲────────────────┘
                 HTTP POST /fs/xml  │ (Directory, Dialplan,     │ ESL (inbound: Events, API)
                 127.0.0.1:8080     │  Konfiguration)           │ 127.0.0.1 only
          ┌─────────────────────────▼───────────────────────────┴────────────────┐
          │ talkops  (Rust: axum, sqlx, tokio)                                   │
          │  REST/WebSocket-API · Web-UI · Provisioning · Webhooks · Auth        │
          └───────────────┬──────────────────────────────────────▲───────────────┘
                          │ SQL                                  │ Jobs (LISTEN/NOTIFY)
          ┌───────────────▼────────────┐         ┌───────────────┴───────────────┐
          │ postgres (127.0.0.1:5432)  │◀────────│ media-worker                   │
          │  Konfiguration, CDR, Jobs  │   SQL   │  Piper (TTS), whisper.cpp      │
          └────────────────────────────┘         └───────────────────────────────┘
          Gemeinsame Volumes: recordings · voicemail · sounds (UID/GID 10001)
```

### Komponenten

| Dienst | Aufgabe | Technik |
|---|---|---|
| `freeswitch` | SIP-Registrar/Proxy-Endpunkt, Medien (RTP, Video-Passthrough, WebRTC), Aufzeichnung, Konferenz, Queues, Fax | FreeSWITCH 1.10.12, aus Quellcode, gepinnt ([ADR 0002](adr/0002-telefonie-engine-freeswitch.md)) |
| `talkops` | Control Plane: liefert FreeSWITCH-Konfiguration aus der DB, steuert Anrufe per ESL, REST-API, Web-UI, Provisioning, Auth | Rust: tokio, axum, sqlx, utoipa |
| `postgres` | Einzige Quelle der Wahrheit: Konfiguration, CDR, Job-Queue, Audit-Log | PostgreSQL 17 ([ADR 0003](adr/0003-datenbank-postgresql.md)) |
| `media-worker` | Asynchrone Jobs: TTS (Piper), Transkription (whisper.cpp) | Rust, Postgres-Job-Queue ([ADR 0005](adr/0005-job-queue-postgres.md)) |
| `caddy` (optional) | HTTPS mit ACME vor dem Web-UI | Caddy 2 |

### Rust-Workspace

| Crate | Inhalt |
|---|---|
| `talkops-core` | Domänentypen, DB-Pool, Migrationen (`migrations/`), Job-Queue, Logging |
| `talkops-api` | HTTP-Server, `/fs/xml` (mod_xml_curl), OpenAPI, Binary `talkops` |
| `talkops-esl` | Async-Client für das FreeSWITCH Event Socket Protocol |
| `talkops-provisioning` | Yealink-Provisioning, XML-Telefonbuch, Firmware (Phase 2) |
| `talkops-media-worker` | Worker-Binary für TTS/Transkription |
| `talkops-doorbell` | Dahua-VTO: Türöffner, Snapshots, Home-Assistant-Events (Phase 6) |

Abhängigkeitsrichtung: `api`, `media-worker` → `core`, `esl`, `provisioning`,
`doorbell` → `core`. Fachlogik liegt in `core` bzw. den Fach-Crates, nicht im
HTTP-Layer.

## Steuerung von FreeSWITCH ([ADR 0006](adr/0006-steuerung-xml-curl-und-esl.md))

1. **Bootstrap**: Das FreeSWITCH-Image enthält nur `docker/freeswitch/conf`
   (Module, Event Socket, xml_curl-Binding, RTP-Ports). Werte kommen beim
   Containerstart aus Umgebungsvariablen (`docker-entrypoint.sh` → `vars_env.xml`).
2. **mod_xml_curl**: Für jede Lookup-Anfrage (`directory`, `dialplan`,
   `configuration`) sendet FreeSWITCH einen HTTP-POST an
   `http://127.0.0.1:8080/fs/xml` (Basic-Auth). TalkOps erzeugt das XML aus der
   DB oder antwortet `not found` → FreeSWITCH nutzt die statische Bootstrap-Datei.
   - `configuration`: `sofia.conf` (SIP-Profile, Gateways/Trunks), `acl.conf`,
     `callcenter.conf`, `conference.conf` …
   - `directory`: User/Geräte bei REGISTER/INVITE-Authentifizierung
   - `dialplan`: pro Anruf eine generierte Extension (Routing-Entscheidung in Rust)
3. **ESL inbound** (`127.0.0.1:8021`): TalkOps hält eine dauerhafte Verbindung
   (automatischer Reconnect), abonniert Events (Channel-Lifecycle, Registrierung,
   Presence, CDR-relevante Events) und führt API-Kommandos aus (`uuid_transfer`,
   `uuid_record`, `sofia profile … rescan`, NOTIFY für Resync/MWI).
4. **ESL outbound** (ab Phase 3): Interaktive Abläufe (Voicemail, IVR,
   Türöffner-DTMF) über die Dialplan-App `socket` → TalkOps lauscht auf
   `127.0.0.1:8084` und steuert den einzelnen Call.
5. **CDR-Sicherheitsnetz**: `mod_xml_cdr` postet CDRs zusätzlich per HTTP, damit
   bei ESL-Unterbrechung keine Datensätze verloren gehen (Phase 1).

Startreihenfolge: `postgres` → `talkops` (Migrationen, `/readyz`) → `freeswitch`.
FreeSWITCH gehört bewusst **nicht** zur Readiness von TalkOps (zirkuläre
Abhängigkeit); sein Zustand steht unter `/api/v1/status`.

## Netzwerk & Ports ([ADR 0007](adr/0007-deployment-und-netzwerk.md))

`freeswitch`, `talkops`, `media-worker` und `caddy` laufen mit
`network_mode: host`; Postgres ist nur auf `127.0.0.1` veröffentlicht.

| Port | Proto | Dienst | Erreichbar von |
|---|---|---|---|
| 5060 | UDP/TCP | SIP (intern, Profil `internal`, ab Phase 1) | LAN |
| 5061 | TCP | SIP/TLS (ab Phase 1) | LAN/WAN |
| 5080 | UDP/TCP | SIP-Trunks (Profil `external`, ab Phase 1) | Provider |
| 7443 | TCP | SIP over WSS (WebRTC, Phase 7) | Browser |
| 16384–16999 | UDP | RTP/SRTP (konfigurierbar) | LAN/Provider |
| 8080 | TCP | Web-UI, API, Provisioning, `/fs/xml` | LAN bzw. via Caddy |
| 8021 | TCP | ESL inbound | nur 127.0.0.1 |
| 5432 | TCP | PostgreSQL | nur 127.0.0.1 |
| 80/443 | TCP | Caddy (optional) | LAN/WAN |

`/fs/xml` ist per Basic-Auth geschützt; ab Phase 8 wird es zusätzlich auf
Loopback-Quellen beschränkt.

## Daten & Volumes

| Volume | Pfad | Nutzer |
|---|---|---|
| `postgres` | `/var/lib/postgresql/data` | postgres |
| `recordings` | `/var/lib/talkops/recordings` | freeswitch (schreibt), talkops, media-worker |
| `voicemail` | `/var/lib/talkops/voicemail` | freeswitch, talkops, media-worker |
| `sounds` | `/var/lib/talkops/sounds` | TTS-Ansagen, MoH-Uploads |
| `provisioning` | `/var/lib/talkops/provisioning` | Firmware, generierte Configs |
| `models` | `/var/lib/talkops/models` | Whisper-/Piper-Modelle |
| `freeswitch-db` | `/usr/local/freeswitch/var/lib/freeswitch/db` | FreeSWITCH-interne SQLite (Registrierungen) |

Alle Images nutzen UID/GID **10001**, damit die gemeinsamen Volumes ohne
`chmod 777` beschreibbar sind.

## Sicherheit

- Web-Login-Passwörter: argon2id. SIP-Passwörter sind separat und werden – weil
  FreeSWITCH (Digest) und Yealink-Provisioning sie im Klartext brauchen –
  **verschlüsselt** gespeichert (XChaCha20-Poly1305, Schlüssel `TALKOPS_SECRET_KEY`,
  [ADR 0008](adr/0008-secrets-at-rest.md)).
- ESL und Postgres nur auf Loopback; xml_curl mit Basic-Auth.
- Container laufen nicht als root.
- Ab Phase 1: Audit-Log für Admin-Aktionen, CSRF-Schutz, Session-Cookies
  (`HttpOnly`, `SameSite=Strict`). Ab Phase 8: Rate-Limits/IP-Sperren gegen
  SIP-Scanner, Länder-/IP-Allowlists.

## Healthchecks & Betrieb

| Dienst | Healthcheck |
|---|---|
| `talkops` | `talkops healthcheck` → `GET /readyz` (DB erreichbar) |
| `freeswitch` | `fs-healthcheck.sh` → `fs_cli -x status` meldet `UP` |
| `media-worker` | Heartbeat-Datei, nach jedem Queue-Durchlauf aktualisiert |
| `postgres` | `pg_isready` |

Endpunkte: `/healthz` (Liveness), `/readyz` (Readiness), `/api/v1/status`
(Komponentenstatus), `/api/v1/openapi.json` (OpenAPI 3.1). Logs strukturiert
als JSON auf stdout (`TALKOPS_LOG_FORMAT=json`). Prometheus-Metriken folgen in
Phase 8.

## Verzeichnisstruktur

Ist-Stand Phase 0 plus (kursiv in den Kommentaren) geplante Ergänzungen der
folgenden Phasen:

```
talkops/
├── Cargo.toml                    Workspace
├── CLAUDE.md                     Leitfaden für Beiträge
├── README.md · LICENSE (AGPL-3.0)
├── docker-compose.yml            nur fertige GHCR-Images (Portainer-tauglich)
├── docker-compose.dev.yml        Override mit lokalen Builds
├── .env.example
├── crates/
│   ├── talkops-core/
│   │   └── src/  db.rs · jobs.rs · tenant.rs · telemetry.rs
│   │             (P1: users.rs, extensions.rs, devices.rs, trunks.rs, numbers.rs,
│   │              cdr.rs, crypto.rs, audit.rs, presets.rs, dialrules.rs)
│   ├── talkops-api/
│   │   └── src/  main.rs · config.rs · esl.rs · routes/{health,fs_xml}.rs
│   │             (P1: auth/, routes/{users,extensions,trunks,numbers,cdr}.rs,
│   │              fsxml/{directory,dialplan,sofia}.rs mit XML-Templates)
│   ├── talkops-esl/              Event-Socket-Client (inbound; P3: outbound-Server)
│   ├── talkops-provisioning/     P2: Yealink-Templates, Telefonbuch, Firmware
│   ├── talkops-media-worker/     P3: Piper-Handler, P5: Whisper-Handler
│   └── talkops-doorbell/         P6: Dahua-HTTP-API/CGI, MQTT/Webhooks
├── migrations/                   sqlx-Migrationen (ein Satz für alle Dienste)
├── presets/
│   └── trunks/                   P1: leonet.yaml, telekom-*.yaml, sipgate-*.yaml …
├── web/                          SvelteKit + TS + Tailwind (SPA)
│   └── src/  lib/{api.ts, i18n/, theme.svelte.ts} · routes/
├── docker/
│   ├── freeswitch/               Dockerfile, build-modules.conf, conf/ (Bootstrap)
│   ├── talkops/                  Dockerfile (Rust + Web-UI)
│   └── media-worker/             Dockerfile (P3/P5: Piper, whisper.cpp, CUDA-Variante)
├── tests/                        P1: SIPp-Szenarien + Compose-Integrationstests
│   └── sipp/                     register.xml, call.xml, voicemail.xml, transfer.xml
├── docs/
│   ├── architecture.md · adr/
│   ├── de/  installation.md · portainer.md (P1+: leonet.md, yealink.md, dahua.md,
│   │                                         ldap.md, backup.md)
│   ├── en/  (gleiche Struktur auf Englisch)
│   └── trunk-presets.md          P1: Format & Beitragsregeln für Vorlagen
└── .github/workflows/            ci.yml · freeswitch.yml · release.yml
```

## Roadmap

| Phase | Inhalt | Status |
|---|---|---|
| 0 | Fundament: Workspace, ADRs, CI, Compose, FreeSWITCH-Image, Postgres, Healthchecks | ✅ |
| 1 | Grundtelefonie: xml_curl-Directory/Dialplan, Nebenstellen, Trunk-Presets (LEONET u. a.), Web-UI Login/User/Trunks, CDR | geplant |
| 2 | Yealink: Provisioning, Templates, XML-Telefonbuch, BLF, Feature-Codes, MWI | geplant |
| 3 | Voicemail & TTS: ESL-Voicemail, Piper, Mehrsprachigkeit, Mail | geplant |
| 4 | Gruppen & Logik: Rufgruppen, Queues, IVR-Editor, Zeitsteuerung/Feiertage, Parken/Pickup | geplant |
| 5 | Recording & Transkription: Hinweisansage, Whisper, Suche, Retention | geplant |
| 6 | Türsprechstelle: Dahua VTO, Video, Türöffner, Snapshots, Home Assistant | geplant |
| 7 | WebRTC & Identität: Softphone mit Video, LDAP/AD, OIDC, 2FA | geplant |
| 8 | Betrieb: Backup/Restore, Metriken, Hardening, Setup-Assistent, Release 1.0 | geplant |
