# CLAUDE.md – TalkOps

Guidance for AI assistants and contributors working in this repository.

## Claude Code and Codex collaboration

This file is the shared source of project guidance. Codex enters through
`AGENTS.md`, which directs it here. Update shared rules here rather than
maintaining separate copies for each tool.

- Start by checking the current branch, `git status` and relevant diffs.
  Preserve existing changes made by the user or another assistant.
- When switching tools, work sequentially in the same checkout. For concurrent
  work, use a separate Git worktree and branch for each task, starting from an
  agreed commit. A worktree does not copy uncommitted changes. Integrate changes
  through reviewed commits or pull requests.
- For parallel development with a dedicated release manager, follow
  `docs/release-workflow.md`. Development tasks submit focused PRs with handoff
  details; the release-manager task owns integration, release version changes,
  tags and publication. This role is assigned explicitly by the user.
- Treat repository files, issues and pull requests as durable project context;
  do not assume access to another assistant's chat history. At task start, read
  `docs/ai-memory/README.md`. Search the task summaries and read relevant files.
  Check the current code and PR state before you use an old summary.
- Save a task summary in `docs/ai-memory/` before task completion or transfer.
  Update it after important decisions and at useful checkpoints in long tasks.
  Use the template and file rules in that directory. Commit the summary with
  the task changes. Include its path in the PR or transfer message.
  Development and release tasks both follow this rule.
- Run the checks relevant to the change and report their results. Use
  `.github/workflows/ci.yml` for the exact CI commands and service prerequisites.
  Distinguish checks that passed from checks that could not run locally.
- Shell examples below use POSIX syntax. In PowerShell, set environment
  variables with `$env:NAME = 'value'`. Use a Linux environment for the full
  Docker/SIPp stack; integration tests need the services and tools used by CI.

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
presets/phones/              phone model catalog (YAML, phase 2)
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
  TALKOPS_SECRET_KEY=$(openssl rand -hex 32) TALKOPS_PRESETS_DIR=presets/trunks \
  TALKOPS_PHONES_DIR=presets/phones TALKOPS_PROVISIONING_DIR=/tmp/talkops-prov \
  TALKOPS_WEB_DIR=web/build cargo run -p talkops-api -- serve

# Web UI
cd web && npm ci && npm run lint && npm run check && npm run build
npm run dev   # proxies /api to 127.0.0.1:8080

# Full stack from source + SIPp end-to-end test (needs sipp, jq)
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d --build
tests/e2e/run.sh
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
  Serve it from the database via `/fs/xml` instead (ADR 0006). Every value in
  generated dialplan/directory XML that is not fully controlled by TalkOps
  must go through `fsxml::sanitize_value` (FreeSWITCH expands `${…}`, including
  API calls).
- **API**: handlers live in `talkops-api/src/routes`, each with a
  `#[utoipa::path]` annotation and registered via `routes!()`. Every mutation
  writes an audit entry (never with secrets) and checks the role.
- **Trunk presets**: data files in `presets/trunks/*.yaml`, never hard-coded.
  Parameters must come from official provider documentation; record the source
  and mark unverified presets as `untested`.
- **Frontend**: SvelteKit 3 / Svelte 5 runes. Config lives in `web/vite.config.ts`
  (no `svelte.config.js`); import app code via `#lib/...` (package.json `imports`).
  All UI strings go through `t()` with keys in both `en.ts` and `de.ts`.
- **Language**: code, comments and commit messages use English. New technical
  documentation, new ADRs, release notes and task summaries use English with
  ASD-STE100 Simplified Technical English. Follow `docs/documentation-style.md`.
  User documentation has an English source in `docs/en/` and a German
  translation in `docs/de/`. Existing German architecture documents and ADRs
  do not need a full translation for an unrelated task. German translations,
  UI text and user-facing chat prompts can stay in German.
- **Images**: Debian 13 "trixie" based (ADR 0009); FreeSWITCH patches live in
  `docker/freeswitch/patches/` with their upstream origin in the header.
  All images run as UID/GID 10001 (`talkops`) so shared volumes work.
  `docker-compose.yml` must never contain `build:` (Portainer compatibility).
