# CLAUDE.md – TalkOps

Guidance for AI assistants and contributors working in this repository.

## What TalkOps is

A self-hosted PBX with the feature set of UniFi Talk plus video door stations,
call recording, local transcription and LDAP/AD. Target: family homelabs and
small businesses (up to ~50 extensions). **Robustness and maintainability beat
building things ourselves**: use proven components where they exist.

- **FreeSWITCH** (1.10.x, built from source) does SIP and media. It is kept
  "dumb": a minimal bootstrap config, everything else fetched from TalkOps.
- **Rust control plane** answers FreeSWITCH's `mod_xml_curl` lookups from
  PostgreSQL and drives calls / consumes events via the Event Socket (ESL).
- **SvelteKit SPA** (static build) served by the Rust server.
- **Media worker** runs Piper (TTS) and whisper.cpp (transcription) jobs from
  a Postgres-backed job queue.

Read `docs/architecture.md` and the ADRs in `docs/adr/` before larger changes.
A change that contradicts an ADR needs a new ADR that supersedes it.

## Repository layout

```
crates/talkops-core          domain types, DB pool, migrations, job queue, telemetry
crates/talkops-api           HTTP server + `talkops` binary (serve/migrate/healthcheck)
crates/talkops-esl           FreeSWITCH Event Socket client
crates/talkops-provisioning  Yealink provisioning (phase 2)
crates/talkops-media-worker  TTS/transcription worker binary
crates/talkops-doorbell      Dahua VTO integration (phase 6)
migrations/                  sqlx migrations (shared by all services)
web/                         SvelteKit + TypeScript + Tailwind UI
docker/                      Dockerfiles; docker/freeswitch/conf = bootstrap config
presets/trunks/              SIP trunk provider presets (YAML, phase 1)
docs/                        architecture, ADRs, user docs (docs/de, docs/en)
```

## Commands

```sh
# Rust
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5432/postgres cargo test --workspace

# Run the server locally (needs Postgres; FreeSWITCH optional)
TALKOPS_DATABASE_URL=postgres://... TALKOPS_ESL_PASSWORD=dev TALKOPS_XMLCURL_PASSWORD=dev \
  TALKOPS_WEB_DIR=web/build cargo run -p talkops-api -- serve

# Web UI
cd web && npm ci && npm run lint && npm run check && npm run build
npm run dev   # proxies /api to 127.0.0.1:8080

# Full stack from source
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build
```

CI (`.github/workflows/ci.yml`) runs exactly these checks; keep them green.

## Conventions

- **Commits**: Conventional Commits (`feat(api): …`, `fix(esl): …`, `docs: …`,
  `ci: …`, `build(docker): …`). Small, focused commits.
- **Phases**: work follows the roadmap in `docs/architecture.md`. Every phase
  ends runnable, tested, documented, with green CI. Do not skip phases.
- **Rust**: edition 2024, `unsafe` is forbidden, clippy warnings are errors in CI.
  Libraries return typed errors (`thiserror`), binaries use `anyhow`.
- **SQL**: use runtime-checked `sqlx::query`/`query_as` (not the `query!`
  macros) so builds never need a live database. Every query touching domain
  data is covered by a `#[sqlx::test]` integration test.
- **Migrations**: `migrations/YYYYMMDDHHMMSS_description.sql`, forward-only,
  never edit a migration that has been released.
- **Tenancy**: every domain table has `tenant_id UUID NOT NULL REFERENCES tenants`.
  The UI is single-tenant (`TenantId::DEFAULT`) for now.
- **Secrets**: no plaintext passwords in the DB. Web logins: argon2id hashes.
  SIP/trunk secrets: encrypted with `TALKOPS_SECRET_KEY` (ADR 0008). Never log secrets.
- **FreeSWITCH config**: do not add business logic to `docker/freeswitch/conf`.
  Serve it from the database via `/fs/xml` instead (ADR 0006).
- **Trunk presets**: data files in `presets/trunks/*.yaml`, never hard-coded.
  Parameters must come from official provider documentation; record the source
  and mark unverified presets as `untested`.
- **Frontend**: SvelteKit 3 / Svelte 5 runes. Config lives in `web/vite.config.ts`
  (no `svelte.config.js`); import app code via `#lib/...` (package.json `imports`).
  All UI strings go through `t()` with keys in both `en.ts` and `de.ts`.
- **Language**: code, comments, README and commit messages in English.
  Architecture docs and ADRs in German. User docs in both `docs/de/` and `docs/en/`.
- **Images**: all images run as UID/GID 10001 (`talkops`) so shared volumes work.
  `docker-compose.yml` must never contain `build:` (Portainer compatibility).
