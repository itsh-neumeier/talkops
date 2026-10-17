# TalkOps 1.8.2 release

## Task state

- Task ID: 20261017-codex-release-1.8.2
- Date: 2026-10-17, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.8.2 (merged and remote deleted)
- Base commit: 86772ec0c31990f7a29a639ffb90db6ebef217e2
- Related PR: https://github.com/itsh-neumeier/talkops/pull/50
- Final record: immutable annotated tag audit/release-1.8.2

## Goal

Integrate all open contributions and publish the user-selected version 1.8.2.
Verify the final candidate, exact main and published artifacts. Preserve foreign
work and all published tags.

## Result

TalkOps 1.8.2 is published and registry verification passed. Source PR49 and
integration PR50 are merged. No agreed contribution is omitted or blocked.
The feature is from Codex; no Claude Code contribution was assigned. No later
open PR was present at final inventory. All additional tag workflows passed.
No 1.8.2 publication work remains.

| PR | Exact source head | Dependency | Migration ID | Status |
| --- | --- | --- | --- | --- |
| [49](https://github.com/itsh-neumeier/talkops/pull/49) | e74a06c51c5fc84dd61d0903d6396ddc09660906 | None; PR47 already in base | 20261105000001 | Merged |

## Changes

- Model-based Yealink programmable and expansion keys, ordered call-state
  softkeys, semantic functions, reset output and explicit modern/legacy codes.
  Keep existing graphical line keys and active model catalog.
- Explicit server/phone transport settings; keep device URLs independent of
  web HTTPS. Add redacted unsaved previews, authenticated boot/softkey files
  and bounded download diagnostics, separate from SIP NOTIFY results.
- Tenant/model templates with immutable revisions, validation before activation
  and earlier-revision restore. Limit evaluation, recursion and output. Raw
  upstream PHP/Smarty is reference data and is not executed.
- All 433 original FusionPBX files from 70 folders at upstream
  be0d0e3220e6b3666775fa46db6f1accc60c79c6. Add provenance and MPL-1.1 notices,
  including server-image copies. About 13 MB of reference data is embedded.
- Preserve the source history with merge commits and final 1.8.1 audit history
  from audit/release-1.8.1. No production integration fix was needed.
- Set Cargo.toml, six workspace versions in Cargo.lock, web/package.json and
  both npm lock root versions to 1.8.2. Add English release notes and the
  missing ADR0038 index entry. No unrelated dependency upgrade.

## Decisions

- Use the explicitly requested version 1.8.2. Main is the verified remote
  default. Merge commits are permitted; no active ruleset or main protection
  was reported. Source PR49 was ready, not a draft, and was not in the base.
- Existing credentials, accounts and line-key storage remain compatible.
  Omitted phone provisioning fields preserve saved overrides. The 70 source
  folders do not imply 70 active supported models. Other vendor adapters are
  future work; unsupported vendors receive no Yealink fallback.
- Source Cargo changes enable existing MiniJinja loader/fuel and direct
  serde_json/url use. No package version changes. Release lock metadata is
  version-only against integrated source baseline
  74b6c331dec86e3ed225bd889f3471cb9be4b1d2.
- Closed, unmerged PR36 is outside scope. Keep claude/vigilant-tesla-ef0027
  at 37f8a18aa2971c1a570821d150dcf0941fae12eb; reserve ADR0033.
- Preserve primary checkout 397fc83 and its three untracked Dahua summaries.
  No foreign chat, uncommitted work or foreign local feature branch was used.

## Checks

- Passed locally: cargo fmt; npm ci/lint/check, 34 tests and production build.
  Svelte: zero errors and warnings. Four Compose configurations passed.
- Passed: all 433 reference Git blob IDs and bytes at the pinned upstream head.
- Passed static integration: 46 ordered unique migrations, previous 45 unchanged;
  2,094 matching EN/DE keys/placeholders without duplicates; 13 browser suites
  retained once; 37 unique ADR files and 38 resolving index links; exact source
  ancestry; release version fields only in both locks.
- Passed exact source head: PR49 CI38973944625/E2E38973943991 and push
  CI38973939305/E2E38973939288. All 12 exact source checks passed at inventory.
- Passed candidate: PR CI39005493440/E2E39005493573 and push
  CI39005490181/E2E39005490056. All 12 final-head checks passed before merge.
  Both final E2E runs passed on their first attempts.
- Passed exact main before tagging: CI39006646325 and E2E39006646300.
  Main E2E passed on its first attempt. Rust: 375 tests passed, one existing
  test ignored. Web: 34 tests passed. All browser, source, Compose and image
  gates passed. Server and worker smoke tests report 1.8.2. Full SIPp,
  recording, video and WebRTC scenarios passed.
- CI logs confirm HTTP overrides, actual SIP-secret redaction, unsaved labels,
  template activation/restore/failure preservation, tenant isolation, bounded
  diagnostics, T41P reset/mute/pickup and native model fixtures. Browser checks
  include contact/mobile targets, 70 source folders and save/resync ordering
  at 320/768/1280 px with both themes.
- Passed Release39007742037, tag CI39007742064 and FreeSWITCH39007742129.
  Additional tag E2E39007742124 passed at 2026-10-17T06:01:48Z on attempt1.
  All four tag workflows passed on the exact release commit.
- Not run locally: full Linux workspace/DB/LDAP and Docker/SIPp stack. No local
  Linux Docker daemon is available. The local provisioning build was not started
  because C: has less than 4 GiB free. No foreign outputs were removed. Linux
  CI supplied all required Rust and service gates. No deployed UI or real phone
  was available for manual production or firmware validation.
- Manual integration review covered API/UI fields, old-client preservation,
  tenant access, permission/audit checks, preview redaction and template limits.
  CodeRabbit CLI was unavailable; no CodeRabbit result is claimed.
- STE review: Manual clarity review. Full official standard unavailable;
  full language review remains separate from functional checks.

## Failures and recovery

- No final candidate or exact-main workflow failed. No test was weakened,
  skipped as a workaround or waived. One pre-existing ignored Rust test remains.
- The first local metadata assertion treated an unrelated dependency version
  1.8.1 as a root version. Corrected the checker to compare workspace/npm root
  versions only. Actual lock diffs have no unrelated upgrade. No candidate code
  change resulted. Local test builds used CI because of disk/service limits.

## Release commit, tag and publication

- Final candidate: bb79930c23d01b252de55c139b93dd0564961f3d.
- PR50 merged at 2026-10-17T05:32:37Z. Release main commit:
  c4765b0495adf004f0b2b0f5099cbd098f384f86.
  Its tree equals the tested candidate tree. The source head is an ancestor.
- Source PR49 auto-closed as merged at 05:32:39Z.
- Rechecked latest-main checks, unchanged remote main, empty open-PR scope,
  exact source ancestry, versions and unused tag immediately before tagging.
- Annotated v1.8.2 pushed once at 2026-10-17T05:50:41Z. Tag object:
  3e5834d117026e29b2099098af47024fe0c25abc. Peeled target:
  c4765b0495adf004f0b2b0f5099cbd098f384f86. No published tag moved.
- [TalkOps 1.8.2](https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.2)
  was published at 2026-10-17T05:57:46Z. Latest stable release, not a draft
  or prerelease. English notes include the full scope, migration and limits.

## Registry verification

All three versioned indexes contain linux/amd64 and linux/arm64. All six OCI
configurations report v1.8.2 and the exact release revision. Moving 1.8 and
latest tags match the versioned platform digests.

| Image | Architecture | Published platform digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:cf3a67fe1b028b5bd04f9d14b4b74dd343035f329dc9507f4c43d2b97dabaa32 |
| talkops-server | arm64 | sha256:07e476e0c6ad1f15d68b66a6de08006dedcc0a3fb608fcf8173b4a4a0716c3c9 |
| talkops-media-worker | amd64 | sha256:d4d3fe43c1c3b1ead73a4d21cd34e2181b952caae7e1efd7b5cc2fc020b77d62 |
| talkops-media-worker | arm64 | sha256:e327455d53cd0717aa2020515ec962ce4038d4af2b302c75543a3f68d8fef1ad |
| talkops-freeswitch | amd64 | sha256:edbd12d88f07d7cb066da7ba931cfbc58bca4c4f963a4f74cca0cebdfc5fd893 |
| talkops-freeswitch | arm64 | sha256:4ecb6b1b67ec636494b5bd312367384289ae9dcc3cb6771f41df5b8161451d5a |

## Cleanup

Deleted the unchanged integrated PR49 branch and owned remote batch branch with
one atomic push and expected-head guards after ancestry and artifact checks.
No history was rewritten. Remaining GitHub branches: main and the preserved
unmerged PR36 branch. Foreign local worktrees and feature branches remain.
Do not push the batch branch again. The final audit descendant changes only
this record and must use audit/release-1.8.2 without changing release code.

## Dependencies and open work

- Back up the database and apply 20261105000001_provisioning_profiles before
  the new API starts. Keep server and worker versions together and reload UI.
- Key/profile/template changes need a configuration download. Activation alone
  does not resync. Existing credentials and account assignments remain valid.
- Actual T41P/AX86R phone, expansion-module, key/reset and transport checks are
  not available here. Use the documented acceptance steps after deployment.
- T41P firmware 36.83.0.120 HTTPS root cause remains unconfirmed. An HTTP
  exception needs an initial reachable LAN device/DHCP URL and credentials.
  No automatic downgrade or global TLS validation change was added. NOTIFY
  success does not prove download success. Additional vendor adapters remain
  future work; source folders are not active model support.
- Existing backup API, notification and Dahua follow-ups remain separate.

## Next action

None for 1.8.2. Release, verification and branch cleanup are complete.
Keep the immutable final audit linked from merged PR50. Preserve closed PR36
and foreign local work. The documented hardware acceptance remains a separate
deployment follow-up; future contributions need a new release assignment.

## References

- [Integration PR50](https://github.com/itsh-neumeier/talkops/pull/50).
- docs/release-workflow.md; docs/release-manager-prompt.md; ADR0038.
- 20261016-codex-provisioning-multivendor-plan.md.
- 20261016-codex-release-1.8.1.md (completed previous release).

## Final workflow evidence

- [Candidate CI](https://github.com/itsh-neumeier/talkops/actions/runs/39005493440)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39005493573).
- [Exact-main CI](https://github.com/itsh-neumeier/talkops/actions/runs/39006646325)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39006646300).
- [Release](https://github.com/itsh-neumeier/talkops/actions/runs/39007742037),
  [tag CI](https://github.com/itsh-neumeier/talkops/actions/runs/39007742064),
  [tag E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39007742124),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/39007742129).
- All final runs passed on their expected immutable heads. Main and published
  tag still target c4765b0495adf004f0b2b0f5099cbd098f384f86.
- The final audit descendant changes only this record. It is not a new release
  candidate and does not change main or the published release tag.
