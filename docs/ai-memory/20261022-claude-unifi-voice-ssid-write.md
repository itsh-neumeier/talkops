# Wi-Fi survey stage 4a: UniFi voice SSID changes

## Task state

- Task ID: 20261022-claude-unifi-voice-ssid-write
- Date: 2026-10-22
- Tool: Claude Code
- Status: Completed (implementation in PR, waiting for review and hardware test)
- Branch: `ccr-74ace6e8-93dh09` (session branch; worktree `../talkops-unifi-write`)
- Base commit: `c093fa8dddd40a14823d9e6ed41922e0e38efc74` (`origin/main`, TalkOps 1.8.6)
- Related commits or PR: Pending (see the PR of the branch)

## Goal

Apply Wi-Fi survey recommendations to the UniFi voice SSID after explicit
confirmation, with a snapshot and a rollback. Read and write only through the
official UniFi Network Integration API. No classic API writes.

## Result

- First step: proposal and ADR 0042 draft. The user approved them
  ("Freigegeben") with the recommended answers to Q1 to Q6.
- Implemented in the same PR:
  - `talkops-unifi`: SSID list and detail reads (GET), `allowlist.rs`
    (fields, checks, snapshots, hash), `proposal.rs` (recommendation to
    change), `write.rs` (read-modify-write, one `PUT` to
    `wifi/broadcasts/{id}`, read-back), extended connection test.
  - Core: migration, permission `unifi.write`, voice SSID and write switch
    on the connection, `unifi_changes` module, `wifi_survey::any_active`.
  - API: `routes/unifi_write.rs` (SSID list, voice SSID, proposal, apply,
    history, rollback with foreign-change check), audit entries, survey
    start blocked during a change.
  - Web: diff dialog, voice SSID card and capability rows in the UniFi tab,
    new tab "SSID changes" with rollback, texts in `en.ts` and `de.ts`.
  - Docs: user guides (EN/DE) with rules, risks, API key rights, steps and
    version table; ADR 0042 accepted; architecture section and roadmap.

## Changes

- `crates/talkops-unifi/src/{lib,allowlist,proposal,write}.rs`, `Cargo.toml`
- `migrations/20261108000001_unifi_wifi_changes.sql`
- `crates/talkops-core/src/{roles,unifi,unifi_changes,lib}.rs`, `wifi_survey/mod.rs`
- `crates/talkops-api/src/routes/{unifi_write,wifi_survey,mod}.rs`, `lib.rs`
- `web/src/lib/{unifiWrite,api,wifiSurvey}.ts`,
  `web/src/lib/components/{UnifiProposalDialog,UnifiChanges,UnifiSettings}.svelte`,
  `web/src/routes/wifi-surveys/{+page,[id]/+page}.svelte`, `web/src/lib/i18n/{en,de}.ts`
- Tests: `crates/talkops-core/tests/unifi_changes.rs`, `crates/talkops-api/tests/unifi_write.rs`,
  unit tests in the crate modules, `web/tests/unifiWrite.test.ts`
- Docs: `docs/unifi-wifi-write-proposal.md`, `docs/adr/0042-*.md`, `docs/adr/README.md`,
  ADR 0041 follow-up link, `docs/architecture.md`, `docs/en/wifi-survey.md`,
  `docs/de/wlan-begehung.md`

## Decisions

- API facts come from the OpenAPI copies 10.6.106 and 11.0.89 in the
  Art-of-WiFi client repository. The Wi-Fi broadcast schemas are the same
  in both versions.
- PUT body = new GET without `id` and `metadata`, with only allowlist
  changes. `write::body` refuses a difference outside the allowlist.
- Only `STANDARD` SSIDs with `metadata.origin` `USER_DEFINED`. No write if
  the GET answer has no readable passphrase or PPSK (masked or missing).
- Q1: `fastRoamingEnabled` (802.11r) stays out of the allowlist; TalkOps
  proposes WPA3 fast roaming only when 802.11r is on. Q2: the `band_5ghz`
  row is not selected by default. Q3: DTIM 1 (2.4 GHz: 3 if locked).
  Q4: handoff threshold = survey `sticky_rssi`, limited to -80 to -60 dBm.
  Q5: the proposal is visible read only. Q6: the change history has no time
  limit; `survey_id` is `ON DELETE SET NULL`.
- Own route `PUT /api/v1/unifi/voice-ssid`: a save of the connection resets
  the version check, so writes need a new connection test after a save.
- One change at a time: unique partial index on `state = 'applying'`. A row
  older than 5 minutes counts as interrupted.
- Snapshot slots: `{"value": …}` or `{"absent": true}`. Numbers compare as
  numbers (`5` = `5.0`).

## Checks

- Passed: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Passed: `cargo test -p talkops-unifi` (26), core `unifi_changes` (2) and
  `wifi_survey` (3), API `unifi_write` (6), with PostgreSQL 16 local.
- Passed: web `npm run lint`, `npm run check`, `npm test` (44), `npm run build`.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Passed: browser check with the real server binary and a mock console
  (Python, Network 11.0.89 format): UniFi tab, diff dialog, apply, history
  and rollback at 390 px (no horizontal scroll) and in desktop dark mode.
- Full workspace test result: see the PR description.
- Not run: real UniFi console, real handset, Docker images, SIPp/E2E.
- STE review: project writing rules applied. The full ASD-STE100 standard
  was not available; full language review is pending.

## Dependencies and open work

- Migration ID: `20261108000001`. Recheck uniqueness at integration.
- No new crate dependency.
- Stage 0 on a real 11.0 console: confirm the field names with
  `GET wifi/broadcasts/{id}` (anonymized, without passphrase), whether the
  GET returns the passphrase in clear text, whether the PUT accepts the
  body without `id` and `metadata`, the API key rights for the PUT and the
  client disconnect time. Add the result to the version table.

## Next action

Release chat: review the PR, wait for CI/E2E, integrate. User: run stage 0
on the real 11.0 console and report the results.

## References

- [Proposal](../unifi-wifi-write-proposal.md), [ADR 0042](../adr/0042-unifi-voice-ssid-write-access.md)
- [Stage 1 to 3 summary](20261022-claude-wifi-voice-survey.md),
  [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
- [User guide](../en/wifi-survey.md), [German guide](../de/wlan-begehung.md)
