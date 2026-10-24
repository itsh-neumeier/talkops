# TalkOps 1.8.9 release

## Task state

- Task ID: 20261023-codex-release-1.8.9
- Date: 2026-10-23, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.9
- Base: efe4b79dd637119e130d7ff392d7dada98c014ae
- Related PR: https://github.com/itsh-neumeier/talkops/pull/71

## Goal and result

Integrate all open ready contributions and publish the user-selected 1.8.9.
Merge the previous 1.8.8 audit and both ready Claude Code contributions with
full source history. No conflicts. Neither source head was in main:

| Source | Exact head | Dependency |
| --- | --- | --- |
| PR69: outbound dialing permissions | 1bbbf24651cdb7474e41218de280733078a237bb | Released geo data |
| PR70: offline map detail layers | 288dea5924e70a23d0d29b4c882d38ceba9e78fa | Released call map |

Both final heads have 16 green checks. No open Codex contribution or missing
agreed handoff. Draft PR61 remains review-only at 16c6c45a61655a9ff215c350ac47d499d15dae9f;
it is excluded and preserved.

## Changes and decisions

- Use the explicit user-selected 1.8.9. PR69 suggests a minor release; the current
  explicit target selects this additive scope. Existing configured tenants keep
  unrestricted dialing; new-installation defaults are documented.
- Version Cargo workspace, seven lock entries and npm roots with package tools.
  No dependency version upgrade. Add English changelog and active EN/DE install examples.
- New migration 20261110000001_dialing_permissions follows 54 released migrations.
- No feature code changed by the release manager. Preserve foreign worktrees,
  draft61, closed/unmerged PR36 and the unmerged development-prompt branch.

## Checks

- Source and all 16 final candidate checks passed.
- Local cargo fmt, clean npm ci, lint/check (zero errors/warnings), 68 tests/build
  passed. Docs: 13 tests, 75 pages/670 links. Four Compose variants, provisioning
  hashes/generators, version-only metadata, 55 unique ordered migrations (54 released
  SQL files unchanged), 2,701 matching EN/DE keys/placeholders and ADR links passed.
- Not run locally: full Linux/Rust/services,
  images and SIPp/browser stack use CI; no local Linux Docker daemon.
- Manual integration review completed: rule/emergency checks, tenant/assignment
  permissions/audit, fax/transfer contracts, XML marks, outbound block reasons,
  lazy map layers and safe labels. CodeRabbit CLI unavailable.
- STE clarity rules applied; full standard unavailable, full language review pending.

## Dependencies and open work

New sensitive permission dialing.manage; five seeded profiles. Country rules
need geo data. Backup and migration required; keep server/worker together.
PR69's dedicated SIPp blocked-call and REFER scenarios and real-phone transfer
mark check are explicit source follow-ups, not completed acceptance evidence.
New pure/database tests and existing workflow regressions must pass for release.
No live deployment, hardware acceptance or Wiki publication is claimed.

## Next action

Validate final integration, merge, verify exact-main CI/E2E, push unused v1.8.9
once and verify Release workflow, GitHub release, all three amd64/arm64 manifests,
version/revision labels and 1.8/latest tags. Archive the final audit separately.

## References

- https://github.com/itsh-neumeier/talkops/pull/69
- https://github.com/itsh-neumeier/talkops/pull/70
- docs/ai-memory/20261023-claude-outbound-dialing-permissions.md
- docs/ai-memory/20261023-claude-call-map.md
- docs/ai-memory/20261023-codex-release-1.8.8.md
- docs/adr/0045-outbound-dialing-permissions.md
- docs/release-workflow.md

## Merged checkpoint

- Final candidate: 10de3668584099104145a92676d75e96f32d7fc2.
- PR CI40029374266/E2E40029374212 and push CI40029313616/E2E40029313634 all passed.
  Rust: 507 passed, one existing ignored model test. Web: 68 tests/17 browser suites.
  DB/LDAP, recovery, image/storage/version smoke, G.711/T.38 fax and SIPp/softphone passed.
  Server and worker displayed 1.8.9.
- Resume on 2026-10-24: scope, main and source heads unchanged. Integration PR71
  and both sources merged. Source PRs closed at 2026-10-24T05:13:04Z.
- Main: 9f693914a9103c5f23ea35f65ca0220e6e1f5b28. Its tree equals the candidate;
  both source heads are ancestors. Exact-main CI40065384349/E2E40065384379 running.
- No tag pushed. Do not push the merged integration branch again. Archive later
  task-only changes through audit/release-1.8.9 after publication.

## Publication started

Exact-main CI40065384349 and E2E40065384379 passed before publication.
Fresh guards verified unchanged main, both source ancestors, identical candidate
and main trees, matching versions, only unchanged draft61 open and latest green gates.
New annotated v1.8.9 was pushed once on 9f693914a9103c5f23ea35f65ca0220e6e1f5b28.
Tag object: 5eab050fbc90607a1fd8159e44248b5ba26353c1.
Release40066670895, tag CI40066670888/E2E40066670889 and FreeSWITCH40066670977
are running. GitHub release and artifact verification pending.
