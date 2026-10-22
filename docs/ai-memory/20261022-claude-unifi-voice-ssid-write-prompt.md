# UniFi voice SSID write access: analysis and development prompt

## Task state

- Task ID: 20261022-claude-unifi-voice-ssid-write-prompt
- Date: 2026-10-22
- Tool: Claude Code
- Status: Completed
- Branch: `claude/charming-bell-fh6q3y`
- Base commit: `c093fa8`
- Related commits or PR: Pending

## Goal

Find out if the Wi-Fi survey can change the UniFi voice SSID to improve
voice quality. Evaluate `ubiquiti-community/py-unifi`. Prepare a
development prompt from `docs/development-chat-prompt.md`.

## Result

The analysis and the prompt are in
`docs/plans/unifi-voice-ssid-write-dev-prompt.md`. No code changed.

## Changes

- `docs/plans/unifi-voice-ssid-write-dev-prompt.md`: analysis, field
  mapping, limits and the development prompt for stage 4a.
- `docs/ai-memory/20261022-claude-unifi-voice-ssid-write-prompt.md`: this
  summary.

## Decisions

- Do not use py-unifi. It is a generated Python client for the classic API
  of Network 8.0.26 (last commit January 2024).
- The user decided: read and write only through the official Integration
  API (`wifi/broadcasts`). No classic API writes.
- Target version: UniFi Network 11.0.x on UniFi OS 6.0 or later. The user
  runs the latest version. Minimum version for writes: 10.6.106.
- Access point radio settings, 802.11r for WPA2, 802.11k and minimum data
  rate stay hints.
- The development chat must first give a proposal for review.

## Checks

- Passed: none needed; documentation only.
- Not run: Rust, web and E2E checks (no code change).
- Not run: field names against a real UniFi 11.0 console (no hardware).
- STE review: manual review; the full standard was not available.

## Dependencies and open work

- Stage 0 on the real console: confirm field names, API key write
  permission and client disconnect time.

## Next action

The user starts a development chat with the prompt in the plan file.

## References

- [Wi-Fi survey summary](20261022-claude-wifi-voice-survey.md)
- [Wi-Fi survey proposal](../wlan-voice-survey-proposal.md)
- [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
