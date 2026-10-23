# TalkOps 1.8.8 release

## Task state

- Task ID: 20261023-codex-release-1.8.8
- Date: 2026-10-23, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.8
- Base: a2478c81877eafce8bd1a40361c84ce097a72fa6
- Related PR: https://github.com/itsh-neumeier/talkops/pull/68

## Goal and result

Integrate all open ready contributions and publish the user-selected 1.8.8.
Merge the previous 1.8.7 audit, then the three stacked Claude Code PRs with
full source history. No conflicts. None of these heads was in main:

| Source | Exact head | Dependency |
| --- | --- | --- |
| PR65: geo data and backend lookup | 4b48bcddf291cb35dcaeed7ac6b22b056226c3d5 | None |
| PR66: map API and call log view | 7bfab01c0ba7fcc4676a0e257cc54dbfcd44720c | PR65 |
| PR67: dashboard widget and optional tiles | b7eec4b50f55c1b06fdbd21badcfe0a8f471706f | PR65 and PR66 |

All three final heads have 16 successful checks. No open Codex contribution.
Draft PR61 is still a review-only proposal at 16c6c45a61655a9ff215c350ac47d499d15dae9f;
it is excluded and preserved. No agreed ready contribution is missing.

## Changes and decisions

- Use the explicit user-selected 1.8.8 for this additive scope.
- Set Cargo workspace, seven workspace lock entries and npm roots with package
  tools. No release-induced dependency upgrade. Leaflet dependencies come from PR66.
- Add combined English changelog notes; update active EN/DE install examples.
- New migration 20261109000001_call_map_tiles must follow the 53 released files.
- Keep offline defaults and existing call visibility. Optional external tiles
  disclose browser IP and map section. Number areas are not caller positions.
- Preserve draft/closed/unmerged branches and foreign worktrees. No live Wiki push.

## Checks

- Source CI/E2E: all green. All 16 final candidate checks passed.
- Local cargo fmt, lint and 55 web tests passed. Docs: 13 tests, 75 pages/664 links.
  Four Compose variants, provisioning hashes/generators, version-only metadata,
  54 unique ordered migrations (53 released files unchanged), 2,643 matching EN/DE
  keys/placeholders and ADR links passed. CI registers 17 browser suites.
- Local C: has approximately 220 MB free. No local dependency installation,
  full Rust build or Linux/Docker stack. Use CI for web, services and images.
- Git fetch obtained the refs, but automatic geometric repack failed with a
  multi-pack-index permission error. No Git files or foreign caches were removed.
- Manual integration review completed: call/tenant scope, tile permission/audit,
  URL validation/CSP, escaped attribution, lazy/offline loading and API/UI contracts.
  CodeRabbit CLI unavailable.
- STE: repository clarity rules applied. Full standard unavailable; review pending.

## Dependencies and open work

Back up, migrate and keep server/worker versions together. Custom deployments
need TALKOPS_GEO_DIR for the shipped data. A missing path gives an empty index;
invalid data stops startup. No new container capability. Follow the configured
tile provider's usage policy. Check API/UI, translation, migration and geo data
contracts before publication. No live deployment or Wiki update is claimed.

## Next action

Check the final candidate, merge, verify exact-main CI/E2E, then push unused
v1.8.8 once. Verify Release workflow, stable release and all three image
manifests for amd64/arm64, version/revision labels and 1.8/latest tags.

## References

- https://github.com/itsh-neumeier/talkops/pull/65
- https://github.com/itsh-neumeier/talkops/pull/66
- https://github.com/itsh-neumeier/talkops/pull/67
- docs/ai-memory/20261023-claude-call-map.md
- docs/ai-memory/20261023-codex-release-1.8.7.md
- docs/adr/0044-call-origin-map-and-number-geolocation.md
- docs/release-workflow.md

## Merged checkpoint

- Final candidate: f1212fe8fc10ca6a533e4694bd09efb2124d7f78.
- PR CI39987855559/E2E39987855809 and push CI39987782510/E2E39987782459 all passed.
  Rust: 486 passed, one existing ignored model test. Web: 55 tests and 17 browser suites.
  DB/LDAP, recovery, storage/version smoke, G.711/T.38 fax and SIPp/softphone passed.
  Server and worker smoke output displayed 1.8.8.
- PR66/67 retargeted from source branches to main after integration. All sources
  and integration PR68 are merged. Source heads unchanged before merge.
- Main: efe4b79dd637119e130d7ff392d7dada98c014ae. Its tree equals the candidate.
  All three source heads are ancestors. Source PRs closed at 17:04:38/39Z.
- Exact-main CI39992089713 and E2E39992089729 are running. No tag pushed.
- Do not push the merged integration branch again. Archive later task-only records
  under audit/release-1.8.8 after publication.

## Publication started

Exact-main CI39992089713 and E2E39992089729 passed before publication.
Fresh guards verified unchanged main, all source ancestors, identical candidate
and main trees, matching versions, only unchanged draft61 open and latest green gates.
New annotated v1.8.8 was pushed once on efe4b79dd637119e130d7ff392d7dada98c014ae.
Tag object: 990e043bafc7d7a0843107bec5db120ac60bab02.
Release39995083021, tag CI39995082896/E2E39995083008 and FreeSWITCH39995082739
are running. GitHub release and artifact verification are pending.
