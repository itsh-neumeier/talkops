# Wi-Fi survey stage 4b: handset roaming threshold and band protection

## Task state

- Task ID: 20261023-claude-wifi-handset-roaming
- Date: 2026-10-23
- Tool: Claude Code
- Status: In progress (proposal and ADR draft waiting for user review)
- Branch: `claude/wifi-handset-roaming` (worktree `../talkops-wifi-handset`)
- Base commit: `c053920` (head of stage 4a, PR #62, not merged)
- Related commits or PR: Pending

## Goal

Provision the roaming setting of Wi-Fi handsets from the survey result and
protect handsets against an SSID band change. The user limited the scope to
the band and the roaming setting. No Wi-Fi credentials.

## Result

- Proposal: `docs/wifi-handset-roaming-proposal.md`.
- ADR draft: `docs/adr/0043-wifi-handset-band-and-roaming.md` (Proposed).
- No code yet. The user reviews the proposal first.

## Decisions

- Roaming threshold: `static.network.wifi.roaming_threshold` (-100 to -30,
  default -70) is documented for the AX83H (Wi-Fi Roaming Guide,
  Administrator's Manual). It becomes a catalog setting of kind `int`.
- Band: no documented AX parameter. Not provisioned until stage 0 confirms
  the name from a configuration export. Band protection uses UniFi
  `stat/sta` (5 GHz / 2.4 GHz / unknown) before an SSID band change.
- `roam_lossrate_threshold`, `roam_aggressive.enable` and
  `wireless.channel_enable` are documented only for the MP66W: not used.

## Checks

- Not run: no code changed yet.
- STE review: project writing rules applied. The full ASD-STE100 standard
  was not available; full language review is pending.

## Dependencies and open work

- Depends on PR #62 (stage 4a).
- Open questions Q1 to Q5 in the proposal.
- Stage 0: band parameter name from an AX83H configuration export, effect
  of the threshold after `check-sync` without reboot, firmware version.

## Next action

User: review the proposal and answer Q1 to Q5. Then implement on this branch.

## References

- [Proposal](../wifi-handset-roaming-proposal.md), [ADR 0043](../adr/0043-wifi-handset-band-and-roaming.md)
- [Stage 4a summary](20261022-claude-unifi-voice-ssid-write.md)
