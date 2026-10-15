# Call log: filters, sort order, phone book names and answering extension

## Task state

- Task ID: 20261015-claude-call-log-filters
- Date: 2026-10-15, Europe/Berlin
- Tool: Claude Code
- Status: Completed. The PR records the readiness.
- Branch: claude/gifted-rubin-my4qq1
- Base commit: 702507b (origin/main, TalkOps 1.7.13)
- Related commits or PR: Pending

## Goal

The user sent a screenshot of the call log with change requests. The user
approved a proposal with four separate PRs, one for each feature:

1. Call log (this task): a check box "Show numbers through the phone book"
   (default on), column filters in popup panels, sort by time and duration,
   and a better "To" column for inbound calls: `Ext. 101 via +4995431241`
   with the extension name.
2. Roles and permissions with a check box editor (one role for each user;
   no permissions for each user).
3. "Transcribe again" with a model selection popup and a time estimate,
   and a transcription jobs page (progress, ETA, pause, resume, stop,
   resource usage). Pause must continue at the paused position if possible.
   Users see their own jobs; managers see all jobs.
4. Design review: skeleton loading (shows too briefly), animations, fades.

## Result

Implemented, tested and documented on the feature branch.

## Changes

- `crates/talkops-core/src/cdr.rs`: `CdrQuery` gets `offset`, `since`,
  `until`, `direction`, `result`, `from`, `to`, `recording`, `sort`,
  `order`. New `CdrScope` (own extensions, phone book numbers),
  `list_scoped`, `count`, `BUSY_CAUSES`. The SQL uses `QueryBuilder`.
- `crates/talkops-api/src/active_calls.rs`: `Directory::load`,
  `Directory::contact_numbers`, `Directory::record_parties`.
  `external_party` and `external` are public in the crate.
- `crates/talkops-api/src/routes/settings.rs`: `GET /api/v1/calls` returns
  `CallEntry` (CDR fields plus `caller`, `called`, `via_number`) and the
  header `X-Total-Count`. Users are limited with one query
  (`extension_id = ANY(...)`) instead of one query for each extension.
- `web/src/lib/calls/filters.ts`: filter state, page address, API query.
- `web/src/lib/components/ColumnFilter.svelte`: filter button and popup
  panel in the top layer (popover API).
- `web/src/routes/calls/+page.svelte`: new call log.
- `web/src/lib/api.ts`: `api.getPage` reads `X-Total-Count`.
- `web/src/app.css`: `.popover-panel`, `.row-in`, `.expand-in` animations
  with `prefers-reduced-motion` support.
- `web/src/lib/i18n/{en,de}.ts`: new keys `calls.*`, `filter.*`.
- Tests: `crates/talkops-core/tests/domain.rs` (`cdr_filters_sort_and_scope`),
  unit test in `active_calls.rs`, `web/tests/callFilters.test.ts`,
  `tests/browser/call-list.mjs` (added to `.github/workflows/ci.yml`).
- Docs: `docs/en/call-log.md`, `docs/de/anrufliste.md`, links in
  `docs/en/first-steps.md` and `docs/de/erste-schritte.md`.

## Decisions

- Server-side sort and filter with offset pages ("Load more"), because a
  client sort is wrong with more than one page.
- The response stays a JSON array (dashboard and other clients stay
  compatible). The total count is in the `X-Total-Count` header.
- Phone book name filter: the API finds the matching contacts in the
  directory and gives their numbers (raw and normalized) to SQL.
- Result "busy/declined" = not answered with `USER_BUSY` or
  `CALL_REJECTED`. "Missed" excludes these causes.
- Inbound "answering extension" = `dest_extension_id`, else `extension_id`
  (same rule as active calls).
- The phone book setting is in `localStorage` (`talkops.calls.phonebook`).
- Filters and sort order are in the page address (`goto` with
  `replace` and `shallow`).
- The recording button shows only the icon below the `@3xl` container
  width, so all columns fit next to the side menu.

## Checks

- Passed: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `DATABASE_URL=… cargo test --workspace` with local PostgreSQL 16.
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build` in `web/`.
- Passed: `tests/browser/call-list.mjs`, `layout.mjs`, `active-calls.mjs`,
  `transcription-settings.mjs` against `vite preview` with Playwright
  1.56.1 and the local Chromium.
- Not run: Docker/SIPp end-to-end test (`tests/e2e/run.sh`). The change does
  not touch FreeSWITCH or call routing.
- STE review: full standard not available; language review pending.

## Dependencies and open work

- No migration.
- Open: PRs 2 to 4 of the approved plan (see Goal).

## Next action

Release manager: review and integrate the PR. Claude Code: continue with
the roles and permissions PR.

## References

- `docs/ai-memory/20261015-claude-shared-phonebook-numbers.md`
- `docs/adr/0027-shared-phonebook-numbers-in-active-calls.md`
