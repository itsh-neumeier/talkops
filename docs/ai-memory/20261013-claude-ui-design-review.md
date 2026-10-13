# Web UI design review: scrollbars, design language and responsive layout

## Task state

- Task ID: 20261013-claude-ui-design-review
- Date: 2026-10-13
- Tool: Claude Code
- Status: Completed
- Branch: `claude/happy-goldberg-4lr4r5` (the session assigned this branch)
- Base commit: `21f5a51` (`origin/main`, TalkOps 1.7.7)
- Related commits or PR: Pending

## Goal

Find the cause of unstyled native scrollbars on some pages. Check that the
web UI uses one design language. Check the responsive layout. The user
approved the proposal with parts A (theme and scrollbars), B (responsive
layout) and C (shared components) in full.

## Result

- Cause 1: the Content Security Policy blocked the inline theme script in
  `web/src/app.html`. The stored dark theme and the OS dark preference were
  lost after each reload.
- Cause 2: `<meta name="color-scheme" content="light dark">` made native
  scrollbars follow the OS, not the theme of the app. A dark app on a light
  OS showed light native scrollbars.
- Cause 3: table cards and the settings strip scrolled sideways with
  unstyled scrollbars, also on tablet width.
- At 768-1023 px, the main menu and the settings menu were two columns
  side by side. The content had approximately 280 px. `/settings` and
  `/trunks/[id]` scrolled sideways.
- Grids without a base column count grew with long words (dashboard at
  360 px).
- All causes are fixed. Shared components and rules are in place.

## Changes

- `web/static/theme-init.js`, `web/src/app.html`: theme script as a file
  (allowed by `script-src 'self'`); color-scheme meta tag removed.
- `web/src/app.css`: `color-scheme` from the theme class, thin themed
  scrollbars, `--header-h`, `.scroll-x` (edge shadows, hidden bar on touch),
  `.nav-item`, `.dialog-panel`, `.col-opt`, `.col-opt-lg`, `td.empty`,
  hyphenation for labels.
- `web/src/routes/+layout.svelte`: fixed header height, `dvh`, `<main>` is
  a size container, shared `.nav-item`, scrollable mobile menu, Escape
  closes it, `aria-expanded` on the menu button.
- `SettingsNav.svelte`, `SettingsShell.svelte`, `settings/+page.svelte`:
  settings menu is a column from `lg` (1024 px), a strip below; the strip
  scrolls the active entry into view.
- New components: `PageHeader.svelte`, `Tabs.svelte` (ARIA tabs, arrow
  keys), `EmptyRow.svelte`, `routes/+error.svelte`.
- All pages use `PageHeader`. Routing, queue and voicemail-text tabs use
  `Tabs`. `Modal` and `LiveTranscript` use `.dialog-panel`.
- Grids in page content use container queries and `grid-cols-1` as base.
- Tables use `.scroll-x`, hide less important columns on narrow content and
  show an empty row when a loaded list is empty.
- Corner radius rule applied (see `docs/ui-guidelines.md`).
- i18n: `common.empty`, `error.*` in `en.ts` and `de.ts`.
- `tests/browser/layout.mjs` and a CI step in `.github/workflows/ci.yml`.
- `docs/ui-guidelines.md`; reference in `CLAUDE.md` (Frontend rule).

## Decisions

- Theme script as a static file instead of a CSP hash: no hash maintenance
  when the script changes.
- Container queries instead of viewport breakpoints for content grids:
  the content width depends on the side menus.
- Tables hide columns on narrow screens instead of a card view per row:
  less code, same data on wider screens. The user accepted this proposal.
- Scrollbars: thin and visible with a pointer device, hidden on touch
  devices; edge shadows show more content.
- The session assigned the branch `claude/happy-goldberg-4lr4r5`, so no
  new `claude/ui-design-review` branch was created.

## Checks

- Passed: `npm run lint`, `npm run check` (0 errors, 0 warnings),
  `npm test`, `npm run build`.
- Passed: `node ../tests/browser/layout.mjs` against `vite preview`
  (20 pages, 360/768/1280 px, light and dark, theme reload, 404 page).
- Passed: `tests/browser/active-calls.mjs` and `tests/recovery/browser.mjs`
  against the new build.
- Not run: Rust checks and e2e SIPp tests. The task changes no Rust code.
- STE review: the full ASD-STE100 standard was not available; review pending.

## Dependencies and open work

- No migrations. No API change. No dependency change.
- Detail pages with complex forms (`/phones/[id]`, `/extensions/[id]`) were
  checked by code review only. The layout test has no fixtures for them.

## Next action

Release manager: review and integrate the PR.

## References

- `docs/ui-guidelines.md`
- `docs/ai-memory/20261012-codex-active-call-overview.md` (earlier mobile
  width fix of the header)
