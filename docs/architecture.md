# TalkOps – Architektur

> Status: Phase 7 (WebRTC & Identität). Entscheidungen mit Begründung stehen in den
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
| `freeswitch` | SIP-Registrar/Proxy-Endpunkt, Medien (RTP, Video-Passthrough, WebRTC), Aufzeichnung, Konferenz, Queues, Fax | FreeSWITCH 1.10.12 auf Debian 13, aus Quellcode, gepinnt ([ADR 0002](adr/0002-telefonie-engine-freeswitch.md), [ADR 0009](adr/0009-basis-images-debian-trixie.md)) |
| `talkops` | Control Plane: liefert FreeSWITCH-Konfiguration aus der DB, steuert Anrufe per ESL, REST-API, Web-UI, Provisioning, Auth | Rust: tokio, axum, sqlx, utoipa |
| `postgres` | Einzige Quelle der Wahrheit: Konfiguration, CDR, Job-Queue, Audit-Log | PostgreSQL 17, `postgres:17-trixie` ([ADR 0003](adr/0003-datenbank-postgresql.md)) |
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

## Anruf-Routing (Phase 1)

Jede Anfrage im Kontext `internal` (authentifizierte Geräte) bzw. `public`
(Trunks) beantwortet `fsxml::dialplan` mit genau einer generierten Extension:

| Kontext | Ziel | Aktion |
|---|---|---|
| internal | Nebenstelle | `bridge` an alle aktiven Geräte (`user/<sip-user>@talkops.local`, gleichzeitig), `call_timeout` = Klingeldauer |
| internal | Notruf (110/112, konfigurierbar) | unverändert über den Trunk der **Standardrufnummer**, nie mit CLIR; Absender = eigene Nummer, falls auf demselben Trunk |
| internal | Sonderrufnummer (`11x`) | unverändert über die Rufnummer der Nebenstelle bzw. Standardrufnummer |
| internal | extern | Wählregeln → E.164 → Format der Vorlage (`number_format`), Absender im Format `caller_id_format` per From/PAI/PPI, optional CLIR (`privacy full`) |
| public | eigene Rufnummer | Normalisierung (+49…, 0049…, 49…, 0…) → Rufnummer → Ziel-Nebenstelle; Anrufernummer national formatiert (Rückruf ohne Umweg) |
| public | unbekannt | `404` |
| internal | `*78` / `*79` | Nicht stören an/aus (Bestätigungston, kein CDR) |
| internal | `*72<Nummer>` / `*73` | Rufumleitung sofort an/aus (Ziel: Nebenstelle oder externe Nummer) |
| internal | `**<Nebenstelle>` | gezieltes Heranholen (`pickup ext-<uuid>`) |
| internal | `*97` / `*98` | eigene Mailbox / beliebige Mailbox mit PIN (ESL outbound) |
| internal | `*30<Nummer>` | Zeitsteuerung zwischen Automatik und „geschlossen“ umschalten |
| internal | `*51` … `*59` | Parkplatz: parken bzw. abholen (`valet_park talkops *5N`) |

Beim Klingeln einer Nebenstelle gilt (Phase 2): deaktiviert → `480`,
**Nicht stören** → `486 Busy Here`, **Rufumleitung** → genau ein Sprung (keine
Ketten, keine Schleifen) zu einer Nebenstelle bzw. extern über die Rufnummer
der umleitenden Nebenstelle (sonst Standardrufnummer). Jeder Klingelvorgang
enthält zusätzlich den Endpunkt `pickup/ext-<uuid>`, damit `**<Nebenstelle>`
und BLF-Tasten den Anruf übernehmen können. Geräte melden Presence als
`<Nebenstelle>@talkops.local` (`presence_id` im Directory) – darauf
abonnieren BLF-Tasten (`manage-presence` im Profil `internal`).

Gespeicherte Werte aus Nutzer- oder Providerhand (Anzeigenamen, Caller-IDs)
werden vor der Ausgabe von FreeSWITCH-Steuerzeichen bereinigt
(`fsxml::sanitize_value`), da FreeSWITCH `${…}` in Applikationsdaten
expandiert – inkl. API-Aufrufen.

Trunk-Accounts werden zu Gateways `gw-<uuid>` im Profil `external`; nach
Änderungen sendet TalkOps `sofia profile external killgw <gw>` + `rescan`.
Bei Anbietern mit Zugangsdaten je Rufnummer (`per_number`) setzt TalkOps
`extension`/`extension-in-contact`, damit eingehende Anrufe der Nummer
zugeordnet werden können.

Ist die Mailbox der Nebenstelle aktiv (Phase 3), gehen Anrufe statt `480`/`486`
an die Voicemail: bei „Nicht stören“ sofort, ohne Geräte sofort, sonst nach dem
erfolglosen `bridge` (keine Annahme, besetzt, nicht erreichbar).

