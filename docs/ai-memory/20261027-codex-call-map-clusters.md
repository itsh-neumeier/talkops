# Call map marker bundles

## Task state

- Task ID: 20261027-codex-call-map-clusters
- Date: 2026-10-27, Europe/Berlin
- Tool: Codex
- Status: Completed; submitted for review
- Branch: `codex/call-map-clusters`
- Base commit: `3603ed56b0073a1dd23646622cf4fb0284644ab7` (verified remote main)
- Related commits or PR: [PR #86](https://github.com/itsh-neumeier/talkops/pull/86),
  implementation `7b095b21fb963071a24b039d8a9103ad87f14e8c`.

## Goal

Prevent phone markers from overlapping. The user allowed bundles that split
on zoom or smaller markers. Commit, push and PR creation remain authorized.

## Result

Added screen-space bundles with a category pie and group count. Each group
remains available through a preview list, including groups at equal positions.
The map rebuilds bundles after zoom, movement, size, category and data changes.
Implementation is committed and pushed to PR #86. The PR holds the current
review and CI state. No merge, release version change, tag or publication was done.

## Changes

- `web/src/lib/calls/mapClusters.ts`: footprint collision checks and category sums.
- `web/src/lib/components/CallMap.svelte`: bundles, member previews and zoom action.
- `web/src/app.css`, both locale files: bundle styles and translated controls.
- Web unit tests and `tests/browser/call-map.mjs`: dense positions, zoom,
  category filtering, touch, keyboard, themes and responsive layouts.
- `docs/en/call-log.md`, `docs/de/anrufliste.md`: bundle instructions.

## Decisions

- Use a new worktree at `C:/Users/timo/.codex/worktrees/call-map-clusters/talkops`.
  PR #84 is already merged. Preserve the old feature worktree and the three
  untracked Dahua files in the primary checkout.
- Bundle intersecting marker footprints, including labels and focus spacing.
  Recheck merged centroids so the bundles cannot overlap other markers.
- Remove fixed country offsets. Country groups at the same position use a
  bundle. Keep API coordinates, IDs, call filters and access scopes unchanged.
- No new map provider, package, migration or API contract is required.
- Keep device shapes and preselection previews from ADR 0047.

## Checks

- Passed: `npm ci`, `npm run lint`, `npm run check` (zero errors/warnings),
  `npm test` (91 tests) and `npm run build`.
- Passed: production `tests/browser/call-map.mjs` with Playwright and Edge.
  Includes both themes at 360, 768 and 1280 px, disjoint marker boxes,
  mounted-map resize, keyboard focus/activation/Escape, touch, equal positions, maximum zoom,
  category filtering, dashboard bundles and existing map checks.
- Passed: 13 handbook tests and Wiki check (75 pages, 688 local links).
- Passed: Git whitespace checks and visual inspection of the production
  dense-map capture with test data. Main checkout and foreign files are unchanged.
- GitHub checks at handoff preparation: handbook, Compose, fax fixtures and
  Coral tools passed on the implementation commit. Web, Rust, image and SIPp
  jobs were still in progress. Check live results on the final PR revision.
- Passed: local diff review. CodeRabbit CLI is absent; no CodeRabbit review
  result is claimed. Rust/SIPp tests are not run locally for this UI-only change.
- Environment issue resolved: a browser check used an old Vite preview process
  after a rebuild. Restarting the preview loaded the current assets and passed.
- Corrected test expectation: nearby places can stay grouped at the offline
  zoom limit. Tests verify that groups split and all members stay available.
- Manual density check: 2,000 projected groups completed in about 17 ms with
  the screen grid. This is a local observation, not a timing guarantee.
- STE review: manual review against `docs/documentation-style.md`.
  Full standard unavailable; full language review pending.

## Dependencies and open work

- Builds on merged PR #84, included in the base commit.
- Migration IDs: None. No dependency or release version changes.
- Release manager owns integration, release versions, tags and publication.

## Next action

Release manager: inspect PR #86 and its final CI results. Integrate the fix
through the release workflow, then manage release versions, tags and publication.

## References

- `docs/ai-memory/20261027-codex-map-emergency-proposal.md`
- ADR 0047
- `docs/release-workflow.md`
