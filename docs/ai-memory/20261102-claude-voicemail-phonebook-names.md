# Voicemail phone book names

## Task state

- Task ID: 20261102-claude-voicemail-phonebook-names
- Date: 2026-11-02
- Tool: Claude Code
- Status: Completed
- Branch: claude/voicemail-phonebook-names
- Base commit: 00b968c
- Related commits or PR: Pending

## Goal

Show phone book names in the voicemail list, as in the call log. Do not show
only the caller numbers.

## Result

`GET /api/v1/voicemail/messages` returns a resolved `caller` party for each
message. The voicemail inbox shows the party with the shared `CallParty`
component: name with a link to the phone book entry, number with flag, and a
card when more than one entry has the number.

## Changes

- `crates/talkops-api/src/active_calls.rs`: new `Directory::voicemail_caller`.
  `job_parties` uses it for voicemail jobs.
- `crates/talkops-api/src/routes/voicemail.rs`: `MessageEntry` with
  `caller: Party` for the message list.
- `crates/talkops-api/tests/voicemail.rs`: test for the phone book name.
- `web/src/lib/api.ts`, `web/src/lib/components/VoicemailInbox.svelte`:
  show `caller` with `CallParty`.
- `docs/en/voicemail.md`, `docs/de/voicemail.md`: describe the caller name.

## Decisions

- Resolve on the server with the call log `Directory`. Then number formats,
  extensions and own phone book sections match the call log.
- Entries from a phone book of the box owner win when several entries match.
- The voicemail list has no toggle for phone book names. The call log toggle
  is a call log setting.
- The stored `caller_name` and `caller_number` fields stay in the response.

## Checks

- Passed: `cargo fmt --all -- --check` (rust:1-trixie container).
- Passed: `cargo clippy -p talkops-api --all-targets -- -D warnings`.
- Passed: `cargo test -p talkops-api --lib --test voicemail
  --test transcription_jobs` with Postgres 17 (116 + 3 + 2 tests).
- Passed: `npm run check` (svelte-check: 0 errors, 0 warnings) and
  `prettier --check` for the changed web files (node:22 container).
- Not run: full workspace tests, web build and SIPp e2e. CI runs them.
- Not run: manual check in the browser.
- STE review: manual review of this file and the user documentation.

## Dependencies and open work

- None.

## Next action

None.

## References

- `docs/ai-memory/20261012-codex-active-call-overview.md`
