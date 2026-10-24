# Search field in the settings

## Task state

- Task ID: 20261024-claude-settings-search
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: `claude/settings-search` (worktree `../talkops-search`)
- Base commit: `9f69391`
- Related commits or PR: Pending

## Goal

PR 3 of 6 from one user request: a search in the settings, like the search
of Android settings, to find a function.

## Result

A search field above the settings navigation (settings page and setup
pages). It finds sections, cards (headings) inside sections, system tabs,
setup pages and their tabs. Enter or click opens the target and marks the
card for 2 s.

## Changes

- `web/src/lib/settingsIndex.ts`: index built from `sections` and
  `settingsPages` of `SettingsNav.svelte` plus sub-entries (heading keys,
  field label keys, German/English synonyms); `fold`, `searchSettings`,
  `entryUrl`.
- `web/src/lib/components/SettingsSearch.svelte`: ARIA combobox, arrow
  keys, Enter, Esc, `/` and Ctrl+K, results with path line, mark of the
  target heading.
- `web/src/lib/components/SettingsNav.svelte`: wrapper
  `[data-settings-nav]` with the search and the nav (sticky moved to the
  wrapper).
- `web/src/routes/settings/+page.svelte`: follows `?section=`/`?tab=`
  changes while the page is open.
- `web/src/app.css`: `.search-hit` mark (reduced motion: the global rule
  removes the animation).
- i18n `settings.search.*` (EN/DE); `web/tests/settingsIndex.test.ts`;
  `tests/browser/settings-search.mjs` with a CI step; `tests/browser/layout.mjs`
  finds the content next to the nav wrapper.
- Docs: `docs/en/system-settings.md`, `docs/de/systemeinstellungen.md`.

## Decisions

- Entries use i18n keys. Results follow the UI language and the UI names,
  and a renamed label changes the search too. The unit test checks that
  each key exists in both languages.
- Matching: each query word must start a word of the title, path, field
  labels or synonyms. Umlauts and `ae`/`oe`/`ue`, and `ß`/`ss`, compare
  equal. Title matches rank first.
- The mark finds the heading by its translated text, so components need no
  new IDs.
- The search applies the same permissions as the navigation.

## Checks

- Passed: `npm run lint`, `npm run check`, `npm test` (73), `npm run build`.
- Passed: browser `settings-search.mjs` (390 px, 1280 px, user without
  permissions), `layout.mjs`, `settings-system.mjs`, `roles.mjs`.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Not run: Rust checks (no Rust change), Docker stack, SIPp/E2E.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- None. PR #73 renames `wifi.tabUnifi`; the index uses the key, so the
  search follows the new name after both PRs merge.
- A new settings card needs a sub-entry in `settingsIndex.ts` to appear as
  its own result.

## Next action

None.

## References

- `docs/ai-memory/20261016-claude-settings-system-structure.md`
