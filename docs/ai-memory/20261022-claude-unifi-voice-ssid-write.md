# Wi-Fi survey stage 4a: UniFi voice SSID changes

## Task state

- Task ID: 20261022-claude-unifi-voice-ssid-write
- Date: 2026-10-22
- Tool: Claude Code
- Status: In progress (proposal and ADR draft waiting for user review)
- Branch: `ccr-74ace6e8-93dh09` (session branch; worktree `../talkops-unifi-write`)
- Base commit: `c093fa8dddd40a14823d9e6ed41922e0e38efc74` (`origin/main`, TalkOps 1.8.6)
- Related commits or PR: Pending

## Goal

Apply Wi-Fi survey recommendations to the UniFi voice SSID after explicit
confirmation, with a snapshot and a rollback. Read and write only through the
official UniFi Network Integration API. No classic API writes.

## Result

- Proposal: `docs/unifi-wifi-write-proposal.md`.
- ADR draft: `docs/adr/0042-unifi-voice-ssid-write-access.md` (Proposed).
- The user asked for a review before implementation. No code yet.

## Changes

- `docs/unifi-wifi-write-proposal.md`: proposal with API facts, gates,
  allowlist, mapping, data model, API, UI, tests and open points.
- `docs/adr/0042-unifi-voice-ssid-write-access.md`, `docs/adr/README.md`.

## Decisions

- API facts come from the OpenAPI copies 10.6.106 and 11.0.89 in the
  Art-of-WiFi client repository. The Wi-Fi broadcast schemas are the same
  in both versions.
- PUT body = new GET minus `id` and `metadata`, with only allowlist changes.
- Only `STANDARD` SSIDs with `metadata.origin` `USER_DEFINED`.
- `fastRoamingEnabled` (802.11r) exists in the API but is not in the task
  allowlist. Open question Q1 for the user.
- Planned migration ID: `20261108000001` (recheck before the PR).

## Checks

- Not run: no code changed yet.
- STE review: project writing rules applied. The full ASD-STE100 standard
  was not available; full language review is pending.

## Dependencies and open work

- Wait for the user review of the proposal (questions Q1 to Q6).
- Stage 0 on a real 11.0 console: field names, passphrase in GET, PUT rights
  of the API key, client disconnect time.

## Next action

User: review the proposal and answer Q1 to Q6. Then implement on the same
branch.

## References

- [Proposal](../unifi-wifi-write-proposal.md), [ADR 0042](../adr/0042-unifi-voice-ssid-write-access.md)
- [Stage 1 to 3 summary](20261022-claude-wifi-voice-survey.md), [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
