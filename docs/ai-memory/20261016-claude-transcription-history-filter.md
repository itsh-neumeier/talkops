# Transcription history: parties, sort order and filters

## Task state

- Task ID: 20261016-claude-transcription-history-filter
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed. The PR records the readiness.
- Branch: claude/transcription-history-filter (stacked on
  claude/phonebook-sort-filter)
- Base commit: c8a4d5b (head of claude/phonebook-sort-filter, PR #39)
- Related commits or PR: #42 (stacked on #39)

## Goal

The history tab (7 days) of the transcriptions page gets a sort order and
filters. The page uses the shared parts of PR #39.

- Sort ascending and descending by time, duration and status.
- Filters: status (the real final states), model, direction, date range in
  the 7 days, free text on the parties.
- Show the parties with the phone book names (`CallParty`), when the API
  can supply them cheaply.
- Filter in the browser when the 7-day list is complete; keep the state in
  the page address.

## Result

Implemented, tested and documented on the feature branch.

## Changes

- `crates/talkops-core/src/transcription_jobs.rs`: `JobView` gets
  `caller_name`, `direction` and the private fields `extension_id`,
  `dest_extension_id`, `number_id` (`#[serde(skip)]`). The voicemail caller
  fills `caller_number` and `caller_name`. New `HISTORY_LIMIT` (1000, was
  200).
- `crates/talkops-api/src/active_calls.rs`: `Directory::job_parties` and
  `JobParties`. A recorded call uses `record_parties` (same as the call
  log). A voicemail from an extension is internal, else inbound.
- `crates/talkops-api/src/routes/transcription_jobs.rs`: response
  `JobEntry` (`JobView` and `JobParties`, flattened). Only the history
  loads the `Directory`.
- `crates/talkops-api/tests/transcription_jobs.rs`: new `#[sqlx::test]`
  `history_resolves_the_parties_through_the_phone_book`.
- `web/src/lib/transcriptions/history.ts`: filter state, page address,
  match and sort functions.
- `web/src/lib/list/sort.ts`: `digitsMatch` (shared with the phone book;
  `0911 2193` finds `+49 911 2193…`).
- `web/src/routes/transcriptions/+page.svelte`: history table with
  `SortButton`, `ColumnFilter`, `FilterOptions`, `CallParty`, search field,
  "n of m" line. The tab is in the address (`?tab=history`). The history
  reloads every 30 s (was 2 s).
- `web/src/lib/api.ts`, `web/src/lib/i18n/{en,de}.ts`: types and new
  `tx.*` keys.
- Tests: `web/tests/transcriptionHistory.test.ts`,
  `tests/browser/transcriptions.mjs` (history checks).
- Docs: `docs/en/transcriptions.md`, `docs/de/transkriptionen.md`,
  `docs/ui-guidelines.md`.

## Decisions

- Real history states: `done`, `failed`, `cancelled` (the history has no
  running or queued jobs).
- The history query keeps one SQL statement; the API resolves the parties
  from the loaded `Directory` (7 small queries). The active list refreshes
  every 2 s and does not resolve the parties.
- Client-side filter and sort: the history is limited to 7 days and 1000
  jobs. The page shows "only the newest 1000" when the limit applies.
- "Duration" is the processing time shown in the column. Jobs without a
  processing time sort last in both directions. The status order is done,
  failed, stopped. Equal values sort by time, newest first.
- The date filter uses the shown time (end of the job, else creation) and
  local days. The date inputs allow the last 7 days.

## Checks

- Passed: `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace` with local PostgreSQL 16: 325 passed,
  0 failed, 1 ignored.
- Passed: `npm run lint`, `npm run check`, `npm test` (23 tests),
  `npm run build` in `web/`.
- Passed: `tests/browser/transcriptions.mjs`, `phonebook.mjs`,
  `call-list.mjs`, `active-calls.mjs`, `layout.mjs`, `motion.mjs` against
  `vite preview` with Playwright 1.56.1 and the local Chromium.
- Not run: Docker/SIPp end-to-end test. The change does not touch call
  routing or the media worker.
- STE review: full standard not available; language review pending.

## Dependencies and open work

- Merge after PR #39 (stacked).
- No migration. API: additive fields in `GET /api/v1/transcription-jobs`;
  the history limit changes from 200 to 1000 jobs.

## Next action

Release manager: review and integrate after PR #39.

## References

- `docs/ai-memory/20261016-claude-phonebook-sort-filter.md`
- `docs/ai-memory/20261015-claude-transcription-jobs.md`
- `docs/adr/0030-transcription-jobs-control.md`
