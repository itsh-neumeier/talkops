# Fix Cisco Enterprise resync and last contact: registration user agent

## Task state

- Task ID: 20261029-claude-cisco-registration-agent
- Date: 2026-10-29
- Tool: Claude Code
- Status: Completed (hardware retest open)
- Branch: claude/cisco-registration-agent
- Base commit: 45e2b25 (main, v1.8.17)
- Related commits or PR: Pending

## Goal

On 1.8.17 the CP-7861 page shows "Das Telefon ist nicht registriert" after
**Resync**. Find and fix the cause.

## Result

`Registration.user_agent` came from the `metadata` column of
`show registrations`. FreeSWITCH fills that column with the directory variable
`registration_metadata` (`sofia_reg.c`, `switch_core_add_registration`), not
with the SIP User-Agent. TalkOps does not set that variable, so the value was
empty. `cisco_service_control` and `record_cisco_contacts` (ADR 0051) filter on
a Cisco user agent and thus never matched. Resync, restart and last contact
did nothing.

TalkOps now also runs `sofia xmlstatus profile internal reg` and copies
`<agent>` to each registration by Call-ID. The live status thus shows real
user agents, too.

## Changes

- `crates/talkops-api/src/telephony.rs`: `load_registrations`, `parse_agents`
  and a unit test with the mod_sofia XML format.

## Decisions

- Keep `show registrations` as the source of the registration list and add the
  agent from mod_sofia. A failed xmlstatus call leaves the agent empty and does
  not mark the poll incomplete.

## Checks

- Passed (rust:trixie): `cargo fmt`, `cargo clippy -p talkops-api --all-targets
  -- -D warnings`, telephony unit tests.
- Not run: hardware retest (user action).
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Hardware retest of the plan in
  `docs/ai-memory/20261029-claude-cisco-enterprise-features.md`.

## Next action

Merge, release, and retest Resync on the CP-7861.

## References

- docs/ai-memory/20261029-claude-cisco-enterprise-features.md
- docs/adr/0051-cisco-enterprise-service-control-ringtones-directory.md
- FreeSWITCH v1.10.12 `mod_sofia/sofia_reg.c` and `mod_sofia.c` (`show_reg_callback_xml`)
