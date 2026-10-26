# Side panel layout, settings search layer and fixed footer

## Task state

- Task ID: 20261024-claude-panel-layout-footer
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: `claude/wonderful-bohr-tmurvt`
- Base commit: `8f32ec7` (release 1.8.10 merge)
- Related commits or PR: Pending

## Goal

User reports with screenshots (wide screen, about 2000 px):

1. The side panel squeezes the lists of extensions, users and phones.
2. The network tab of the phone panel uses two cramped columns.
3. The settings search results are behind the page content.
4. New: a fixed footer with the copyright and the system load. The user
   said that more requests follow.

## Result

1. The page had `lg:pr-[26rem]`, but the page is inside the centred
   `max-w-7xl` column. Now `.app-body:has(.side-panel)` in `app.css` keeps
   the panel width free on the app body. The centred column moves left; on
   very wide screens the list keeps its full width. The per-page padding is
   removed.
2. `PhoneNetworkCard` uses `@lg:grid-cols-2`. The nearest query container
   was `main`, not the panel. `SidePanel` is now a `@container`.
3. The settings navigation is `lg:sticky`, which makes a stacking context
   without a z-index. Later page content (cards with `.scroll-x`) painted
   above the results. `lg:z-10` on the navigation column fixes this.
4. `AppFooter.svelte`: fixed, `--footer-h: 2rem`. It shows
   "© <year> ITSH Neumeier · TalkOps <version>" for every signed-in user. With
   `diagnostics.manage` it also shows CPU, load 1/5/15, RAM and uptime from
   the new `GET /api/v1/system/load`, every 15 s while the page is visible.
   Values are amber from 80 % and red from 95 %.

## Changes

- `crates/talkops-core/src/system_load.rs`: new; `/proc/stat` CPU delta,
  `/proc/loadavg`, memory (cgroup limit or host), `/proc/uptime`.
  `parse_meminfo` and `memory` moved here from the media worker.
- `crates/talkops-media-worker/src/resources.rs`: uses the core `memory`.
- `crates/talkops-api/src/routes/diagnostics.rs`: `system_load` route.
- `crates/talkops-api/tests/diagnostics.rs`: route and permission test.
- `web/src/routes/+layout.svelte`: `.app-body` wrapper, footer, `main`
  bottom padding, nav heights without the footer.
- `web/src/app.css`: `--side-panel-w`, `--footer-h`, panel rule.
- `web/src/lib/components/SidePanel.svelte`: `@container`, ends above the
  footer on `lg`.
- `web/src/lib/components/SettingsNav.svelte`: `lg:z-10`.
- `web/src/lib/components/Softphone.svelte`: mini softphone above footer.
- `web/src/lib/components/AppFooter.svelte`, `web/src/lib/api.ts`,
  `web/src/lib/i18n/{en,de}.ts`: footer, `SystemLoad`, `footer.*` keys.
- `web/src/routes/{extensions,phones,users}/+page.svelte`: padding removed.
- `tests/browser/side-panel.mjs`: list width beside the panel (1280/2000).
- `tests/browser/settings-search.mjs`: results above the extension list.
  Without the fix it fails ("search results are behind the page content").
- `tests/browser/footer.mjs` (new, added to CI): footer content, position,
  last row not covered, panel above footer, no load without permission.
- `docs/ui-guidelines.md`, `docs/{en,de}/monitoring.md`: documentation.

## Decisions

- Reserve the panel space once in the root layout, not on each page; new
  panel pages need no extra class.
- System load needs `diagnostics.manage` (system information); version and
  copyright are for all signed-in users (`/api/v1/status` is public).
- CPU and load in a container refer to the host (`/proc`); memory uses the
  cgroup limit when one is set. The first sample has no CPU value; the
  footer asks again after 2 s.
- The footer ignores responses without the expected shape, because many
  browser test fixtures answer unknown API paths with `[]`.

## Checks

- Passed: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test -p talkops-core --lib system_load`,
  `cargo test -p talkops-api --test diagnostics`,
  `cargo test -p talkops-media-worker` (local Postgres 16).
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`.
- Passed: all browser suites from the CI list against `npm run preview`.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Not run: full `cargo test --workspace`, Docker stack, SIPp/E2E.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- No migration. The user announced more requests for this branch.

## Next action

Wait for the next user request.

## References

- `docs/ai-memory/20261024-claude-side-panel.md`
- `docs/ui-guidelines.md` (sections "Footer" and "Side panel")
