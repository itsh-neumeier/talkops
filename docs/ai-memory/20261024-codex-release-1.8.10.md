# TalkOps 1.8.10 release

## Task state

- Task ID: 20261024-codex-release-1.8.10
- Date: 2026-10-24, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.10
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

Validate final candidate, merge, verify exact-main CI/E2E, push unused v1.8.10
once and verify workflow, stable release, all three amd64/arm64 manifests,
version/revision labels and 1.8/latest. Save the final audit separately.

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

## Merged checkpoint

- Final candidate: 2735b53deeba235671f46c93b19d4a8f500afc8f.
- PR CI40085342807/E2E40085342817 and push CI40085303591/E2E40085303608 all passed.
  Rust: 511 passed, one existing ignored model test. Web: 83 tests/22 browser suites.
  Audit regression and all new browser suites passed. DB/LDAP, recovery, image/storage
  smoke, G.711/T.38 fax and SIPp/softphone passed. Server/worker displayed 1.8.10.
- PR74/#78 retargeted to main after integration. Integration79 and all seven sources
  merged; source heads unchanged before merge. Source PRs closed at 10:38:47/48Z.
- Main: 8f32ec7778ec7a147c09ac98c97c5f1a434e22cc. Its tree equals the candidate;
  all seven source heads are ancestors. Exact-main CI40086734382/E2E40086734353 running.
- No tag pushed. Do not push the merged release branch again. Archive later task-only
  changes under audit/release-1.8.10 after publication.

## Publication checkpoint

- Exact-main CI40086734382 and E2E40086734353 completed successfully.
- Pushed new annotated tag v1.8.10 once. Tag object:
  c996d8c33f3ee5385d5026fbf404c67059c3f532.
  Peeled target: 8f32ec7778ec7a147c09ac98c97c5f1a434e22cc.
- Release40087924900 builds all six image variants. Publication and registry
  verification remain pending. Additional tag CI40087924877, E2E40087924889
  and FreeSWITCH40087924926 are running. No competing release created.