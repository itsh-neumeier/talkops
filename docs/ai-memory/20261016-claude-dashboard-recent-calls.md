# Dashboard recent calls with resolved parties

## Task state

- Task ID: 20261016-claude-dashboard-recent-calls
- Date: 2026-10-16
- Tool: Claude Code
- Status: Completed
- Branch: claude/dashboard-recent-calls
- Base commit: 0f128ad
- Related commits or PR: Pending

## Goal

The dashboard card "Recent calls" must show the parties in the same format
as "Calls in progress": phone book name link, number in brackets, "+N more"
for numbers in more than one entry, and the local extension. The row must also
show the own number of the call (dialed number for inbound calls, caller ID
number for outbound calls).

## Result

Completed locally. The branch is pushed and a PR is open (see PR link in the
transfer message).

## Changes

- `crates/talkops-api/src/active_calls.rs`: `record_parties` now also returns
  the own number for outbound calls. The value comes from `Cdr::number_id`.
  The outbound dialplan sets `talkops_number_id`, and the CDR writer stores it.
  Records without a number, or with a deleted number, return `None`.
  The unit test covers inbound, outbound, missing number, unknown number and
  internal calls.
- `crates/talkops-api/src/routes/settings.rs`, `web/src/lib/api.ts`: updated
  the `via_number` documentation.
- `web/src/lib/components/CallSummary.svelte`: new shared two-line party
  summary. It takes direction, caller, called and an optional own number.
- `web/src/lib/components/ActiveCalls.svelte`: uses `CallSummary` without an
  own number. The output does not change.
- `web/src/routes/+page.svelte`: the recent calls card uses `CallSummary`.
  Icon, time, duration and the red missed state stay. A fallback party from
  the raw CDR fields applies when the API sends no resolved party.
- `tests/browser/layout.mjs`: the `/api/v1/calls` fixture has resolved
  parties, an ambiguous number and an outbound missed call with long names.
- `docs/en/active-calls.md`, `docs/de/laufende-gespraeche.md`: new section
  "Recent calls" / "Letzte Anrufe".

## Decisions

- No new i18n keys. The rows reuse `dash.call.*` and `calls.via`, so the
  dashboard and the call log use the same words.
- Internal calls use the two-line layout of active calls. This keeps one
  shared component.
- The call log page (`web/src/routes/calls/+page.svelte`) shows
  `via_number` only for inbound calls. The new outbound value does not change
  that page.

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with PostgreSQL 16.
  A first run failed once in `tests/doors.rs` (`door_station`, line 276):
  the order of two ring events was different. This test does not use the
  changed code. The second run passed.
- Passed: `npm ci`, `npm run lint`, `npm run check`, `npm run build`.
- Passed: `tests/browser/layout.mjs` and `tests/browser/active-calls.mjs`
  against `npm run preview` with Playwright 1.56.1.
- Passed: manual screenshots of the card at 360 px and 1280 px.
- Not run: Docker stack and SIPp end-to-end test.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- None.

## Next action

None.

## References

- `docs/ai-memory/20261015-claude-shared-phonebook-numbers.md`
- `docs/ai-memory/20261015-claude-call-log-filters.md`
