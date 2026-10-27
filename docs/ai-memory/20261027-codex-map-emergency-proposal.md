# Call map design and emergency alert proposal

## Task state

- Task ID: 20261027-codex-map-emergency-proposal
- Date: 2026-10-27, Europe/Berlin
- Tool: Codex
- Status: In progress. User approved implementation; final checks are running.
- Branch: `codex/map-emergency-proposal`
- Base commit: `f8bdfb37cf3f7705d7fbbaf63d6bd6148caba400` (remote main, 1.8.11)
- Related commits or PR: [Draft PR #84](https://github.com/itsh-neumeier/talkops/pull/84),
  proposal commit `817b2955da1a061bd9d9853de0c77af7f4dc14ac`.

## Goal

Implement the approved device markers, map previews and configurable emergency
email alerts. User approval on 2026-10-27 completed the design review checkpoint.

## Result

Prepared a technical proposal and two interactive marker designs. Added a
sample emergency alert editor and email preview. The browser checks pass.
The proposal is committed, pushed and published as draft PR #84. Product code
now adds device markers, previews, split country groups with legacy aliases,
and an opt-in emergency dialing event. Final check results and publication of
the implementation are pending. Dependencies and release metadata are unchanged.

## Changes

- `docs/proposals/call-map-and-emergency-alert.md`: current behavior, proposed
  behavior, compatibility, implementation steps and acceptance checks.
- `docs/mockups/`: interactive review fragment, screenshots and test commands.
- `tests/browser/call-map-emergency-mockup.mjs`: mockup interactions,
  responsive layouts and no-network checks.
- This task summary records the review checkpoint.

## Decisions

- Use a separate worktree at
  `C:/Users/timo/.codex/worktrees/map-emergency-proposal/talkops`.
  The original checkout is on main at `397fc83` and has three unrelated
  untracked Dahua summaries. Do not change or copy those files.
- The user first required feedback, then approved implementation. Keep the
  PR in draft state until the implementation and relevant checks are complete.
- User feedback on 2026-10-27 supersedes the initial circular local markers
  and mobile tile alternative. Use desk phones with a handset and keypad
  for local areas, and a smartphone for mobile groups. Keep a circular pie
  inside each display. The final label correction is "DE ohne Ortsbezug".
- The updated mockup compares two densities of these device shapes: counts
  beside each marker when space permits, or counts in the hover/focus preview.
  Narrow screens use the compact density. Do not infer connecting call routes
  from the reference image. No relationship data exists in the aggregate API.
- The user approved the revised design and alarm in the next message.
- Propose separate country groups for mobile and other unlocated calls.
  Preserve old combined map-to-list filter links. Add an ADR after approval
  because this changes one grouping decision in ADR 0044.
- Reuse the notification rules and templates in ADR 0035. Propose an opt-in
  critical event for authenticated extension emergency dialing attempts.
- Propose durable event deduplication by tenant and originating call UUID.
  Do not suppress distinct calls through delays, digests or hourly limits.
  Do not promise exactly-once SMTP delivery or general retries.

## Checks

- Passed: `node tests/browser/call-map-emergency-mockup.mjs`, with bundled
  Playwright and installed Microsoft Edge. Twelve map variant/theme/width
  cases, six settings cases and touch preview/details passed. No runtime
  errors or network requests. Widths: 1024, 736 and 320 px.
- Passed: updated browser checks after the second user reference. Desk-phone
  shapes, nine-key layouts, smartphone shape, country label and count density
  checks pass in all twelve map cases.
- Passed: manual screenshot inspection of the updated light/dark maps and
  dark mobile settings. Preview, labels and controls fit the inspected surfaces.
- Passed: `node tests/docs/wiki.mjs` (13 tests).
- Passed: `node scripts/docs/build-wiki.mjs --check` (75 pages, 672 links).
- Passed: `git diff --check` before commit.
- Resolved environment failure: default Playwright browser was absent.
  The same checks passed with the installed Edge executable override.
- Implementation checks passed so far: production web check, unit tests,
  build and lint; map browser checks; four PostgreSQL map integration tests,
  ten existing dialing tests and two notification integration tests.
- Further checks are running: updated layout coverage, notifier SMTP delivery,
  the storage-failure routing case, final migration ID and workspace clippy.
- Local environment: MSVC Rust toolchain; private PostgreSQL 17 data directory
  `A:/aislop/codex/talkops/map-emergency-pg`, port 55439. Verified with
  `SHOW data_directory`. Task build cache is on A: after local disk exhaustion.
  The GNU toolchain lacks gcc/dlltool. Use `cargo +stable-x86_64-pc-windows-msvc`
  with `CARGO_TARGET_DIR=A:/aislop/codex/talkops/map-emergency-target` and
  `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`.
- STE review: manual review against `docs/documentation-style.md`.
  Full ASD-STE100 standard unavailable; full language review pending.

## Dependencies and open work

- Existing map and notification features are included in the base commit.
- Open PRs at inspection: #61 (Wi-Fi proposal), #81 and #82 (Coral). No
  dependency identified. Recheck remote state before implementation.
- Migration ID: `20261113000001_emergency_dial_alert.sql`. Initial local ID
  20261112000001 was changed before publication because PR #81 uses it.
  PR #82 also uses 20261112000002. ADR 0047 avoids Coral's ADR 0046.
- No Docker daemon is available. Full Linux/LDAP/FreeSWITCH/SIPp validation
  needs CI. CodeRabbit CLI is absent; review the diff locally.

## Next action

Finish the pending checks, commit and push implementation, then update PR #84
with final evidence. The release chat owns integration,
version changes, tags and publication.

## References

- `docs/ai-memory/20261023-claude-call-map.md`
- `docs/ai-memory/20261016-claude-notifications.md`
- ADR 0035, ADR 0044 and ADR 0045
- `docs/release-workflow.md`
