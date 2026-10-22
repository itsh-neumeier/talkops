# Wi-Fi voice survey and UniFi roaming analysis

## Task state

- Task ID: 20261022-claude-wifi-voice-survey
- Date: 2026-10-22
- Tool: Claude Code
- Status: In progress (proposal published, waiting for user review)
- Branch: `claude/wonderful-fermat-6iar0b`
- Base commit: `b1fa28350dc1830e222a27ee770641edb1935ccc` (`origin/main`, TalkOps 1.8.5)
- Related commits or PR: https://github.com/itsh-neumeier/talkops/pull/59 (draft)

## Goal

Let an administrator walk a site with a Wi-Fi handset. TalkOps calls the
handset, plays a test signal, records the call, measures the voice stream
each second, reads the handset state from UniFi Network (read only), rates
each roam and shows a result page with a comparison of two surveys.

The user requires a proposal first. Code, migrations and UI start only
after user approval.

## Result

- Proposal and ADR draft written and committed. No runtime change.
- The worktree is the session checkout on the designated branch. It was
  clean and equal to `origin/main` at task start. No other work existed in it.

## Changes

- `docs/wlan-voice-survey-proposal.md`: scope, stages, data model, API, UI,
  prototype results, open points, size estimate.
- `docs/adr/0041-wifi-voice-survey-and-unifi-correlation.md`: ADR draft
  (Proposed).
- `docs/adr/README.md`: index row for ADR 0041.
- `docs/ai-memory/20261022-claude-wifi-voice-survey.md`: this summary.

## Decisions

- Media data from FreeSWITCH only: 1 s poll of `mediaStats` /
  `uuid_set_media_stats`, `RECV_RTCP_MESSAGE` events, CDR error periods.
  Source analysis of FreeSWITCH v1.10.12 confirmed these interfaces.
  No packet capture unless the hardware test proves a need.
- UniFi: new crate `talkops-unifi`, GET requests only. Integration API for
  sites/devices/client identity, classic `stat/sta/{mac}` for RSSI, channel
  and rate. Roams derived from AP/BSSID changes. No syslog/SIEM.
- Analysis as media worker job `wifi_survey.analyse`; live collection in
  the API process.
- Stage 4 (UniFi writes, roaming threshold provisioning) is a follow-up
  phase with its own ADR.
- Proposed delivery: stages 1 to 3 as three PRs.

## Checks

- Passed: `git fetch origin main`; branch equals `origin/main` (`b1fa283`).
- Passed: open PR #58 changes no ADR and no migration; ADR 0041 is free.
- Not run: Rust, web and E2E checks. Only Markdown files changed.
- Not run: hardware prototype. No UniFi console and no Wi-Fi handset in
  this environment. UniFi facts are desk research and need verification.
- STE review: project writing rules applied. The full ASD-STE100 standard
  was not available; full language review is pending.

## Dependencies and open work

- Proposed migration IDs (not created yet): `20261107000001`,
  `20261107000002`. Recheck at implementation and integration.
- Open questions for the user are in the proposal, section "Open points".
- Not yet changed: `docs/architecture.md` roadmap entry (planned with the
  implementation after approval).

## Next action

Wait for user feedback on the proposal. After approval, update the proposal
and the ADR status, then start stage 1 (survey call and media samples).

## References

- [Proposal](../wlan-voice-survey-proposal.md)
- [ADR 0041 draft](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
- Related: [fax proposal summary](20261018-codex-fax-sip-proposal.md) (proposal workflow model)
