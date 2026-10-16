# TalkOps 1.8.1 release

## Task state

- Task ID: 20261016-codex-release-1.8.1
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.8.1 (merged and remote deleted)
- Base commit: 7d7990b99e9f02c126ad3983da633476a22a9708
- Related PR: https://github.com/itsh-neumeier/talkops/pull/48
- Final record: immutable annotated tag audit/release-1.8.1

## Goal

Integrate all open contributions and publish the user-selected version 1.8.1.
Verify the final candidate, exact main and published artifacts. Preserve foreign
work and all published tags.

## Result

TalkOps 1.8.1 is published and registry verification passed. Both agreed source
PRs and integration PR48 are merged. No agreed contribution is omitted or
pending. Both features are from Codex; no Claude Code contribution was assigned.
All publication and additional tag checks passed. No later open PR was present
at final inventory. No 1.8.1 release work remains.

| PR | Exact source head | Dependency | Migration ID | Status |
| --- | --- | --- | --- | --- |
| [46](https://github.com/itsh-neumeier/talkops/pull/46) | f81e140b62abec0e709bd737f3d6d7d07fd1e4f2 | None | None | Merged |
| [47](https://github.com/itsh-neumeier/talkops/pull/47) | a48a2fade3a851d102a706fef733bd19f821126f | None | None | Merged |

## Changes

- PR46: Country flags with bundled numbering data and local SVGs. Immediate
  call placeholders, independent cards, refresh retention, reduced motion and
  stale-response protection. Add the call-presentation browser suite.
- PR47: Ten new Yealink desk models; graphical key setup for all 21 desk
  models. Phonebook and internal targets, save/resync, additive model metadata,
  corrected capacities and retained legacy overrides. Add phone-keys tests.
- Preserve both source histories with merge commits. Preserve the final 1.8.0
  audit history from audit/release-1.8.0. No production integration fix was needed.
- Set Cargo.toml, six workspace packages in Cargo.lock, web/package.json and
  both npm lock root versions to 1.8.1. Add English release and upgrade notes.
  Only the version fields changed relative to the merged source baseline
  b747d9fb092f78b935afb8de67710edf0b6c1720.

## Decisions

- Use the explicitly requested version 1.8.1. Target main is the verified
  remote default. Merge commits are permitted. No active ruleset or main
  protection was reported. No public endpoint was removed.
- Neither source was already in the base. Both exact heads had ready handoffs
  and 12 green source checks. No dependency between the two PRs was required.
- New pinned packages are libphonenumber-js 1.13.15 and flag-icons 7.6.0,
  from PR46. No unrelated dependency upgrade was included.
- Closed, unmerged PR36 is outside the open-PR scope. Preserve
  claude/vigilant-tesla-ef0027 at 37f8a18aa2971c1a570821d150dcf0941fae12eb.
  It is not part of the release. ADR0033 remains reserved there.
- Preserve primary checkout 397fc83 and its three untracked Dahua summaries.
  Do not import uncommitted work or use foreign chats. Foreign worktrees and
  local feature branches were not changed.

## Checks

- Passed locally: cargo fmt; 13 provisioning tests and doc tests; npm ci,
  lint, check, 31 tests and build. Svelte: zero errors and warnings. Four
  Compose configurations passed without a Docker Linux daemon.
- Passed static review: 45 ordered unique migrations, all released SQL files
  unchanged; 1,975 matching EN/DE keys and placeholders without duplicates;
  both source ancestors; all 12 browser suites retained once; version-only
  release metadata against the integrated source baseline.
- Passed source PR46: CI38946303569, E2E38946303551, push CI38946297378 and
  E2E38946298448. Passed PR47: CI38949067742, E2E38949067882,
  push CI38949060274 and E2E38949060264. All 12 checks per exact source head.
- Passed final candidate: PR CI38952162964/E2E38952162953 and push
  CI38952157423/E2E38952157572. All 12 checks were green before merge.
- Passed exact main before tagging: CI38956465215 and E2E38956465225.
  Main E2E passed on its first attempt. Logs confirm model maps, last-key
  rendering, all 21-model API/browser cases, legacy overrides and call loading.
  Full SIPp, recording, video and WebRTC passed. Server and worker smoke tests
  report 1.8.1. All workspace/DB/LDAP, web, Compose and image gates passed.
- Passed publication: Release38958752273. Additional tag CI38958752284 and
  FreeSWITCH38958752047 passed. Additional E2E38958752093 passed at
  2026-10-16T20:07:25Z. All four tag workflows use the exact release commit.
- Not run locally: full Rust workspace/DB/LDAP and Linux Docker/SIPp stack.
  Windows GNU lacks dlltool.exe; no Linux Docker daemon is available. Linux CI
  supplied those gates. No deployed UI or physical phone was available for
  manual production or firmware validation. The ignored real-whisper unit
  test was not executed; real clip transcription E2E coverage is separate.
- Manual integration review checked API/UI metadata, tenant-scoped phone
  updates, existing permission/audit checks, copied phonebook values, retained
  legacy keys and source changes. CodeRabbit CLI was unavailable; no CodeRabbit
  review is claimed.
- STE review: Manual clarity review. Full official standard unavailable;
  full language review remains separate from functional checks.

## Failures and recovery

- Candidate push E2E38952157572 attempt1 missed the ring-group voicemail
  storage deadline. Both calls reached vm_deposit and recording. The second
  message was not stored in the 15-second count window. Voicemail, ring-group
  and SIPp scenario code were unchanged by this batch. Parallel PR E2E passed
  on attempt1; push attempt2 passed without code, test or timeout changes.
  This shows a sporadic failure; its precise cause is not established. The
  failure remains in the workflow history. No test was weakened or waived.
- Cargo metadata offline first stopped because a locked cross-platform crate
  was not cached. Online metadata downloaded existing locked crates. Only the
  six workspace versions changed in Cargo.lock.
- The first local publication helper stopped before tag creation: PowerShell
  needed -AsHashtable for the empty npm lock root key. Corrected the helper,
  repeated all guards, and pushed one new tag. No code or test change resulted.

## Release commit, tag and publication

- Final candidate: 5260739be6d530d6d4f8fa76d122ea0efaa9249c.
- PR48 merged at 2026-10-16T19:38:58Z. Release main commit:
  86772ec0c31990f7a29a639ffb90db6ebef217e2.
  Its tree equals the tested candidate tree. Both agreed heads are ancestors.
- Source PRs46 and47 auto-closed as merged at 19:39:00Z.
- Rechecked latest-main checks, unchanged remote main, empty open-PR scope,
  both source ancestors, versions and unused tag immediately before tagging.
- Annotated v1.8.1 was pushed once at 2026-10-16T19:57:47Z.
  Tag object: b62c6350a56682917e6f9136389fd8a3243258e1. Peeled target:
  86772ec0c31990f7a29a639ffb90db6ebef217e2. No published tag moved.
- [TalkOps 1.8.1](https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.1)
  was published at 2026-10-16T20:05:36Z. It is the latest stable release,
  not a draft or prerelease. English notes cover both contributions and limits.

## Registry verification

All three versioned manifests contain linux/amd64 and linux/arm64. All six OCI
configurations report v1.8.1 and the exact release revision. Moving tags 1.8 and
latest match the versioned platform digests.

| Image | Architecture | Published platform digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:6a64d5285bfbad66b6a05fa9d940bdf197fe9627e202c81893cb41b6bb6ee62c |
| talkops-server | arm64 | sha256:b07f81a37591e4905143ab2270b7b9e1083c9f7d8c06c7e21915a273d3c7b2aa |
| talkops-media-worker | amd64 | sha256:0ec58fb19d53e978b860e7eb858ff2848aea5d4f3fff077423dbc97d20961312 |
| talkops-media-worker | arm64 | sha256:ed6f2d88bfbb619c63f7acbab57a4e1bb7851de3711ee8c862cd8d107b876d05 |
| talkops-freeswitch | amd64 | sha256:5362234ce87a465288b33dd603c7bbd9da9a7402a1cb8e8cbd3241ec0cc827e7 |
| talkops-freeswitch | arm64 | sha256:3b2f35d0cb7531fe0105269f3200f6fa123f713bc429e52c29c3e2232a997eb3 |

## Cleanup

Deleted only the two unchanged integrated source branches and the owned remote
batch branch. One atomic push used expected-head guards after ancestry and
published-artifact checks. No history was rewritten. Remaining GitHub branches:
main and the preserved unmerged PR36 branch. Foreign local work remains intact.
Do not push the batch branch again. The final audit descendant changes only
this task record and must be published under audit/release-1.8.1.

## Dependencies and open work

- No schema change or new migration. Model layout metadata is additive.
  Existing line_keys storage and call routing remain compatible. Resync phones
  after saving layouts; review unsupported old overrides explicitly. New
  presets retain their untested physical hardware status.
- Flags identify number ranges, not current locations. Raw national softphone
  numbers without country metadata have no flag. Initial placeholders use a
  300 ms minimum and 180 ms fade; reduced motion skips both.
- Existing 1.8.0 backup API, notification and Dahua follow-ups remain separate.
  The earlier release notes still apply to upgrades from versions before 1.8.0.

## Next action

None for 1.8.1. Release, verification and branch cleanup are complete.
Keep the immutable final audit linked from merged PR48. Preserve closed PR36
and foreign local work. Future contributions need a new release assignment.

## References

- [Integration PR48](https://github.com/itsh-neumeier/talkops/pull/48).
- docs/release-workflow.md; docs/release-manager-prompt.md.
- 20261016-codex-call-flags-skeleton.md; 20261016-codex-yealink-t41p-dss.md.
- 20261016-codex-release-1.8.0.md (completed previous release).

## Final workflow evidence

- [Candidate CI](https://github.com/itsh-neumeier/talkops/actions/runs/38952162964)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38952162953).
- [Exact-main CI](https://github.com/itsh-neumeier/talkops/actions/runs/38956465215)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38956465225).
- [Release](https://github.com/itsh-neumeier/talkops/actions/runs/38958752273),
  [tag CI](https://github.com/itsh-neumeier/talkops/actions/runs/38958752284),
  [tag E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38958752093),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/38958752047).
- All final runs passed on their expected immutable heads. Main and the
  published tag still target 86772ec0c31990f7a29a639ffb90db6ebef217e2.
- The final audit descendant changes only this task record. It is not a new
  candidate and does not change main or the published release tag.