## Ziele und Anrufsteuerung (Phase 4)

Alle Routing-Ziele sind ein Paar `(art, id)` mit `art` ∈ {`extension`,
`voicemail`, `ring_group`, `time_condition`, `ivr`, `queue`} (Postgres-Enum
`number_destination`). `fsxml::dialplan::route_to` löst sie rekursiv auf;
Ketten (Gruppe → Ausweichziel → Zeitsteuerung → …) sind auf 5 Stufen begrenzt.
Interne Nummern sind über alle Arten eindeutig (`talkops_core::numbering`).

| Ziel | Umsetzung |
|---|---|
| Rufgruppe | ein `bridge`: gleichzeitig `a,b,…` (+ `pickup/ext-*`), nacheinander `[leg_timeout=N]a\|[leg_timeout=N]b`; danach Ausweichziel |
| Zeitsteuerung | Auswertung beim Routing in der Mandanten-Zeitzone: Override → Feiertag (`talkops_core::holidays`, Osterformel, Bundesländer) → Schließtag → Wochenplan |
| Sprachmenü | ESL outbound (`talkops_app=ivr`), `play_and_get_digits`, Auswahl per `transfer dest:<art>:<id> XML talkops` |
| Warteschlange | `callcenter q-<id>` (mod_callcenter); `callcenter.conf` per xml_curl, Agenten/Tiers per `callcenter_config` abgeglichen (diff-basiert, alle 30 s und nach Änderungen) |

Der Kontext `talkops` ist nur über `transfer` erreichbar (keinem Profil
zugeordnet). Alle Anrufe exportieren `force_transfer_context=talkops`, damit
Blind-Transfers (SIP REFER) dort landen und wie gewählte Nummern geroutet
werden – interne Nummern, Parkplätze, externe Nummern über die
Standardrufnummer.

## Identität und Softphone (Phase 7, [ADR 0014](adr/0014-identitaet-oidc-ldap-totp.md), [ADR 0015](adr/0015-webrtc-softphone.md))

- **Anmeldung:** lokal (argon2id) → sonst LDAP (Suche mit Dienstkonto, Bind
  als Benutzer) → optional TOTP-Challenge (`login_challenges`). OIDC über
  `/api/v1/auth/oidc/start|callback` (PKCE, nonce, JWKS-Prüfung). Externe
  Konten: `users.auth_source` + `external_id`, Rollen aus Gruppen
  (`identity_settings`), stündlicher LDAP-Abgleich.
- **Softphone:** Profil `internal` mit `ws-binding 127.0.0.1:5066` und
  `inbound-late-negotiation`; `GET /api/v1/webrtc/ws` reicht SIP über
  WebSocket für angemeldete Benutzer durch (`tokio-tungstenite`);
  `POST /api/v1/me/webrtc` liefert das eigene Browser-Gerät. Im Browser
  SIP.js (`SimpleUser`).

## Türsprechstellen (Phase 6, [ADR 0013](adr/0013-tuersprechstellen-dahua.md))

- Die VTO ist ein Gerät einer Nebenstelle; `fsxml::dialplan::plan_internal`
  erkennt Anrufe von Nebenstellen mit `door_stations`-Eintrag und routet sie
  über `DoorStation::route` (Taste → Ziel, sonst Standardziel) mit
  `talkops_door_id`. Der xml_curl-Handler legt daraufhin asynchron das
  Ereignis `ring` an und holt einen Schnappschuss (`snapshots`-Volume).
- `*85[n]`/`*86[n]`: ESL-Outbound-App `door_open` (Ansage `door_opened`/
  `door_failed`).
- `talkops-doorbell`: HTTP-Digest-Client für `openDoor`, `snapshot.cgi`,
  `magicBox` und den `eventManager`-Multipart-Stream (inkrementeller Parser).
- `talkops_api::doors`: Listener je Station (alle 30 s abgeglichen, Backoff
  bis 60 s), Ereignis-Zuordnung (`AccessControl` → `opened`, `DoorStatus` →
  `door_open`/`door_closed`, `BackKeyLight` 9 → `unlock_failed`,
  Alarm-Codes → `alarm`), Webhook (JSON, 3 Versuche), Token-Hook
  `/hooks/door/<id>/open`.

## Aufzeichnung & Transkription (Phase 5, [ADR 0012](adr/0012-aufzeichnung-und-transkription.md))

- Richtlinie: Mandanten-Vorgabe je Richtung, je Nebenstelle `inherit`/`always`/
  `never` (`never` gewinnt), ausgewertet in `talkops_core::recordings::should_record`.
