# TalkOps 1.8.9 release

## Task state

- Task ID: 20261023-codex-release-1.8.9
- Started: 2026-10-23, Europe/Berlin; continued 2026-10-24
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.9 (merged and removed from origin)
- Base: efe4b79dd637119e130d7ff392d7dada98c014ae
- Final candidate: 10de3668584099104145a92676d75e96f32d7fc2
- Main/release: 9f693914a9103c5f23ea35f65ca0220e6e1f5b28
- Integration: https://github.com/itsh-neumeier/talkops/pull/71
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.9

## Goal and result

Publish all open ready contributions in the user-selected version 1.8.9.
Both Claude Code contributions are merged with full source history:

| Source | Exact head | Dependency |
| --- | --- | --- |
| PR69: outbound dialing permissions | 1bbbf24651cdb7474e41218de280733078a237bb | Released geo data |
| PR70: offline map detail layers | 288dea5924e70a23d0d29b4c882d38ceba9e78fa | Released call map |

Both source heads had 16 green checks and were not in the original main.
No open Codex contribution or missing agreed handoff. Draft PR61 remains
review-only at 16c6c45a61655a9ff215c350ac47d499d15dae9f; excluded and preserved.
On resume, scope, main and source heads were unchanged. All sources and PR71
merged; source PRs closed at 2026-10-24T05:13:04Z. Both heads are ancestors
of main, whose tree equals the final candidate.

## Changes and decisions

- Merge previous 1.8.8 audit, then PR69 and PR70. No conflicts.
- Use the explicit user-selected 1.8.9 for this additive scope. PR69's minor
  suggestion does not override the current explicit target. Existing configured
  tenants retain unrestricted dialing; new-installation defaults are documented.
- Version Cargo workspace, seven workspace lock entries and npm roots with
  package tools. No dependency version upgrade. Add combined English changelog
  and active EN/DE installation/update/Portainer/reference examples.
- No feature code changed by the release manager. No live deployment or Wiki push.

## Checks

- Local cargo fmt, clean npm ci, lint/check (zero errors/warnings), 68 web tests
  and build passed. All 13 docs tests/Wiki --check passed: 75 pages/670 links.
- Four Compose variants, provisioning hashes and both generator checks passed.
  Version-only metadata verified; 55 unique ordered migrations and 54 released
  SQL files unchanged. All 2,701 EN/DE keys/placeholders match. ADR links valid.
- Manual review: emergency bypass and rule precedence; profile/assignment
  permissions, tenant scope and audit; fax/transfer contracts, XML marks and
  outbound block reasons; lazy map layers, safe labels and API/UI contracts.
- All 16 final candidate checks passed: PR CI40029374266/E2E40029374212 and
  push CI40029313616/E2E40029313634.
- Exact-main CI40065384349 and E2E40065384379 passed before publication.
  Rust: 507 passed, one existing model-dependent test ignored. Web: 68 tests
  and 17 browser suites. Docs: 13 tests. DB/LDAP, recovery, storage/version smoke,
  G.711/T.38 fax and SIPp/browser softphone passed. Server and worker displayed 1.8.9.
- No integration test failure. The source local root-only readonly fax failure
  was absent in final unprivileged CI; no release gate was waived.
- Full Linux/Rust build/services, containers and SIPp/browser tests were not run
  locally; no local Linux Docker daemon. CI supplied evidence.
- CodeRabbit CLI unavailable. Repository clarity rules applied. Full STE standard
  unavailable; full language review pending.

## Publication and artifacts

Fresh guards verified unchanged main, both source ancestors, identical candidate
and main trees, matching versions, only unchanged draft61 open and latest green gates.
New annotated v1.8.9 was pushed once on the release commit. Tag object:
5eab050fbc90607a1fd8159e44248b5ba26353c1.
Release workflow40066670895 passed. Stable GitHub release published at
2026-10-24T05:40:41Z. FreeSWITCH40066670977 passed. Additional tag CI40066670888
and E2E40066670889 also passed. All final tag workflows are successful.

Docker buildx imagetools verified all three manifests and both platform configs.
All six OCI version labels equal v1.8.9; revision labels equal the release commit.
Tags 1.8/latest have the same platform digests as 1.8.9.

| Image | Platform | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:685e5b8157cd5c09f9f4e37803b714fed0e1e3a1c4c87d491d9df9fd8b7650f1 |
| talkops-server | arm64 | sha256:2c709cc437ed9c821731eb45301e4ea55d9f078fab9f3174204e289c70140fdd |
| talkops-media-worker | amd64 | sha256:84a4717bb3417038333a4b2a3d69d893b37930496b6f6d406b6d2d0dd8ab5d75 |
| talkops-media-worker | arm64 | sha256:f477ef57edb03d14d6a4f6d1b41f1c127ff74f246201bd5d0937a4073308ef81 |
| talkops-freeswitch | amd64 | sha256:13cf0227212dfcc6a347d48cc8ad6bc62fe6d95ab5aa66dad61e1aae2196bdcc |
| talkops-freeswitch | arm64 | sha256:5dcf6f33e0c1d506d5a64f103dcb921856798487baf2daf04fefaf87904fde69 |
## Cleanup and separate follow-ups

Removed both unchanged merged source branches and codex/release-1.8.9 with
atomic expected-head guards. Preserve draft61 branch claude/wonderful-fermat-6iar0b,
closed unmerged PR36 branch claude/vigilant-tesla-ef0027 and development-prompt
branch claude/charming-bell-fh6q3y. Foreign worktrees/primary files untouched.
Archive the task-only record under audit/release-1.8.9; do not push the merged
integration branch again. The audit does not change released application code.

Back up and apply 20261110000001_dialing_permissions; keep server/worker together.
The migration seeds five profiles. Existing tenants with extensions/trunks keep
Unrestricted. New installations use International with premium/network blocks.
New sensitive permission: dialing.manage. Country rules need shipped geo data.

Dedicated SIPp blocked-call and REFER cases and real-phone blind-transfer mark
acceptance remain explicit source follow-ups. New pure/database tests and
existing workflow regressions passed; those specific SIPp/hardware cases are
not claimed as verified. This limitation is in PR71 and the release notes.

Map details load from the own server by zoom; no street data and no places below
1,000 people. Optional tiles still need the provider's policy. Number areas are
not caller positions. No live deployment, hardware acceptance or Wiki update claimed.

## Next action

None for this release. The final record is archived under audit/release-1.8.9
and linked from PR71 and the release.
Dedicated transfer/SIPp/hardware tests, draft61 and Wiki publication are separate follow-ups.

## References

- https://github.com/itsh-neumeier/talkops/pull/69
- https://github.com/itsh-neumeier/talkops/pull/70
- https://github.com/itsh-neumeier/talkops/pull/71
- docs/ai-memory/20261023-claude-outbound-dialing-permissions.md
- docs/ai-memory/20261023-claude-call-map.md
- docs/ai-memory/20261023-codex-release-1.8.8.md
- docs/adr/0045-outbound-dialing-permissions.md
- docs/release-workflow.md
