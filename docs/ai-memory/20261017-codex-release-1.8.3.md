# TalkOps 1.8.3 release

## Task state

- Task ID: 20261017-codex-release-1.8.3
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.3
- Base commit: c4765b0495adf004f0b2b0f5099cbd098f384f86
- Integration PR: Pending

## Goal

Integrate all open PRs and publish the user-selected version 1.8.3.

## Result

The inventory has one open, ready PR. PR51 (Codex) has exact head
8c8e1deac2c10859f3411202718040ae0a90ea0a. It targets main and is not a draft.
Its complete CI/E2E passed. No Claude Code contribution is assigned or pending.
The source was not in main. Merge commits preserve its complete history and the
previous release audit. There were no merge conflicts. Publication is pending.

## Changes

- PR51: multi-vendor provisioning, manufacturer settings, native exports and
  same-model phone copies with independent credentials. See its source summary.
- Release metadata: Cargo workspace and six lock entries, npm root versions
  and CHANGELOG.md. Version 1.8.3 follows the explicit user request.
- This file: durable integration and publication record.

## Decisions

- Use the dedicated release checkout. Preserve foreign worktrees and changes.
- No new migration. All 46 released SQL files are unchanged and uniquely ordered.
- Keep dependency versions unchanged. PR51 adds four existing locked dependency
  edges and MiniJinja loop controls. The release changes only package versions.
- Preserve closed, unmerged PR36 and its branch. It is outside the open scope.
- Do not tag until the final candidate and exact merged main CI/E2E pass.

## Checks

- Passed locally: Rust fmt; web clean install, lint, type check, 35 unit tests
  and production build; four Compose configurations; git diff --check.
- Passed locally: 1,037 original library blobs, 433 prior Yealink blobs and two
  original function/protocol blobs. Native and function generators reproduce.
- Passed locally: all 46 migrations unchanged; 2,155 matching EN/DE keys and
  placeholders; 14 retained browser suites; 38 unique ADR files and valid index.
- Source PR CI: 39056380706; E2E: 39056380708. Both passed on the exact source.
- Manual integration review: tenant/role guards, atomic clone transaction,
  stale-preview check, secret exclusions, declared artifact paths, bounded
  rendering and additive API/UI fields. No integration correction was required.
- CodeRabbit CLI unavailable. No CodeRabbit review ran.
- Not run locally: full Rust/database/LDAP, Linux images, SIPp and browser
  stack. Docker daemon unavailable and C: has less than 2 GiB free. Use CI.
- STE review: repository clarity rules applied. Full standard unavailable;
  full language review pending.

## Dependencies and open work

- No missing source handoff or PR dependency. PR49 is already released.
- Migration 20261105000001 from 1.8.2 is the existing storage prerequisite.
- All new profiles retain device_validation: not_tested. Real device and
  firmware acceptance remains a deployment task. Firmware files are absent.
- Spectralink and Grandstream Wave stay blocked due to empty source templates.
  Some old Cisco devices need TFTP; Cloud Softphone needs external Cloud ID.
  Native profiles have no remote resync/reboot without a manufacturer adapter.

## Next action

Run final integration CI/E2E, merge, validate exact main, then publish and verify
v1.8.3. Record each gate and artifact before reporting completion.

## References

- https://github.com/itsh-neumeier/talkops/pull/51
- docs/ai-memory/20261017-codex-provisioning-all-vendors-proposal.md
- docs/ai-memory/20261017-codex-release-1.8.2.md
- docs/adr/0039-source-derived-multi-vendor-provisioning.md
- docs/release-workflow.md

## Merged checkpoint

- Integration PR: https://github.com/itsh-neumeier/talkops/pull/52
- Final candidate: 79c8a8f162562563b18983eb7f720979a2955631.
- Candidate CI39061647155 and E2E39061647151 passed. Duplicate push CI39061640712
  and E2E39061640721 passed. All 12 checks are green; 392 Rust tests passed,
  one pre-existing test ignored. All 14 browser suites passed.
- PR52 merged on 2026-10-17T19:33:19Z. Source PR51 is merged through it.
- Main: 7beb63d5b8e3917b7621588413afcec2a8893456. Its tree equals the tested candidate.
- Exact-main CI39062967200 and E2E39062967294 are running. Tag remains pending.
- Do not push the batch branch again. Preserve later summary changes in an
  audit descendant after all publication evidence is available.

## Publication started

Main CI39062967200 and E2E39062967294 passed on the exact release commit.
A fresh scope check found no open PR. Main remained unchanged. Guarded tag
publication verified the merged PR, candidate tree, ancestry and versions.
Tag v1.8.3 was pushed once at 7beb63d5b8e3917b7621588413afcec2a8893456.
Annotated tag object: 910ebc6af4e38cb8471a5f73ec888f755b738e31.
The existing Release workflow owns image and GitHub release publication.
Publication and registry verification are not yet complete.
