# TalkOps 1.8.8 release

## Task state

- Task ID: 20261023-codex-release-1.8.8
- Date: 2026-10-23, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.8 (merged and removed from origin)
- Base: a2478c81877eafce8bd1a40361c84ce097a72fa6
- Final candidate: f1212fe8fc10ca6a533e4694bd09efb2124d7f78
- Main/release: efe4b79dd637119e130d7ff392d7dada98c014ae
- Integration: https://github.com/itsh-neumeier/talkops/pull/68
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.8

## Goal and result

Publish all open ready contributions in the user-selected version 1.8.8.
All three stacked Claude Code contributions are merged with full source history:

| Source | Exact head | Dependency |
| --- | --- | --- |
| PR65: geo data and backend lookup | 4b48bcddf291cb35dcaeed7ac6b22b056226c3d5 | None |
| PR66: map API and call log view | 7bfab01c0ba7fcc4676a0e257cc54dbfcd44720c | PR65 |
| PR67: dashboard widget and optional tiles | b7eec4b50f55c1b06fdbd21badcfe0a8f471706f | PR65 and PR66 |

All source heads had 16 green checks. No open Codex contribution or missing
agreed handoff. Draft PR61 remains review-only and in progress at
16c6c45a61655a9ff215c350ac47d499d15dae9f; it is excluded and preserved.
No ready head was in the original main. PR66/67 were retargeted to main
after integration. Source PRs closed as merged at 17:04:38/39Z.
The final candidate and main trees are identical; all source heads are ancestors.

## Changes and decisions

- Merge the previous 1.8.7 audit, then PR65, PR66 and PR67. No merge conflicts.
- Use the explicit user-selected 1.8.8 for this additive scope. No breaking change identified.
- Version Cargo workspace, seven workspace lock entries and npm roots with
  package tools. No release-induced dependency upgrades. Leaflet comes from PR66.
- Add combined English changelog notes; update active EN/DE install examples.
  No feature code changed by the release manager.
- Keep offline defaults, call visibility, tenant scope and source attribution.
  Optional tiles disclose browser IP and map section. Number areas are not caller positions.
- No live Wiki publication or deployment.

## Checks

- Local cargo fmt, lint and 55 web tests passed. Docs: 13 tests, 75 pages/664 links.
  Four Compose variants, provisioning hashes and both generator checks passed.
- Version-only lock metadata verified. All 54 migration IDs unique and ordered;
  53 released SQL files unchanged. All 2,643 EN/DE keys/placeholders match.
  ADR IDs/index links valid. All 17 browser suites registered, including call-map.
- Manual review: call scope, tenant isolation, tile permission/audit, URL validation,
  CSP host restriction, escaped attribution, lazy/offline loading and API/UI contracts.
- All 16 final candidate checks passed: PR CI39987855559/E2E39987855809;
  push CI39987782510/E2E39987782459.
- Exact-main CI39992089713 and E2E39992089729 passed before publication.
  Rust: 486 passed, one existing model-dependent test ignored. Web: 55 tests
  and 17 browser suites. Docs: 13 tests. DB/LDAP, recovery, image/storage smoke,
  G.711/T.38 fax and SIPp/browser softphone passed. Server/worker displayed 1.8.8.
- No tests failed during integration. Source local root-only readonly fax failure
  was absent in final unprivileged CI; no release test was waived.
- Not run locally: clean dependency installation, full Rust build/services,
  images and Linux/SIPp/browser stack. C: had about 220 MB free and no Linux Docker.
  CI supplied evidence. Installed dependencies sufficed for local lint/unit tests.
- Git fetch obtained refs but automatic geometric repack failed on a multi-pack-index
  permission error. No Git files or foreign caches removed. Later fetch disabled auto maintenance.
- CodeRabbit CLI unavailable. STE clarity rules applied; full standard unavailable,
  so full language review remains pending.

## Publication and artifacts

Fresh guards verified unchanged main, all source ancestors, identical candidate
and main trees, matching metadata, only unchanged draft61 open and latest green gates.
New annotated v1.8.8 was pushed once on the release commit. Tag object:
990e043bafc7d7a0843107bec5db120ac60bab02.
Release workflow39995083021 passed. Stable GitHub release published at
2026-10-23T17:36:02Z. FreeSWITCH39995082739 passed. Additional tag CI39995082896
and E2E39995083008 also passed. All final tag workflows are successful.

Docker buildx imagetools verified all three versioned manifests and both platform
configs. All six OCI version labels equal v1.8.8, revision labels equal the
release commit. Tags 1.8 and latest have the same platform digests as 1.8.8.

| Image | Platform | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:520405b3edb03dff88ecda83fa5ce14bf3ae921529076ade4e74db760388333c |
| talkops-server | arm64 | sha256:9e5432d5e2096dec6efef3698d9a4eb7a9d0241ba55ecc9b7273401bc409ec29 |
| talkops-media-worker | amd64 | sha256:2bc6b26d4c175134a3134e2e65b6f4ef7d4ee537f0d0e4b9caa8853163527391 |
| talkops-media-worker | arm64 | sha256:1545f8786ffb92df1bff061b2b5b3817a5004495c1c848f93ee0c61887992a5b |
| talkops-freeswitch | amd64 | sha256:9e67cd353971000f525ec63f19578ab49cf22ae9246db4078bd5e24cc5e80811 |
| talkops-freeswitch | arm64 | sha256:96b3c384403720c7663a1586f4730bcee4624e21443e821d7e6bbd9b3feda278 |
## Cleanup and open work

Removed all three unchanged merged source branches and codex/release-1.8.8
with atomic expected-head guards. Preserve draft61 branch claude/wonderful-fermat-6iar0b,
closed unmerged PR36 branch claude/vigilant-tesla-ef0027 and development-prompt
branch claude/charming-bell-fh6q3y. Foreign worktrees/primary files are untouched.
Archive this task-only record under audit/release-1.8.8; do not push the merged
integration branch again. The audit does not change released application code.

Back up and apply 20261109000001_call_map_tiles; keep server/worker together.
The image includes static geo data. Custom deployments can set TALKOPS_GEO_DIR.
A missing path gives an empty index/warning; invalid data stops startup.
No new container capability. External tiles require the provider's usage policy.
Map windows report truncation after 50,000 calls. Portability/VoIP limit accuracy.
No live deployment or new Wiki publication is claimed.

## Next action

None for this release. The final record is archived under audit/release-1.8.8
and linked from PR68 and the release.
Draft61 and future Wiki publication remain separate follow-ups.

## References

- https://github.com/itsh-neumeier/talkops/pull/65
- https://github.com/itsh-neumeier/talkops/pull/66
- https://github.com/itsh-neumeier/talkops/pull/67
- https://github.com/itsh-neumeier/talkops/pull/68
- docs/ai-memory/20261023-claude-call-map.md
- docs/ai-memory/20261023-codex-release-1.8.7.md
- docs/adr/0044-call-origin-map-and-number-geolocation.md
- docs/release-workflow.md
