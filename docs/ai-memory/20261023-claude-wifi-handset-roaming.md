# Wi-Fi survey stage 4b: handset roaming threshold and band protection

## Task state

- Task ID: 20261023-claude-wifi-handset-roaming
- Date: 2026-10-23
- Tool: Claude Code
- Status: Completed (implementation in PR, waiting for review and hardware test)
- Branch: `claude/wifi-handset-roaming` (worktree `../talkops-wifi-handset`)
- Base commit: `c053920` (head of stage 4a, PR #62, not merged)
- Related commits or PR: Pending (PR against `ccr-74ace6e8-93dh09`)

## Goal

Provision the roaming setting of Wi-Fi handsets from the survey result and
protect handsets against an SSID band change. The user limited the scope to
the band and the roaming setting. No Wi-Fi credentials.

## Result

- Proposal and ADR 0043 draft first. The user accepted them: "ein PR auf
  Basis 62 und dann merge ich beide". Q1 to Q4 use the recommended options.
- Implemented:
  - Provisioning: `SettingKind::Int` (`min`, `max`); Yealink setting
    `wifi_roaming_threshold` → `static.network.wifi.roaming_threshold`
    (-100 to -30, default -70) for the `wifi` family.
  - Migration `20261108000002_wifi_handset_changes.sql`: `target`,
    `phone_id`, `phone_name`, nullable `site_id`/`wifi_id`, state `waiting`.
  - Core: `phones::set_setting`, `unifi_changes::handset_fetched` (called by
    `phones::record_seen`).
  - API: `routes/wifi_handset.rs` (handset proposal, `POST
    /api/v1/wifi-handset-changes`, handset rollback, band check);
    `unifi_write.rs` proposal gets `handset` and `band_check`; apply and
    rollback of an SSID band removal run the band check
    (`confirm_unknown_handsets`).
  - Web: dialog section "Handset", band check with named confirmations,
    history with handset rows and "waiting for handset", number field for
    integer phone settings, Wi-Fi group.
  - Docs: user guides (EN/DE), ADR 0043 accepted, architecture, roadmap.

## Decisions

- Proposed threshold: survey `sticky_rssi` + 5 dB, limited to -75…-60, only
  for the recommendation `min_rssi`; no row if equal to the effective value.
- The handset change needs `unifi.write` and `phones.manage`, not the UniFi
  write switch. It applies only to the survey handset (phone setting).
- Band: no documented AX parameter, so not provisioned. Band check uses
  classic `stat/sta`: radio on a removed band and ESSID = voice SSID (or
  unknown) → blocked; not reported → named confirmation.
- `roam_lossrate_threshold`, `roam_aggressive.enable`,
  `wireless.channel_enable`: documented only for MP66W, not used.

## Checks

- Passed: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `talkops-provisioning` tests (catalog incl. int limits), core
  `unifi_changes` (3), API `wifi_handset` (3), API `unifi_write` (6),
  `wifi_survey` tests, API unit test `wifi_handset`.
- Passed: web `npm run lint`, `npm run check`, `npm test` (45), `npm run build`.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Passed: browser check with the real server and a Python mock console:
  handset section, band check blocks a 2.4 GHz handset, apply, history
  "waiting for handset", phone page number field (390 px, desktop dark).
- Full workspace test result: see the PR description.
- Not run: real AX83H, real UniFi console, Docker images, SIPp/E2E.
- STE review: project writing rules applied. The full ASD-STE100 standard
  was not available; full language review is pending.

## Dependencies and open work

- Depends on PR #62 (stage 4a). Merge PR #62 first.
- Migration ID: `20261108000002` (after `20261108000001` of PR #62).
- Stage 0: band parameter name from an AX83H configuration export; effect of
  the threshold after `check-sync` without reboot; firmware version.

## Next action

User: merge PR #62, then this PR. Run stage 0 on an AX83H.

## References

- [Proposal](../wifi-handset-roaming-proposal.md), [ADR 0043](../adr/0043-wifi-handset-band-and-roaming.md)
- [Stage 4a summary](20261022-claude-unifi-voice-ssid-write.md)
