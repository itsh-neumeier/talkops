# Dashboard lists with three entries and an expand control

## Task state

- Task ID: 20261024-claude-dashboard-collapse
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: `claude/dashboard-collapse` (worktree `../talkops-dash`)
- Base commit: `9f69391`
- Related commits or PR: Pending

## Goal

PR 4 of 6 from one user request. The dashboard cards "Calls in progress"
and "Recent calls" show three entries for now, with an expand control.

## Result

Both cards show three entries and a "Show N more" / "Show less" button.

## Changes

- `web/src/lib/collapse.ts`: `visibleItems` (first entries plus kept
  entries), `readOpen`/`writeOpen` (localStorage with try/catch).
- `web/src/lib/components/ExpandToggle.svelte`: button with
  `aria-expanded` and `aria-controls`.
- `web/src/lib/components/ActiveCalls.svelte`, `web/src/routes/+page.svelte`:
  use the helpers; list IDs `dash-active-list`, `dash-recent-list`.
- i18n `dash.showMore`, `dash.showLess` (EN/DE).
- `web/tests/collapse.test.ts`, `tests/browser/dashboard-collapse.mjs`,
  CI step in `.github/workflows/ci.yml`.
- Docs: `docs/en/active-calls.md`, `docs/de/laufende-gespraeche.md`.

## Decisions

- Limit 3 is the constant `DASH_LIMIT`, so a later change is one line.
- A ringing call stays visible in the collapsed list, so a new call is not
  hidden.
- The recent list still loads 8 calls; the expanded list shows all 8.
- The open state is a per-browser convenience in localStorage, one key per
  card (`talkops.dash.active.open`, `talkops.dash.recent.open`).

## Checks

- Passed: `npm run lint`, `npm run check`, `npm test` (70), `npm run build`.
- Passed: `tests/browser/dashboard-collapse.mjs` (390 px, 1280 px),
  `tests/browser/active-calls.mjs`, `tests/browser/layout.mjs`.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Not run: Rust checks (no Rust change), Docker stack, SIPp/E2E.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- None. PR #72 also changes `web/src/routes/+page.svelte` (call map card);
  a merge conflict is possible but small.

## Next action

None.

## References

- `docs/ai-memory/20261016-claude-dashboard-recent-calls.md`
