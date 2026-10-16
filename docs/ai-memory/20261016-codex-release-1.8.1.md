# TalkOps 1.8.1 release

## Task state

- Task ID: 20261016-codex-release-1.8.1
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.1
- Base commit: 7d7990b99e9f02c126ad3983da633476a22a9708
- Related PR: https://github.com/itsh-neumeier/talkops/pull/48

## Goal

Integrate all open PRs and publish the user-selected version 1.8.1.
Verify the final candidate, exact main and published images. Preserve foreign
work and the published 1.8.0 tag.

## Result

All current open contributions are integrated with their source history.
PR46 and PR47 are ready, not drafts. Both are from Codex. No Claude Code
contribution is assigned to this batch. None was already in main. No agreed
contribution is omitted or pending. Both source PRs and integration PR48 are merged. Publication is pending
exact-main checks and artifact verification.

| PR | Exact source head | Dependency | Migration ID |
| --- | --- | --- | --- |
| [46](https://github.com/itsh-neumeier/talkops/pull/46) | f81e140b62abec0e709bd737f3d6d7d07fd1e4f2 | None | None |
| [47](https://github.com/itsh-neumeier/talkops/pull/47) | a48a2fade3a851d102a706fef733bd19f821126f | None | None |

## Changes

- PR46: Country flags, bundled numbering data and local SVG assets. Immediate
  call placeholders, separate card loading, refresh retention and stale-result
  protection. Add the call-presentation browser suite.
- PR47: Ten new Yealink desk models and graphical key setup for all 21 desk
  models. Phonebook/internal targets, save/resync, additive model metadata,
  corrected capacities and retained legacy overrides. Add phone-keys tests.
- Preserve final 1.8.0 audit history from audit/release-1.8.0.
- Cargo and web versions become 1.8.1. Add English release and upgrade notes.
  No merge conflict required a product decision or own production code change.

## Decisions

- Use the explicitly requested version 1.8.1. No public endpoint is removed.
- Target main, verified remote default. Merge commits are permitted; no active
  ruleset or main protection was reported. Keep source branches unchanged.
- Keep closed, unmerged PR36 branch claude/vigilant-tesla-ef0027 and reserve
  ADR0033. It is outside the open-PR scope.
- Preserve primary checkout 397fc83 and its three untracked Dahua summaries.
  Do not read foreign chats or import uncommitted changes.
- Only libphonenumber-js 1.13.15 and flag-icons 7.6.0 are new pinned packages,
  from PR46. The release lock changes must be version-only against the merged
  source baseline. No unrelated dependency upgrade.

## Checks

- Passed at inventory: 12 exact-head checks per source PR. PR46 CI38946303569,
  E2E38946303551, push CI38946297378/E2E38946298448. PR47 CI38949067742,
  E2E38949067882, push CI38949060274/E2E38949060264.
- Passed: cargo fmt; 13 provisioning tests and doc tests; npm ci/lint/check,
  31 web tests and production build. Svelte reports zero errors and warnings.
  All four Compose configurations pass without a Docker Linux daemon.
- Passed: 45 unchanged unique ordered migrations, 1,975 matching EN/DE keys
  and placeholders with no duplicates, all 12 browser suites retained once,
  both source ancestors and version-only metadata against merged baseline
  b747d9fb092f78b935afb8de67710edf0b6c1720.
- Passed: final candidate CI/E2E. Exact-main CI/E2E and publication remain
  pending. Immutable workflow run IDs and the first failure are below.
- Cargo metadata offline first stopped because a locked cross-platform crate
  was not cached. Online metadata downloaded the existing locked crates.
  Cargo.lock changes only the six workspace package versions.
- Not available locally: full workspace/DB/LDAP and Docker Linux stack.
  Windows GNU lacks dlltool.exe; the Docker Linux daemon is unavailable.
  Linux CI supplies the required evidence. FreeSWITCH sources are unchanged.
- Manual diff review: API/UI layout fields agree; legacy key preservation
  keeps tenant-scoped access and existing permission/audit checks. Phonebook
  selection copies values. Both added browser suites remain in CI.
- CodeRabbit CLI is unavailable. Do not claim that a CodeRabbit review ran.
- STE review: Manual clarity review. Full official standard unavailable;
  full language review remains separate from functional checks.

## Dependencies and open work

- No schema change or new migration. All 45 released migrations must remain
  unchanged. New model metadata is additive; line_keys storage stays.
- Resync phones after saving. Review unsupported old overrides explicitly.
  Physical phone/firmware tests are not available here. New presets retain
  their untested hardware status.
- Existing 1.8.0 backup API, notification and Dahua follow-ups remain separate.
- Initial placeholder minimum: 300 ms, fade: 180 ms; reduced motion skips both.
  Flags indicate number ranges, not current locations. Raw national softphone
  numbers without country metadata have no flag.

## Next action

Wait for exact-main CI38956465215 and E2E38956465225. Recheck current main,
scope, versions and unused tag. Push one v1.8.1 tag to the tested main commit.
Verify Release, GitHub release and all three two-platform image manifests.

## References

- docs/release-workflow.md; docs/release-manager-prompt.md.
- 20261016-codex-call-flags-skeleton.md.
- 20261016-codex-yealink-t41p-dss.md.
- 20261016-codex-release-1.8.0.md (completed previous release).

## Candidate and main checkpoint

- Final candidate: 5260739be6d530d6d4f8fa76d122ea0efaa9249c.
  All 12 checks pass. PR CI38952162964/E2E38952162953 and push
  CI38952157423/E2E38952157572.
- E2E38952157572 attempt1 failed at ring-group voicemail storage. Both calls
  reached vm_deposit and recording. The second message was not stored within
  the 15-second count window. No voicemail, ring-group or SIPp scenario code
  changed in this batch. The parallel PR E2E passed all scenarios on attempt1.
  Push E2E attempt2 passed without code, test or timeout changes. This is
  evidence of a sporadic failure; its precise cause is not established.
  No test was weakened and no gate was waived.
- CI logs confirm model maps, last-key rendering, all 21-model API/browser
  cases, retained legacy overrides and call presentation. Server/worker smoke
  tests report 1.8.1. Full SIPp, recording, video and WebRTC scenarios passed.
- PR48 merged at 2026-10-16T19:38:58Z. Release main commit:
  86772ec0c31990f7a29a639ffb90db6ebef217e2.
  Its tree equals the tested candidate tree. All agreed heads are ancestors.
- Source PRs46 and47 auto-closed as merged at 19:39:00Z. The open-PR list is
  empty. Main CI38956465215 and E2E38956465225 are in progress.
- Do not push the batch branch after merge. Local checkpoint commits change
  only this task record. Archive the final record under audit/release-1.8.1
  after publication, without changing released code or creating another release.
