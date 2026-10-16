# TalkOps

**Self-hosted phone system for homelabs and small businesses** – the feature
set of UniFi Talk plus video door stations, call recording, local
transcription and LDAP/AD, packaged as a Docker Compose stack.

> **Status: 1.0.** All roadmap phases are implemented and
> covered by unit, integration and SIPp/browser end-to-end tests. Not yet
> verified with real hardware: LEONET live trunk, Yealink phones, Dahua door
> stations. See the [roadmap](docs/architecture.md#roadmap) and the
> [changelog](CHANGELOG.md).

## Features

- SIP trunks from 28 versioned provider presets (LEONET, Telekom, Vodafone,
  sipgate, easybell, FRITZ!Box, Twilio, …), multiple trunks and numbers,
  emergency routing
- Extensions with multiple devices, DND and forwarding (also via feature codes)
- Smart Attendant call flows (keypress menu, ring phones, schedule, voicemail
  for several recipients, park, forward), queues with greeting, music on hold,
  overflow and business hours, ring groups, opening hours with public holidays
  per German state, call parking
- Prompts generated with two computer voices per language, recorded in the
  browser or uploaded; waveform player
- Dashboard with call statistics, calls in progress and recent calls
- Voicemail with TTS greetings (Piper, local), e-mail delivery with transcript
- Call recording with announcement, local transcription (whisper.cpp),
  full-text search, retention
- Yealink auto-provisioning (desk phones, DECT and AX83H/AX86R Wi-Fi handsets), XML phonebook, BLF, MWI,
  firmware management
- Dahua VTO door stations: video ring, door opener, snapshots, Home Assistant
  webhooks
- WebRTC softphone with video in the browser
- Local accounts with TOTP, OIDC single sign-on, LDAP/Active Directory
- Daily backups with one-command restore, Prometheus metrics, SIP login
  protection (bans after failed logins), hardened containers

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
codec profiles for Wi-Fi handsets [EN](docs/en/codec-profiles.md) / [DE](docs/de/codec-profile.md) ·
voicemail [EN](docs/en/voicemail.md) / [DE](docs/de/voicemail.md) ·
e-mail design and texts [EN](docs/en/mail.md) / [DE](docs/de/e-mail.md) ·
call routing [EN](docs/en/call-routing.md) / [DE](docs/de/anrufsteuerung.md) ·
recording & transcription [EN](docs/en/recording.md) / [DE](docs/de/aufzeichnung.md) ·
door stations [EN](docs/en/door-station.md) / [DE](docs/de/tuersprechstelle.md), Dahua VTO/VTH [EN](docs/en/door-station-dahua.md) / [DE](docs/de/tuersprechstelle-dahua.md) ·
sign-in, 2FA, SSO, LDAP [EN](docs/en/sign-in.md) / [DE](docs/de/anmeldung.md) ·
softphone [EN](docs/en/softphone.md) / [DE](docs/de/softphone.md) ·
backup & restore [EN](docs/en/backup.md) / [DE](docs/de/datensicherung.md) ·
monitoring [EN](docs/en/monitoring.md) / [DE](docs/de/monitoring.md) ·
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

### Working with Claude Code and Codex

Open this repository in either tool. [CLAUDE.md](CLAUDE.md) contains the shared
project guidance; [AGENTS.md](AGENTS.md) tells Codex to read it. Keep both files
in version control so the setup travels with the repository.

For alternating work, finish or hand off the current task before switching
tools in the same checkout. For simultaneous work, give each task its own Git
worktree and branch, then integrate through commits or pull requests. Chat
history is not shared: record unfinished work and verification results in an
issue, PR or short repository note so the next assistant can continue.

For a dedicated merge and release chat, follow the
[parallel development and release workflow](docs/release-workflow.md) and use
the [release-manager prompt](docs/release-manager-prompt.md) (German). Development
tasks submit PRs; the release manager combines the agreed contributions, checks
the integrated result and owns version bumps, merges and publication.

Start new development tasks in either tool with the
[development-chat prompt](docs/development-chat-prompt.md) (German), adding the
task and any acceptance criteria at the end.

New technical documentation and task summaries use ASD-STE100 Simplified
Technical English. See the [documentation rules](docs/documentation-style.md).
Each development and release task saves its result in
[shared task memory](docs/ai-memory/README.md). New chats read relevant summaries
and check their facts against the current code and PR state.

## License

[AGPL-3.0](LICENSE). FreeSWITCH is licensed under MPL 1.1.
