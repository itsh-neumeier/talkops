# Phone book: sort order and column filters

## Task state

- Task ID: 20261016-claude-phonebook-sort-filter
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed. The PR records the readiness.
- Branch: claude/phonebook-sort-filter
- Base commit: 0f128ad (origin/main, TalkOps 1.7.14)
- Related commits or PR: Pending

## Goal

The phone book page gets an ascending and descending sort order and column
filters, the same as the call list. The sort header button and the address
helpers become shared parts, and the call list uses them without a change in
behavior.

- Sort by name, company and phone book section.
- Filters: phone book (global and sections, more than one value), number
  present (work, mobile, other), free text.
- The page address keeps the state. The `?contact=` link continues to work.

## Result

Implemented, tested and documented on the feature branch. A stacked PR
(`claude/transcription-history-filter`) uses the shared parts for the
transcription history.

## Changes

- `web/src/lib/list/sort.ts`: `nextSort`, `ariaSort`, `parseOrder`,
  `pickList`, `toggle`, `queryString`, `textCompare`.
- `web/src/lib/list/address.ts`: `syncAddress` (`goto` with `replace` and
  `shallow`).
- `web/src/lib/components/SortButton.svelte`: sort button of a column title
  (was the `sortButton` snippet of the call list).
- `web/src/lib/components/FilterOptions.svelte`: check boxes or radio
  buttons in a `ColumnFilter` panel.
- `web/src/lib/calls/filters.ts`, `web/src/routes/calls/+page.svelte`: use
  the shared parts. No change in behavior.
- `web/src/lib/phonebook/filters.ts`: filter state, page address, match and
  sort functions.
- `web/src/routes/phonebook/+page.svelte`: sort buttons, column filters,
  "n of m" line, "Reset all filters". The section select box is replaced by
  the section column filter.
- `web/src/lib/i18n/{en,de}.ts`: `calls.sortBy` becomes `filter.sortBy`;
  new `contacts.*` keys; `contacts.allSections` removed (not used).
- Tests: `web/tests/phonebookFilters.test.ts`, `tests/browser/phonebook.mjs`
  (added to `.github/workflows/ci.yml`), `tests/browser/active-calls.mjs`
  (the phone book has no section select box now).
- Docs: `docs/en/yealink.md`, `docs/de/yealink.md` (section 5),
  `docs/ui-guidelines.md` (new section "Sort and filter lists").

## Decisions

- The page loads all contacts, so the filter and the sort run in the
  browser.
- Number filter: one filter for each number column with "all", "with
  number" and "without number". This also finds contacts without a mobile
  number. The filters of the columns combine with AND.
- Company order: contacts without a company are at the end in both
  directions. Section order: the global phone book comes first in the
  ascending order. Equal values sort by name. Text compare uses
  `Intl.Collator` with the UI language.
- The free text also finds the section name, and digits-only text finds
  numbers with other formatting.
- A new sort column starts ascending (text columns). The call list keeps
  descending.
- `?contact=<id>`: the entry is highlighted. Only when the current filters
  hide the entry, the page replaces the filters with the section of the
  entry. The old page always selected the section. The parameter stays in
  the address together with the filters. The page handles each linked id
  only once, so a filter change does not reset the filters again.
- The section column is visible on all widths when sections exist, so its
  filter is always available. The *Other* column stays `col-opt-lg`.

## Checks

- Passed: `npm run lint`, `npm run check`, `npm test` (19 tests),
  `npm run build` in `web/`.
- Passed: `tests/browser/phonebook.mjs`, `call-list.mjs`,
  `active-calls.mjs`, `layout.mjs` against `vite preview` with Playwright
  1.56.1 and the local Chromium.
- Not run: Rust checks. The change has no Rust code.
- Not run: Docker/SIPp end-to-end test. The change does not touch call
  routing.
- STE review: full standard not available; language review pending.

## Dependencies and open work

- No migration. No API change.
- Stacked PR: transcription history sort and filters
  (`docs/ai-memory/20261016-claude-transcription-history-filter.md` on its
  branch).

## Next action

Release manager: review and integrate the PR before the stacked PR.

## References

- `docs/ai-memory/20261015-claude-call-log-filters.md`
- `docs/ui-guidelines.md`
