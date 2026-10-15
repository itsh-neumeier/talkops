# Shared phonebook numbers in active calls

## Task state

- Task ID: 20261015-claude-shared-phonebook-numbers
- Date: 2026-10-15, Europe/Berlin
- Tool: Claude Code
- Status: Completed
- Branch: claude/sweet-goodall-w9bfjp
- Base commit: 6276e7d5e1b3bbe2a97e0fe237758b3f3a0e44b4 (origin/main, TalkOps 1.7.12)
- Related commits or PR: Pending

## Goal

An external number can be in more than one phonebook entry. The dashboard
showed only "(Mehrere Telefonbucheinträge)". The user requested:

- One phonebook name in the row.
- A hover card with a table: phonebook, entry (name), "Go to entry" button.
- With one entry: no card. A click on the name opens the entry.
- "Go to entry" scrolls to the entry and highlights it. It does not open
  the edit dialog.
- The phonebook of the own extension supplies the name first.

The user approved the proposal before implementation.

## Result

Implemented, tested and documented on the feature branch.

## Changes

- `crates/talkops-api/src/active_calls.rs`: `Party` has `contact_id` and
  `matches` (`ContactMatch`). `external_party` selects the name: own
  sections first, then the first name in alphabetical order. `resolve`
  repeats the selection with the extension of the other party. `load` reads
  phonebook sections and phones. New unit tests.
- `web/src/lib/components/CallParty.svelte`: Name link, "+N more" button and
  hover card (`popover="manual"`, fixed position, flips above near the
  bottom edge, compact rows below 640 px).
- `web/src/routes/phonebook/+page.svelte`: `?contact=<id>` selects the section,
  clears the search, scrolls to the row and highlights it (`aria-current`).
- `web/src/lib/api.ts`, `web/src/lib/i18n/en.ts`, `web/src/lib/i18n/de.ts`:
  Types and texts. The unused key `dash.party.multiple` is removed.
- `tests/browser/active-calls.mjs`: Hover card, columns, links, pointer
  grace period, keyboard, 320 px viewport, phonebook jump and highlight,
  single-entry link.
- `docs/adr/0027-shared-phonebook-numbers-in-active-calls.md`: New ADR. It
  supersedes the "no ambiguous name" rule of ADR 0022. ADR index and
  ADR 0022 header link to it.
- `docs/en/active-calls.md`, `docs/de/laufende-gespraeche.md`: User docs.

## Decisions

- Own sections of an extension: sections on the provisioned phones of its
  devices, plus a section with the same name as the extension display name.
  The name rule covers the user's setup ("Familie Zweig" extension and
  phonebook) without phone assignments.
- Card order: global entries first, then section name, then entry name.
- No interest invokers, anchor positioning or polyfill. Browser support is
  limited and the project prefers no new dependencies.
- The `ambiguous` field stays for API compatibility.

## Checks

- Passed: `cargo fmt --all -- --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `DATABASE_URL=… cargo test --workspace` with local PostgreSQL 16.
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build` in `web/`.
- Passed: `tests/browser/active-calls.mjs`, `tests/browser/layout.mjs`,
  `tests/browser/transcription-settings.mjs` against `vite preview`
  with Playwright 1.56.1 and the local Chromium.
- Not run: Docker/SIPp end-to-end test (`tests/e2e/run.sh`). The change does
  not touch FreeSWITCH or call routing.
- STE review: Full standard not available; language review pending.

## Dependencies and open work

- No migration.
- ADR number 0027 can conflict with a concurrent task. Renumber at
  integration if necessary.

## Next action

Release manager: review and integrate the PR.

## References

- `docs/ai-memory/20261012-codex-active-call-overview.md`
- `docs/adr/0022-active-call-identities-and-live-previews.md`
