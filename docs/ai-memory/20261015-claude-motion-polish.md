# Design review: loading states, animations and fades

## Task state

- Task ID: 20261015-claude-motion-polish
- Date: 2026-10-15, Europe/Berlin
- Tool: Claude Code
- Status: Completed. The PR records the readiness.
- Branch: claude/motion-polish
- Base commit: c783658 (claude/transcription-jobs, itsh-neumeier/talkops#31)
- Related commits or PR: Pending

## Goal

PR 4 of 4 of the approved call log feedback plan (see
`20261015-claude-call-log-filters.md`). The user reported that the
skeleton loading is so fast that it has no effect, and asked for a general
design review: animations and fades must be consistent and attractive.

## Result

Implemented, tested and documented on the feature branch.

## Changes

- `web/src/app.css`: motion tokens (`--ease-out`, `--dur-fast`, `--dur`,
  `--dur-slow`, `--skeleton-delay`, skeleton colors for both themes);
  `.skeleton` (delayed fade-in) and `.skeleton-bar` (light sweep);
  `.page-in`, `.menu-in`, native dialog motion; transitions for `.btn`
  (with press feedback), `.input`, `.nav-item`; the animations of PR 1 and
  PR 3 use the tokens; a global `prefers-reduced-motion` rule.
- `web/src/lib/components/Skeleton.svelte`, `SkeletonRows.svelte`: new
  classes instead of `animate-pulse`.
- `web/src/routes/+layout.svelte`: page content in a keyed `.page-in`
  wrapper; the mobile menu slides in; the initial skeleton uses the classes.
- `web/src/lib/components/Modal.svelte`: backdrop fade and panel fly
  transitions (in and out).
- `web/src/lib/motion.ts`: `motion(ms)` gives 0 with reduced motion.
- `web/src/lib/components/Tabs.svelte`: color transition.
- `tests/browser/motion.mjs` (added to CI), `docs/ui-guidelines.md`.

## Decisions

- Cause: skeletons were visible from the first frame and replaced after a
  few milliseconds, which looks like a flicker. Fix: skeletons stay
  invisible for 180 ms; fast answers show the content with a fade, slow
  answers show a calm skeleton with a sweep. No minimum display time (it
  would hold back data that is already there).
- One curve and three durations for the whole UI.
- The page wrapper is keyed by the path, not the query string, so filters
  in the address (call log) do not restart the page.

## Checks

- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`.
- Passed: browser checks `motion.mjs`, `transcriptions.mjs`,
  `call-list.mjs`, `roles.mjs`, `layout.mjs`, `active-calls.mjs`,
  `transcription-settings.mjs`.
- Not run: Rust checks are not affected (no Rust change in this PR).
- STE review: full standard not available; language review pending.

## Dependencies and open work

- Based on itsh-neumeier/talkops#31 (which contains #29 and #30) because
  it also unifies the animations of those PRs. No migration.

## Next action

Release manager: integrate after #29, #30 and #31.

## References

- `docs/ui-guidelines.md`
- `docs/ai-memory/20261013-claude-ui-design-review.md`
