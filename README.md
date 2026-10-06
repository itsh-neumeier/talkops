# TalkOps

**Self-hosted phone system for homelabs and small businesses** – the feature
set of UniFi Talk plus video door stations, call recording, local
transcription and LDAP/AD, packaged as a Docker Compose stack.

> **Status: early development (phase 5 – recording & transcription).** Extensions with
> multiple devices, SIP trunks from 28 provider presets, internal and external
> calls, emergency routing, a call log, Yealink auto-provisioning (desk phones
> and DECT), BLF keys, XML phonebook, DND/forwarding feature codes and
> firmware management, voicemail with TTS greetings (Piper, German/English)
> and e-mail notification, ring groups, opening hours with German public
> holidays, voice menus, queues, call parking, call recording with
> announcement and local transcription (whisper.cpp) with full-text search
> work; door stations follow. See the [roadmap](docs/architecture.md#roadmap).

## Highlights (planned)

- Multiple SIP trunks with versioned provider presets (LEONET, Telekom,
  Vodafone, sipgate, easybell, FRITZ!Box, Twilio, …)
- Extensions with multiple devices, ring groups, queues, graphical IVR editor,
  business hours and public holidays per German state
- Voicemail with TTS greetings (Piper, local), e-mail delivery with transcript
- Call recording with announcement, local transcription (whisper.cpp), full-text search
- Yealink auto-provisioning, XML phonebook, BLF, DECT, firmware management
- Dahua VTO door stations with H.264 video, door opener, snapshots, Home Assistant events
- WebRTC softphone with video in the browser
- Local accounts, LDAP/AD and OIDC single sign-on, TOTP

## Architecture

| Component | Role |
|---|---|
| **FreeSWITCH 1.10** | SIP and media engine (kept "dumb") |
| **talkops** (Rust) | Control plane: serves FreeSWITCH config from PostgreSQL via `mod_xml_curl`, controls calls via the Event Socket, REST API, web UI |
| **PostgreSQL 17** | Single source of truth, job queue |
| **media-worker** (Rust) | TTS (Piper) and transcription (whisper.cpp) jobs |
| **Web UI** | SvelteKit + TypeScript + Tailwind, served by the Rust server |

Details: [docs/architecture.md](docs/architecture.md) and the
[architecture decision records](docs/adr/README.md) (German).

## Quick start

Requirements: a Linux host with Docker Engine and the Compose plugin
(host networking is required for SIP/RTP).

```sh
curl -LO https://raw.githubusercontent.com/itsh-neumeier/talkops/main/docker-compose.yml
curl -L -o .env https://raw.githubusercontent.com/itsh-neumeier/talkops/main/.env.example
# set POSTGRES_PASSWORD, TALKOPS_SECRET_KEY, TALKOPS_ESL_PASSWORD, TALKOPS_XMLCURL_PASSWORD
docker compose up -d
```

Then open `http://<host>:8080` and create the admin account. Guides:
installation [EN](docs/en/installation.md) / [DE](docs/de/installation.md) ·
first steps [EN](docs/en/first-steps.md) / [DE](docs/de/erste-schritte.md) ·
Portainer [EN](docs/en/portainer.md) / [DE](docs/de/portainer.md) ·
LEONET [EN](docs/en/leonet.md) / [DE](docs/de/leonet.md) ·
Yealink phones [EN](docs/en/yealink.md) / [DE](docs/de/yealink.md) ·
voicemail [EN](docs/en/voicemail.md) / [DE](docs/de/voicemail.md) ·
call routing [EN](docs/en/call-routing.md) / [DE](docs/de/anrufsteuerung.md) ·
recording & transcription [EN](docs/en/recording.md) / [DE](docs/de/aufzeichnung.md) ·
[trunk presets](docs/trunk-presets.md).

## Development

```sh
# Rust workspace (tests need a PostgreSQL server)
cargo clippy --workspace --all-targets -- -D warnings
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/postgres cargo test --workspace

# Web UI
cd web && npm ci && npm run dev

# Whole stack from source, then SIPp end-to-end test
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build
tests/e2e/run.sh
```

See [CLAUDE.md](CLAUDE.md) for conventions. Contributions of trunk presets are
especially welcome – see [docs/trunk-presets.md](docs/trunk-presets.md).

## License

[AGPL-3.0](LICENSE). FreeSWITCH is licensed under MPL 1.1.
