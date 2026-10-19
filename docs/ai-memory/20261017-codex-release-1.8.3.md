# TalkOps 1.8.3 release

## Task state

- Task ID: 20261017-codex-release-1.8.3
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.3, now removed from origin
- Base commit: c4765b0495adf004f0b2b0f5099cbd098f384f86
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/52

## Goal

Integrate every open PR and publish the user-selected version 1.8.3.

## Result

TalkOps 1.8.3 is published as the latest stable GitHub release. The inventory
had one open, ready contribution: Codex PR51, exact head
8c8e1deac2c10859f3411202718040ae0a90ea0a. It was absent from main and had
complete handoff and successful exact-head CI/E2E. No Claude Code contribution
was assigned. No open contribution or promised handoff was omitted.

Merge commits preserve the source history and the previous release audit.
There were no conflicts or integration code corrections. PR52 and source PR51
are merged. Exact-main CI/E2E passed before the new tag was pushed once.

## Changes

- PR51: 385 usable source profiles and the explicit GXP1630 selector variant.
  Preserve 1,037 original files from FusionPBX revision
  20143aec4366afc2755f336d3058243479d49d44. Do not execute upstream PHP/Smarty.
- Add categorized manufacturer fields, native key functions, locale controls,
  authenticated device files, native downloads and QR exports.
- Add atomic same-model phone copies. Generate independent SIP credentials.
  Exclude identity, secrets and runtime fields. Reject stale previews and
  defer copied keys until accounts are assigned. Keep existing Yealink IDs.
- Version metadata: Cargo.toml, six workspace Cargo.lock entries,
  web/package.json, npm lock root versions and CHANGELOG.md.
- This record: release gates, source scope, publication and artifact evidence.

## Decisions

- Use the owned release checkout. Preserve all foreign worktrees and changes.
- No new migration. All 46 released migration files remain unchanged.
  Existing JSON storage requires 20261105000001 from version 1.8.2.
- No dependency version upgrade. PR51 adds four direct edges to already locked
  packages and MiniJinja loop controls. Release lock edits change versions only.
- Keep the closed, unmerged PR36 branch. Its changes are outside the open scope.
- No tag move, main force-push, manual release dispatch or competing release.

## Checks

- Passed locally: Rust fmt; clean web install, lint, check (zero errors/warnings),
  35 tests and production build; four Compose variants; git diff --check.
- Passed locally: original hashes for all 1,037 source files, previous 433
  Yealink blobs and two protocol/function blobs; both generator --check runs.
- Passed locally: 46 unique, ordered, unchanged migrations; 2,155 matching
  EN/DE keys and placeholders; 14 browser suites; 38 unique ADR files and
  valid index links. Release package tools changed no dependency versions.
- Manual integration review: tenant/role guards, clone transaction and source
  lock, stale-preview hash, secret exclusions, declared artifact paths,
  bounded rendering, additive API/UI fields and existing Yealink behavior.
- Source CI39056380706 and E2E39056380708 passed on exact source head.
- Candidate 79c8a8f162562563b18983eb7f720979a2955631 passed all 12 checks:
  PR CI39061647155 / E2E39061647151 and push CI39061640712 / E2E39061640721.
- Exact-main CI39062967200 and E2E39062967294 passed before tagging.
  Rust: 392 tests passed; one pre-existing test ignored. Web: 35 tests and
  all 14 browser suites passed. Full Linux database/LDAP, image smoke tests,
  Compose, SIPp and browser softphone passed. No failed gate or waiver.
- Both exact-main image smoke tests report 1.8.3 for server and worker.
- Release39064111899, tag CI39064111892 and FreeSWITCH39064111972 passed.
- Additional tag E2E39064111921 passed on the exact release commit.
- CodeRabbit CLI unavailable. No CodeRabbit run occurred.
- Not run locally: full Linux services and image/telephone stack. Docker daemon
  unavailable and C: has less than 2 GiB free. Full checks used GitHub CI.
