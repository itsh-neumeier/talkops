# Call map design and emergency alert proposal

## Task state

- Task ID: 20261027-codex-map-emergency-proposal
- Date: 2026-10-27, Europe/Berlin
- Tool: Codex
- Status: In progress. Proposal is complete; user review precedes implementation.
- Branch: `codex/map-emergency-proposal`
- Base commit: `f8bdfb37cf3f7705d7fbbaf63d6bd6148caba400` (remote main, 1.8.11)
- Related commits or PR: Pending draft PR creation.

## Goal

Propose map previews, distinct mobile markers and configurable emergency
email alerts. Obtain user feedback before application changes.

## Result

Prepared a technical proposal and two interactive marker designs. Added a
sample emergency alert editor and email preview. The browser checks pass.
Application code, dependencies, database migrations and release metadata
are unchanged. Commit, push and a draft PR are authorized.

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
- The user explicitly requires feedback before implementation. Only review
  material is changed. Keep the PR in draft state.
- Recommend a phone frame with a round pie in the display. Offer a mobile
  tile as an alternative. Keep local area markers circular.
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
- Passed: manual screenshot inspection of the light map and dark mobile
  settings. Preview, labels and controls fit the inspected surfaces.
- Passed: `node tests/docs/wiki.mjs` (13 tests).
- Passed: `node scripts/docs/build-wiki.mjs --check` (75 pages, 672 links).
- Passed: `git diff --check` before commit.
- Resolved environment failure: default Playwright browser was absent.
  The same checks passed with the installed Edge executable override.
- Not run: Rust, production web build, database and SIP/SMTP integration
  checks. This contribution changes review files only. The proposal lists
  the required checks for implementation.
- STE review: manual review against `docs/documentation-style.md`.
  Full ASD-STE100 standard unavailable; full language review pending.

## Dependencies and open work

- Existing map and notification features are included in the base commit.
- Open PRs at inspection: #61 (Wi-Fi proposal), #81 and #82 (Coral). No
  dependency identified. Recheck remote state before implementation.
- Migration IDs: none. A partial event-deduplication index is proposed.
  Allocate a unique forward migration ID when implementation starts.
- Pending user feedback on marker design and alarm behavior. No product
  feature is implemented or ready for release.

## Next action

Publish the proposal as a draft PR and ask for review. After user approval,
implement the accepted design, add feature tests and bilingual user docs,
run relevant CI checks and update the PR. The release chat owns integration,
version changes, tags and publication.

## References

- `docs/ai-memory/20261023-claude-call-map.md`
- `docs/ai-memory/20261016-claude-notifications.md`
- ADR 0035, ADR 0044 and ADR 0045
- `docs/release-workflow.md`
