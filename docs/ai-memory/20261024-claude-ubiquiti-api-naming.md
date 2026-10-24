# Name the UniFi connection "Ubiquiti Networks API"

## Task state

- Task ID: 20261024-claude-ubiquiti-api-naming
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: `claude/ubiquiti-api-naming` (worktree `../talkops-naming`)
- Base commit: `9f69391`
- Related commits or PR: Pending

## Goal

The user wants the UniFi connection in the UI to have the name
"Ubiquiti Networks API". This is PR 1 of 6 from one user request. The other
PRs: phone network data (PR 2), settings search (PR 3), dashboard lists
with 3 entries (PR 4), charts (PR 6), UniFi Talk style side panel (PR 5).
The user accepted the proposal before work started.

## Result

UI text and user guides use "Ubiquiti Networks API" for the connection.

## Changes

- `web/src/lib/i18n/en.ts`, `web/src/lib/i18n/de.ts`: 25 keys. Tab, card
  title, permission names, connection states, errors and menu paths.
- `docs/en/wifi-survey.md`, `docs/de/wlan-begehung.md`: section title,
  menu paths, permission table and a note about the name.

## Decisions

- Change only the name of the TalkOps connection. Text about the UniFi
  product stays: menu paths in UniFi Network, "UniFi Network 10.6.106",
  "UniFi consoles", "change it in UniFi".
- Do not change code identifiers: crate `talkops-unifi`, permissions
  `unifi.manage`/`unifi.write`, API paths, tables and i18n keys. Thus no
  migration and no API change. Existing roles stay valid.
- Rust error text (API messages and logs) stays. The UI shows i18n text for
  the error codes.
- The heading "Connect the Ubiquiti Networks API" changes a wiki anchor.
  No page linked to the old anchor.

## Checks

- Passed: `npm run lint`, `npm run check`, `npm test` (68), `npm run build`.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Not run: Rust checks (no Rust change), Docker stack, SIPp/E2E.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- None. PR 2 (`claude/phone-network-stats`) uses this branch as its base.

## Next action

None.

## References

- `docs/ai-memory/20261022-claude-wifi-voice-survey.md`
- `docs/ai-memory/20261022-claude-unifi-voice-ssid-write.md`
