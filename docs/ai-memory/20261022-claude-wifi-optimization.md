# Wi-Fi optimization after a survey (stage 4)

## Task state

- Task ID: 20261022-claude-wifi-optimization
- Date: 2026-10-22
- Tool: Claude Code
- Status: In progress (proposal published, waiting for user review)
- Branch: `claude/wonderful-fermat-6iar0b` (restarted from `origin/main` after PR #59 merged)
- Base commit: `c093fa8` (`origin/main`, TalkOps 1.8.6)
- Related commits or PR: draft PR, see the PR list of the branch

## Goal

Turn selected Wi-Fi survey recommendations into changes that an
administrator applies after explicit confirmation: handset values through
Yealink provisioning, SSID values through the official UniFi Integration
API with snapshot and rollback. The user said "weiter" after stages 1 to 3
were merged; the agreed workflow requires a proposal before code.

## Result

- Proposal `docs/wlan-optimization-proposal.md` and ADR draft 0042. No code.

## Decisions (proposed)

- Handset: catalog settings `vad`, `audio_dscp`, `wifi_roaming_threshold`
  (new `int` kind). Roaming parameter name and range from the Yealink AX83H
  Wi-Fi Roaming Guide: `static.network.wifi.roaming_threshold`, -100 to -30,
  default -70. The pinned FusionPBX template writes it without `static.`.
- UniFi: only the official `PUT /v1/sites/{siteId}/wifi/broadcasts/{id}`
  (full replacement), allowlisted fields present in the read object,
  encrypted snapshot (object contains the passphrase), typed SSID
  confirmation, conflict checks, rollback. Off by default; new permission
  `unifi.write`.
- No private API for radio settings in this stage.
- Delivery recommendation: handset part first, UniFi part after a hardware
  test of the survey with the real console.

## Checks

- Passed: no open PRs; ADR 0042 is free; latest migration is `20261107000002`.
- Not run: code checks (documentation only).
- STE review: project writing rules applied. The full ASD-STE100 standard
  was not available; full language review is pending.

## Dependencies and open work

- Proposed migration ID: `20261108000001_wifi_changes.sql` (not created).
- Open questions for the user are in the proposal.
- UniFi field names must be verified with the user's console version.

## Next action

Wait for user feedback on the proposal.

## References

- [Proposal](../wlan-optimization-proposal.md)
- [ADR 0042 draft](../adr/0042-wifi-optimization-with-confirmed-changes.md)
- [Survey task summary](20261022-claude-wifi-voice-survey.md)
