# TalkOps 1.8.1 release

## Task state

- Task ID: 20261016-codex-release-1.8.1
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.1
- Base commit: 7d7990b99e9f02c126ad3983da633476a22a9708
- Related commits or PR: Pending

## Goal

Integrate all open PRs and publish the user-selected version 1.8.1.
Verify the final candidate, exact main and published images. Preserve foreign
work and the published 1.8.0 tag.

## Result

All current open contributions are integrated with their source history.
PR46 and PR47 are ready, not drafts. Both are from Codex. No Claude Code
contribution is assigned to this batch. None was already in main. No agreed
contribution is omitted or pending. Publication is pending validation.

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
- Pending: combined local checks; final candidate and exact-main CI/E2E;
  publication workflow, GitHub release and three two-platform manifests.
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

Run combined checks. Publish the integration PR and wait for final-head CI/E2E.
Merge, verify exact main, then push one unused v1.8.1 tag and verify artifacts.

## References

- docs/release-workflow.md; docs/release-manager-prompt.md.
- 20261016-codex-call-flags-skeleton.md.
- 20261016-codex-yealink-t41p-dss.md.
- 20261016-codex-release-1.8.0.md (completed previous release).
