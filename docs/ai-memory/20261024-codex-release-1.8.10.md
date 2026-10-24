# TalkOps 1.8.10 release

## Task state

- Task ID: 20261024-codex-release-1.8.10
- Date: 2026-10-24, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.10 (merged and removed)
- Base: 9f693914a9103c5f23ea35f65ca0220e6e1f5b28
- Related PR: https://github.com/itsh-neumeier/talkops/pull/79

## Goal and result

Integrate all ready contributions and publish the user-selected 1.8.10.
Preserve source histories and the previous release audit. Included scope:

| PR | Source head | Contribution and dependency |
| --- | --- | --- |
| 72 | fcf9ecd774450e0b0b1bdf9c71dab5f851ab74ab | Dashboard map details |
| 73 | 986a2f634e60ea5afc1313aac1393b2c04202787 | Ubiquiti API naming |
| 74 | a97e918c7d12e79af4efc6f5aa861f24a0ccb78a | Phone network data; depends on 73 |
| 75 | 56e06df90500148ebddd93ca4f5f119d5f2f13ff | Dashboard collapsed lists |
| 76 | 3949c696699b56d16b9a5c0642f6a7b40e124b3c | Settings search; source web CI failed |
| 77 | 6653a153c28193e3abf7fd83b0e7c87868e95055 | Dashboard charts |
| 78 | 6e44e26e176d1582d68e2c122340fa965610da6d | Side panels; depends on 74/73 |

All contributions are from Claude Code. No open Codex contribution or missing
agreed handoff. Draft PR61 remains review-only at 16c6c45a61655a9ff215c350ac47d499d15dae9f;
it is excluded and preserved. None of the selected source heads was in main.

## Changes and decisions

- Merge in order 72, 73, 74, 78, 75, 76, 77; preserve all source histories.
- Resolve CI workflow conflicts by retaining all old/new browser suite commands.
  Resolve dashboard import conflict by keeping collapse and KPI/chart imports.
- PR76 web CI failed in audit.mjs: getByLabel('Suche') matched both the old
  audit searchbox and the new settings combobox. Use the audit searchbox role
  and exact accessible name. No test removed or assertion weakened.
- Use the explicit user-selected 1.8.10 for this additive scope. Source requests
  mention the next feature release; the current explicit target selects this batch.
- Version Cargo workspace/seven lock entries and npm roots with package tools.
  No dependency upgrades. Add English changelog and active EN/DE install examples.

## Checks

- Six sources have 16 green checks. PR76 has two failed web jobs; the cause is
  corrected in integration. All 16 final candidate checks passed.
- Local fmt, clean npm ci, lint/check (zero errors/warnings), 83 tests/build passed.
  Docs: 13 tests, 75 pages/672 links. Four Compose variants, provisioning checks,
  version-only metadata, 56 unique ordered migrations (55 released files unchanged),
  2,770 matching EN/DE keys/placeholders, ADR links and 22 suite registrations passed.
- Local hidden preview launch rejected by automatic approval review (blocked by policy).
  No local browser suite ran. Final CI supplied all 22 browser-suite results.
- Manual review completed: default-off read-only network requests, permissions/tenant
  cache/audit, API/UI contracts, panel actions, search visibility and chart/list/map coexistence.
- Full Linux/Rust/services, images and SIPp stack use CI; no local Linux Docker daemon.
- CodeRabbit CLI unavailable. STE clarity rules applied; full standard unavailable,
  full language review pending.

## Dependencies and open work

Migration 20261111000001_unifi_phone_network follows 55 released files.
Network switch is off by default; read-only console requests, tenant cache,
unifi.manage for the switch and phones.view for data. Existing identifiers stay.
New chart field previous is additive. Preserve search permissions, responsive
panels, ringing-call visibility and all 22 browser suites.
Real-console phone-network acceptance remains pending. No hardware test, live
deployment or Wiki publication is claimed. Preserve draft/unmerged branches.

## Next action

None for this release. The final record is archived under audit/release-1.8.10.
Draft PR61, real-console acceptance and Wiki publication remain separate work.

## References

- docs/ai-memory/20261023-claude-call-map.md
- docs/ai-memory/20261024-claude-ubiquiti-api-naming.md
- docs/ai-memory/20261024-claude-phone-network-stats.md
- docs/ai-memory/20261024-claude-dashboard-collapse.md
- docs/ai-memory/20261024-claude-settings-search.md
- docs/ai-memory/20261024-claude-chart-design.md
- docs/ai-memory/20261024-claude-side-panel.md
- docs/ai-memory/20261023-codex-release-1.8.9.md
- docs/release-workflow.md
## Integration and publication

- Final candidate: 2735b53deeba235671f46c93b19d4a8f500afc8f. All 16 final checks passed.
  PR CI 40085342807/E2E 40085342817; push CI 40085303591/E2E 40085303608.
- Integration PR79 and all seven sources merged. PR74/78 were retargeted to main
  after integration. Source heads stayed unchanged. Every source head is an ancestor.
- Exact target: 8f32ec7778ec7a147c09ac98c97c5f1a434e22cc. Its tree equals the final candidate.
  Exact-main CI 40086734382 and E2E 40086734353 passed before tagging.
- Rust: 511 passed, one existing ignored model test. Web: 83 tests, 22 browser
  suites. Docs: 13 tests. DB/LDAP, recovery, image/storage/version smoke,
  G.711/T.38 received fax PDFs and SIPp/browser softphone passed.
  Server and worker displayed 1.8.10. No final gate was waived.
- New annotated v1.8.10 pushed once on the exact target. Tag object:
  c996d8c33f3ee5385d5026fbf404c67059c3f532. No published tag moved.
- Release workflow 40087924900 passed. Stable release published at
  10/24/2026 11:05:24. Additional tag CI 40087924877,
  E2E 40087924889 and FreeSWITCH 40087924926 passed.
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.10

## Registry verification

Docker buildx imagetools verified all three versioned manifests and six configs.
Every OCI version label equals v1.8.10; revision labels equal the exact target.
Tags 1.8/latest have the same platform digests as 1.8.10.

| Image | Platform | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:8e0c2a2d692032acfcca1c4bf69fc0bdf4d0825858c69bea9be3a2640a2dadc0 |
| talkops-server | arm64 | sha256:2756ef807ce234a4bf06ddb0d5ef88ef940dc10d05d4f4cc96a32045775b5ea7 |
| talkops-media-worker | amd64 | sha256:53f66c8a0856f2765c8b8de212aa8c839ed8820ede12c4af4a52bc206d7a16d2 |
| talkops-media-worker | arm64 | sha256:a5137b471f8b5b9b47b5d8b54c65fcbae2c3f7d9b87643dc7c920ba04c23c7f8 |
| talkops-freeswitch | amd64 | sha256:7032ec627a88d1613dcf70c3e28133889878fe498189015d661132e63ba8e3c1 |
| talkops-freeswitch | arm64 | sha256:d0005b2f315e783fc5143f453018e32a56d3842013ca4550417c399b4e4ec72f |

## Cleanup and audit

Removed the seven unchanged merged source branches and codex/release-1.8.10
with atomic expected-head guards. Preserve draft PR61 at its recorded head,
closed unmerged PR36 branch claude/vigilant-tesla-ef0027 and development-prompt
branch claude/charming-bell-fh6q3y. Foreign worktrees and primary files untouched.
Only this task record differs from the released target in the separate audit.
Do not push the merged integration branch again. This audit changes no product code.