- `fsxml::dialplan::plan` fügt vor dem ersten `bridge`/`callcenter` ein:
  `talkops_recording=<mandant>/<jjjj-mm>/${uuid}.wav`, `RECORD_STEREO`,
  `RECORD_ANSWER_REQ`, `record_session`; Hinweisansage per
  `bridge_pre_execute_{a,b}leg_app=playback` (Queue: `playback` vorab);
  `stop_record_session` vor Ausweichzielen.
- Der CDR (`/fs/cdr`) trägt `talkops_recording`; TalkOps legt den Datensatz in
  `recordings` an (Länge aus dem WAV-Header, unter 1 s verworfen) und – falls
  eingeschaltet – den Job `transcribe`.
- Media-Worker: zwei Job-Spuren (TTS, Transkription). `transcribe` trennt die
  Stereokanäle, rechnet auf 16 kHz um, ruft `whisper-cli` je Kanal auf und
  führt die Segmente nach Zeit zusammen (`transcripts`, `tsvector` 'simple'
  mit GIN-Index). Voicemails: die Mail (`mail.voicemail`) wird erst nach dem
  Transkript eingeplant.
- Suche: `websearch_to_tsquery`, Ausschnitte per `ts_headline`; Benutzer sehen
  nur Treffer ihrer Nebenstellen. Löschfrist: stündlicher Lauf
  (`talkops_api::retention`).

## Voicemail & Sprachausgabe (Phase 3, [ADR 0010](adr/0010-voicemail-und-sprachausgabe.md))

- Der Dialplan setzt `talkops_app` (`vm_deposit`, `vm_check`, `vm_login`) und
  übergibt per `socket 127.0.0.1:8084 async full` an TalkOps
  (`talkops_esl::outbound`). Jede Applikation läuft per `sendmsg` mit
  `event-lock` und `Event-UUID`; `linger` sorgt dafür, dass eine laufende
  Aufnahme nach dem Auflegen noch gemeldet wird.
- Aufnahmen: `voicemail/<tenant>/<nebenstelle>/<nachricht>.wav`, Begrüßung
  `…/greeting.wav`; Länge aus dem WAV-Header, unter 1 s wird verworfen.
- Systemansagen (`talkops_core::prompts`) rendert der Media-Worker mit Piper
  nach `sounds/system/<sprache>/<key>-<hash>.wav`; Text-Begrüßungen als Job
  `tts.greeting`.
- Neue Nachricht → Job `mail.voicemail` (im TalkOps-Dienst, SMTP via `lettre`)
  und MWI an alle Geräte der Nebenstelle; neu registrierte Geräte bekommen den
  aktuellen Stand sofort.

## Provisioning (Phase 2)

Telefone holen ihre Konfiguration per HTTP(S) von `/provisioning/…` (Basic-Auth
mit generierten Zugangsdaten; Fehlversuche sind wie der Login begrenzt):

| Pfad | Inhalt |
|---|---|
| `y0000000000XX.cfg` | gemeinsame Yealink-Datei: Provisioning-Server, nächtlicher Abgleich, Admin-Passwort, Zeitzone/NTP, Sprache, Telefonbücher, Action-URLs |
| `<mac>.cfg` | je Telefon: SIP-Konten (DECT: Mobilteile), Funktionstasten (Leitung/BLF/Kurzwahl), MWI, Firmware-URL; nur für angelegte MACs |
| `phonebook/internal.xml`, `phonebook/contacts.xml` | Yealink-XML-Telefonbücher (Nebenstellen bzw. gemeinsame Kontakte, Suche über `search`) |
| `firmware/<id>/<datei>` | hochgeladene Firmware (aktiv je Modell) |
| `events?key=…` | Action-URLs: DND am Telefon, „Setup abgeschlossen“ |

Die Modellliste ist eine Datendatei (`presets/phones/yealink.yaml`), die
Templates (`crates/talkops-provisioning/templates`) basieren auf dem Yealink
Auto Provisioning Guide und setzen jeden Wert über `cfg_value` (keine
Zeilenumbrüche). Ein Resync schickt `NOTIFY check-sync` (`sofia profile
internal check_sync`); Yealink startet dann neu und lädt die Konfiguration.
MWI wird per `MESSAGE_WAITING`-Event gesetzt (genutzt ab Phase 3).

## API & Authentifizierung

- REST-API unter `/api/v1`, OpenAPI 3.1 unter `/api/v1/openapi.json` (utoipa).
- Sitzungen: zufälliges Token im Cookie `talkops_session` (`HttpOnly`,
  `SameSite=Strict`, `Secure` hinter HTTPS); in der DB liegt nur der SHA-256.
  Gleitender Ablauf nach 12 h Inaktivität.
- CSRF: zustandsändernde Requests brauchen `X-Requested-With: TalkOps` und das
  Sitzungs-CSRF-Token in `X-CSRF-Token`.
- Rollen: `admin` (alles), `operator` (lesen + Anrufliste aller), `user`
  (eigene Nebenstellen/Geräte/Anrufe).
