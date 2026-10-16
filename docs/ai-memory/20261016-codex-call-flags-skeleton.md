# Country flags and visible loading placeholders

## Task state

- Task ID: 20261016-codex-call-flags-skeleton
- Date: 2026-10-16, Europe/Berlin
- Tool: Codex
- Status: Completed locally. PR 46 records final checks and readiness.
- Branch: codex/call-flags-skeleton
- Base commit: 7d7990b99e9f02c126ad3983da633476a22a9708
- Related commits or PR: implementation `3f122d4`;
  [PR 46](https://github.com/itsh-neumeier/talkops/pull/46).

## Goal

Show country flags for outbound destinations and inbound callers in recent
calls, active calls and the call list. Make skeleton loading visible and make
the change from placeholders to content smooth.

## Result

The user reviewed the proposal and approved implementation. Implemented flags
in the dashboard, call list and softphone recent calls. Added immediate
placeholders, a 300 ms minimum first display and a 180 ms content fade.
Reduced motion removes the minimum interval and animations. Dashboard call
cards load independently. Polling retains content. Filters retain rows with
a loading message. Pagination adds placeholder rows.

The exclusive worktree is
`C:/Users/timo/.codex/worktrees/call-flags-skeleton/talkops`.
Verified the remote default branch and fetched origin before worktree creation.
The original checkout has three untracked Dahua summaries. Preserved them.
A precise Git safe-directory exception permits this network-backed worktree.

## Changes

- `web/src/lib/calls/country.ts`: Resolve international numbers with full
  numbering metadata. Exclude extensions, malformed numbers, shared service
  ranges, hidden numbers and numbers without a resolved country.
- `web/src/lib/components/PhoneNumber.svelte`: Local SVGs and localized country
  labels. Keep display numbers and brackets. Use normalized party metadata
  when the displayed number has another format.
- `CallParty.svelte`, `CallSummary.svelte`, `ActiveCalls.svelte`,
  `Softphone.svelte`, dashboard and call list: Apply flags only to remote
  inbound sources and outbound destinations. Separate loading from emptiness.
- `LoadingContent.svelte`, `Skeleton.svelte`, `SkeletonRows.svelte`,
  `web/src/app.css`: Immediate placeholders and reusable first-load timing.
  Timer cleanup prevents late updates after navigation.
- Call list request generations: Ignore stale filter and pagination results.
  Clear pending filter timers on navigation.
- Softphone: Load history when the identity changes, independently of SIP
  registration. Ignore old identity responses. Keep callback labels intact.
- `web/package.json`, lock file: Pin `libphonenumber-js` 1.13.15 and
  `flag-icons` 7.6.0. No other package upgrades.
- Unit and browser tests: Cover country detection and presentation behavior.
  Update the motion expectation and wait for loaded softphone history.
  Add `call-presentation.mjs` to CI.
- `docs/ui-guidelines.md`, English and German call docs: Describe behavior,
  loading rules, metadata, assets and the raw softphone number limitation.

## Decisions

- Country labels identify the number range, not the caller's current location.
- Use full `libphonenumber-js` metadata to resolve shared codes such as `+1`
  and exclude service ranges. Use local `flag-icons` SVGs. No external lookup.
- Keep backend normalization and API contracts. The softphone endpoint has raw
  records, so its flags require an international `+` or `00` prefix.
- Replace the old invisible 180 ms skeleton phase. The approved 300 ms minimum
  can delay very fast first content. It applies to call content, not polling.
- No migration, routing change, release metadata change or message to another
  chat. Integration and publication belong to the release manager.

## Checks

- Passed: `npm run lint`, `npm run check` (zero errors and warnings),
  `npm test` (28 tests), `npm run build`, `npm audit --omit=dev` (zero findings).
- Passed: `tests/browser/call-presentation.mjs`: immediate visibility, minimum
  duration, fade, DE/CA flags, national display, phone book toggle, filtering,
  pagination, navigation, independent loading, polling, empty/error outcomes,
  softphone, reduced motion, narrow views and both themes.
- Passed: `tests/browser/call-list.mjs`, `motion.mjs`, `active-calls.mjs`.
- Passed: `tests/e2e/softphone-ui.mjs`: deterministic HTTP/SIP fixtures, identity
  selection, audio controls, history, navigation and logout.
- Passed: `tests/browser/layout.mjs` across all main pages, three widths
  and both themes.
- Passed: manual diff review and visual inspection of light/dark screenshots.
- Corrected during checks: an old softphone assertion assumed immediate data.
  It now waits for loaded history before it checks the same count. Local lint
  first failed because checkout files had CRLF; normalized local text without
  adding unrelated file changes. One browser run was interrupted by a rebuild
  of the preview assets; repeated against the completed build.
- Not run locally: Rust, Docker and live FreeSWITCH/SIPp checks. No Rust, SQL or
  routing change. GitHub Actions provide the complete integration checks.
- Not run: CodeRabbit CLI (not installed). Reviewed the diff locally.
- STE review: used repository writing rules. The full official standard was
  unavailable; full language review is pending.

## Dependencies and open work

- Migration IDs: None. No upgrade steps or API compatibility changes.
- New packages: `libphonenumber-js` 1.13.15 and `flag-icons` 7.6.0. They add
  bundled assets and metadata, with no external runtime service.
- No dependency on another PR. Verify final CI on the PR head before handoff.
- Proposed release notes: Add country flags to inbound callers and outbound
  destinations in call views. Show immediate skeleton loading with smooth
  content transitions. Keep call content visible during refreshes.

## Next action

Check remote CI on the final PR revision. Mark the PR ready when applicable
checks pass. The release manager owns integration and publication.

## References

- ../release-workflow.md
- ../ui-guidelines.md
- 20261015-claude-motion-polish.md
- 20261015-claude-call-log-filters.md
- 20261012-codex-active-call-overview.md
