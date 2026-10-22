# TalkOps

[English](README.md) · [Deutsch](README.de.md)

TalkOps is a self-hosted phone system for homelabs and small businesses.
It combines SIP calls, phone provisioning, browser calls, voicemail, fax,
door stations and local speech processing in a Docker Compose stack.

[Install](docs/en/installation.md) · [English handbook](docs/en/index.md) ·
[Deutsches Handbuch](docs/de/index.md) ·
[Releases](https://github.com/itsh-neumeier/talkops/releases) · [Changelog](CHANGELOG.md)

## Capabilities

- **Calls:** provider presets, multiple trunks and numbers, extensions, groups,
  opening hours, Smart Attendant, queues, parking, forwarding and call blocking.
- **Devices:** multi-vendor provisioning, manufacturer settings, device copies,
  line keys and directories; detailed Yealink and Dahua guides.
- **Daily use:** browser softphone with video, shared phone book, call log,
  voicemail, fax sending/receiving and attachment-only mail-to-fax.
- **Media:** generated, recorded or uploaded prompts; recording, local Piper
  speech synthesis and whisper.cpp transcription. Optional cloud transcription
  is configured separately by purpose.
- **Wi-Fi:** [voice survey and roaming analysis](docs/en/wifi-survey.md) with
  read-only UniFi data, live measurements and recorded playback.
- **Administration:** local accounts, TOTP, OIDC, LDAP/AD, roles, audit log,
  encrypted recovery backups, storage targets, diagnostics and metrics.

Provider presets and converted model templates have separate verification states.
Template coverage does not certify each phone or firmware. Test your real provider,
devices, fax delivery and recovery. See the
[verification limits](docs/en/overview.md#support-and-verification-limits).

## Requirements

Use a Linux x86_64 or arm64 host with Docker Engine 24 or later and the Compose
plugin. SIP and RTP use host networking. Docker Desktop is outside the supported
deployment. Reserve the required [ports](docs/en/reference.md#network-defaults).
Use HTTPS for browser microphone access. Transcription needs additional memory
and CPU according to the selected model.

## Quick start: new instance

For an existing backup, read [recovery](docs/en/backup.md) before generating new
credentials. These commands select the published 1.8.5 configuration and images.
Use the same Git release and image version together.

```sh
mkdir talkops && cd talkops
curl -fLO https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.5/docker-compose.yml
curl -fL -o .env https://raw.githubusercontent.com/itsh-neumeier/talkops/v1.8.5/.env.example
sed -i 's/^TALKOPS_VERSION=.*/TALKOPS_VERSION=1.8.5/' .env
sed -i "s/^POSTGRES_PASSWORD=$/POSTGRES_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_SECRET_KEY=$/TALKOPS_SECRET_KEY=$(openssl rand -hex 32)/" .env
sed -i "s/^TALKOPS_ESL_PASSWORD=$/TALKOPS_ESL_PASSWORD=$(openssl rand -hex 16)/" .env
sed -i "s/^TALKOPS_XMLCURL_PASSWORD=$/TALKOPS_XMLCURL_PASSWORD=$(openssl rand -hex 16)/" .env
chmod 600 .env
docker compose config -q
docker compose up -d
docker compose ps
```

Open `http://<host>:8080` and create the first administrator on a trusted network.
Follow [first steps](docs/en/first-steps.md) for the first call and
[installation](docs/en/installation.md) for HTTPS and secret handling.
Keep `TALKOPS_SECRET_KEY` and a recovery backup safe. Complete backup capture
needs the file-access setup in the [backup guide](docs/en/backup.md).

`latest` is the moving stable image tag. A fixed version makes updates explicit;
see [updates](docs/en/updates.md). Do not replace existing deployment files with
this new-instance example.

## Handbook

| Reader | English | Deutsch |
| --- | --- | --- |
| Administrator: install, configure and operate | [Administrator path](docs/en/index.md#administrator-path) | [Administrationspfad](docs/de/index.md#administrationspfad) |
| User: calls, contacts and messages | [User path](docs/en/index.md#user-path) | [Benutzerpfad](docs/de/index.md#benutzerpfad) |
| All topics | [Complete handbook](docs/en/index.md) | [Vollständiges Handbuch](docs/de/index.md) |
| A failed procedure | [Troubleshooting](docs/en/troubleshooting.md) | [Fehlerbehebung](docs/de/fehlerbehebung.md) |

The handbook sources also generate the GitHub Wiki. Repository links work before
Wiki publication. Maintainers use the [Wiki build and publication guide](docs/wiki/README.md).

## Architecture

| Component | Role |
| --- | --- |
| FreeSWITCH 1.10 | SIP and media engine. |
| TalkOps server, Rust | Routing, FreeSWITCH XML/ESL control, REST API and static web UI. |
| PostgreSQL 17 | Persistent data and background job queue. |
| Media worker, Rust | Speech synthesis and transcription jobs. |
| SvelteKit, TypeScript and Tailwind | Browser interface. |

See the [architecture](docs/architecture.md) and [ADRs](docs/adr/README.md).
Existing design records are partly German.

## Development and support

Use [CLAUDE.md](CLAUDE.md) for setup and conventions, [AGENTS.md](AGENTS.md) for
the Codex entry point, and [CI](.github/workflows/ci.yml) for exact checks.
Concurrent tasks use separate worktrees and follow the
[release workflow](docs/release-workflow.md),
[development prompt](docs/development-chat-prompt.md) and
[release-manager prompt](docs/release-manager-prompt.md).
Read the [documentation rules](docs/documentation-style.md) and relevant
[task memory](docs/ai-memory/README.md) before contributing.

Report reproducible problems in [GitHub issues](https://github.com/itsh-neumeier/talkops/issues)
with the version and redacted evidence. Contributions of tested
[provider presets](docs/trunk-presets.md) are welcome.

## License

[AGPL-3.0-only](LICENSE). FreeSWITCH uses MPL 1.1. Imported provisioning sources
retain their own attribution and license information.