- Login-Rate-Limit pro IP (10 Versuche / 5 min), argon2id, Audit-Log für alle
  Änderungen und jede Anzeige von SIP-Zugangsdaten.
- Erst-Einrichtung `POST /api/v1/setup` funktioniert nur, solange es keinen
  Benutzer gibt (Advisory-Lock gegen Wettläufe).

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

Ist-Stand Phase 7 plus geplante Ergänzungen (P8):

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
│   │   └── src/  db · jobs · tenant · telemetry · crypto · users · extensions
│   │             trunks · settings · dialing · presets · cdr · audit · phones
│   │             voicemail · prompts · mail · numbering · ring_groups
│   │             time_conditions · holidays · ivr · queues · recordings · doors
│   │             mfa · identity
│   ├── talkops-api/
│   │   └── src/  main · config · auth · error · esl · telephony · mailer
│   │             menu · callcenter · retention · doors · ldap
│   │             voicemail/{mod,ivr}
│   │             fsxml/{sofia,directory,dialplan,cdr,callcenter}
│   │             routes/{health,auth,users,extensions,trunks,settings,fs,
│   │                     phones,provisioning,voicemail,groups,
│   │                     time_conditions,ivr,queues,recordings,doors,
│   │                     oidc,identity,webrtc}
│   ├── talkops-esl/              Event-Socket-Client (inbound) und outbound-Server
│   ├── talkops-provisioning/     Yealink-Templates, Modellkatalog, XML-Telefonbuch
│   ├── talkops-media-worker/     Piper (Ansagen, Begrüßungen), whisper.cpp (Transkription)
│   └── talkops-doorbell/         Dahua-HTTP-API (Digest): Türöffner, Schnappschuss, Ereignisse
├── migrations/                   sqlx-Migrationen (ein Satz für alle Dienste)
├── presets/
│   ├── trunks/                   28 Anbieter-Vorlagen (leonet.yaml, telekom-*.yaml …)
│   └── phones/                   Telefonmodelle (yealink.yaml)
├── web/                          SvelteKit + TS + Tailwind (SPA)
│   └── src/  lib/{api.ts, i18n/, theme.svelte.ts} · routes/
├── docker/
│   ├── freeswitch/               Dockerfile, build-modules.conf, conf/ (Bootstrap), patches/
│   ├── talkops/                  Dockerfile (Rust + Web-UI)
│   └── media-worker/             Dockerfile (Piper + Stimmen, whisper.cpp)
├── tests/
│   ├── sipp/                     register, call, call_busy, call_voicemail, call_ivr,
│   │                             call_hold, bad_password, tone.ulaw
│   └── e2e/run.sh                Ende-zu-Ende-Test gegen den laufenden Stack
├── docs/
│   ├── architecture.md · adr/
│   ├── de/  installation.md · portainer.md · erste-schritte.md · leonet.md · yealink.md
│   │        voicemail.md · anrufsteuerung.md · aufzeichnung.md · tuersprechstelle.md
│   │        anmeldung.md · softphone.md
│   │        (P8: backup.md)
│   ├── en/  (gleiche Inhalte auf Englisch)
│   └── trunk-presets.md          Format & Beitragsregeln für Vorlagen
└── .github/workflows/            ci.yml · freeswitch.yml · e2e.yml · release.yml
```

## Roadmap

| Phase | Inhalt | Status |
|---|---|---|
| 0 | Fundament: Workspace, ADRs, CI, Compose, FreeSWITCH-Image, Postgres, Healthchecks | ✅ |
| 1 | Grundtelefonie: xml_curl-Directory/Dialplan, Nebenstellen, Trunk-Presets (LEONET u. a.), Web-UI Login/User/Trunks, CDR | ✅ (LEONET-Livetest offen) |
| 2 | Yealink: Provisioning, Templates, XML-Telefonbuch, BLF, Feature-Codes, MWI | ✅ (Test mit echten Geräten offen) |
| 3 | Voicemail & TTS: ESL-Voicemail, Piper, Mehrsprachigkeit, Mail | ✅ |
| 4 | Gruppen & Logik: Rufgruppen, Queues, IVR-Editor, Zeitsteuerung/Feiertage, Parken/Pickup | ✅ |
| 5 | Recording & Transkription: Hinweisansage, Whisper, Suche, Retention | ✅ |
| 6 | Türsprechstelle: Dahua VTO, Video, Türöffner, Snapshots, Home Assistant | ✅ (Test mit echter VTO offen) |
| 7 | WebRTC & Identität: Softphone mit Video, LDAP/AD, OIDC, 2FA | ✅ |
| 8 | Betrieb: Backup/Restore, Metriken, Hardening, Setup-Assistent, Release 1.0 | geplant |