- STE review: repository clarity rules applied. Full standard unavailable;
  full language review pending.

## Publication

- Main/release commit: 7beb63d5b8e3917b7621588413afcec2a8893456.
- Main tree equals the final tested integration candidate. Main was rechecked
  immediately before tagging; source ancestry and empty open scope were verified.
- PR52 merged at 2026-10-17T19:33:19Z. Source PR51 is merged through it.
- Tag: v1.8.3; annotated object 910ebc6af4e38cb8471a5f73ec888f755b738e31.
- GitHub release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.3
- Published: 2026-10-17T19:56:50Z. Latest stable, not a draft or prerelease.
- The existing Release workflow built six images and published three manifests.
  Every index contains linux/amd64 and linux/arm64. All six OCI configurations
  report v1.8.3 and the exact release revision. Moving 1.8/latest tags match
  the versioned platform digests. English release notes include scope and limits.

## Artifact evidence

| Image | Architecture | Published platform digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:f239fd32be105beba187cd31a557fd3519bc8414e5e59f6de3ff441d6842bd2e |
| talkops-server | arm64 | sha256:29c1eb40c4ff03f6f33cdcef55006eee64ad874c016eb6944e902373cde0fe07 |
| talkops-media-worker | amd64 | sha256:16f099dc7002ed9eaf4d5a2734b8812f9dff8ce507783afdf0a7508f1fbf58e8 |
| talkops-media-worker | arm64 | sha256:b4a6ce1c509acb56849b6e152a077287175840ecf03eaae0bb2f6cea0060f478 |
| talkops-freeswitch | amd64 | sha256:b0ba49995ccc14097f0cf7e138eccd55f4b8f0f36e7e090b0e5f135955798e39 |
| talkops-freeswitch | arm64 | sha256:b2075d0eed3d7a1f06dc5952ad67431ddbd44e391139bf12891a73ed69f96767 |

## Cleanup

The unchanged source PR51 and owned release branches were removed from origin
with one atomic push after ancestry, publication and artifact checks. Expected
head guards prevented deletion of changed branches. No history was rewritten.
Remote branches left: main and closed, unmerged PR36's claude/vigilant-tesla-ef0027.
Foreign local branches and worktrees remain intact. Preserve the final record
through audit/release-1.8.3; never push the deleted batch branch again.

## Dependencies and open work

No missing contribution or software release blocker remains. Hardware acceptance
is separate. All new profiles retain device_validation: not_tested. Fixtures do
not certify registration, LEDs, firmware or language dictionaries. Firmware is
not bundled. Empty Spectralink and Grandstream Wave entries remain blocked.
CP965/Sangoma source aliases need device tests. Swissvoice can use an external
directory. Some old Cisco devices need TFTP. Cloud Softphone needs a Cloud ID
and portal configuration. Native profiles have no remote resync/reboot without
an adapter. Provisioning changes need a configuration download. Keep server and
worker versions together; back up before upgrading and reload the web UI.

## Next action

None. Publication, verification and cleanup are complete. The final audit
changes only this record. It does not change main or v1.8.3.

## References

- https://github.com/itsh-neumeier/talkops/pull/51
- https://github.com/itsh-neumeier/talkops/pull/52
- docs/ai-memory/20261017-codex-provisioning-all-vendors-proposal.md
- docs/ai-memory/20261017-codex-release-1.8.2.md
- docs/adr/0039-source-derived-multi-vendor-provisioning.md
- docs/release-workflow.md

## Final workflow evidence

- [Candidate CI](https://github.com/itsh-neumeier/talkops/actions/runs/39061647155)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39061647151).
- [Exact-main CI](https://github.com/itsh-neumeier/talkops/actions/runs/39062967200)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39062967294).
- [Release](https://github.com/itsh-neumeier/talkops/actions/runs/39064111899),
  [tag CI](https://github.com/itsh-neumeier/talkops/actions/runs/39064111892),
  [tag E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39064111921),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/39064111972).
- All runs passed on their expected heads at first attempt. No waiver.
