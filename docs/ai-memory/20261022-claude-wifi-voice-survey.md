# Wi-Fi voice survey and UniFi roaming analysis

## Task state

- Task ID: 20261022-claude-wifi-voice-survey
- Date: 2026-10-22
- Tool: Claude Code
- Status: Completed (implementation in PR, waiting for review and hardware test)
- Branch: `claude/wonderful-fermat-6iar0b`
- Base commit: `b1fa28350dc1830e222a27ee770641edb1935ccc` (`origin/main`, TalkOps 1.8.5)
- Related commits or PR: https://github.com/itsh-neumeier/talkops/pull/59

## Goal

Let an administrator walk a site with a Wi-Fi handset. TalkOps calls the
handset, plays a test signal, records the call, measures the voice stream
each second, reads the handset state from UniFi Network (read only), rates
each roam and shows a live dashboard, a result page and a comparison of two
surveys.

## Result

- First step: proposal and ADR draft. The user asked for the admin UI with
  all live values on one screen; the proposal got an admin page and a live
  dashboard section. The user then approved the proposal ("Freigabe").
- Implemented stages 1 to 3 of the proposal in the same PR:
  - migrations, core module `wifi_survey` (queries, rating, patterns,
    recommendations, comparison) and `unifi` (encrypted connection)
  - crate `talkops-unifi` (GET-only client, certificate pinning)
  - API: originate and `*39` self-dial, socket session (recording, test
    signal), 1 s collector (`mediaStats`, UniFi `stat/sta`), DTMF markers,
    RTCP events, CDR error periods, routes, retention, HTTP range for
    recording audio
  - media worker job `wifi_survey.analyse`
  - web: Settings → Wi-Fi survey (surveys, settings, UniFi), start dialog,
    live dashboard, result page, comparison, device-row button
  - docs: `docs/en/wifi-survey.md`, `docs/de/wlan-begehung.md`, ADR 0041
    accepted, architecture section and roadmap rows, README link
- Stage 4 (handset provisioning, UniFi writes) is not implemented. It is a
  follow-up phase with its own ADR.

## Changes

- `migrations/20261107000001_unifi_connections.sql`, `migrations/20261107000002_wifi_surveys.sql`
- `crates/talkops-core/src/wifi_survey/{mod,analysis}.rs`, `crates/talkops-core/src/unifi.rs`,
  `roles.rs` (3 permissions), `numbering.rs` (`*39`), `prompts.rs` (2 prompts)
- `crates/talkops-unifi/` (new crate)
- `crates/talkops-api/src/wifi_survey.rs`, `routes/wifi_survey.rs`, `routes/voicemail.rs`
  (`wav_range`), `routes/recordings.rs`, `fsxml/dialplan.rs`, `cdr_spool.rs`,
  `retention.rs`, `main.rs`, `lib.rs`
- `crates/talkops-media-worker/src/main.rs` (lane and job)
- `web/src/routes/wifi-surveys/**`, `web/src/lib/wifiSurvey.ts`,
  `web/src/lib/components/{WifiSurveyChart,WifiSurveySettings,UnifiSettings}.svelte`,
  `DeviceList.svelte`, `SettingsNav.svelte`, `api.ts`, `i18n/{en,de}.ts`
- Tests: `crates/talkops-core/tests/wifi_survey.rs`, `crates/talkops-api/tests/wifi_survey.rs`,
  unit tests in each module, worker test, `web/tests/wifiSurvey.test.ts`

## Decisions

- Media data from FreeSWITCH only (`mediaStats` JSON API each second,
  `RECV_RTCP_MESSAGE`, CDR `error-log`). No packet capture.
- Test signal with `playback` in the outbound socket session:
  `endless_playback` does not read the DTMF queue, so no `DTMF` events.
- The survey owns its recording file (not a `recordings` row).
- Loss block = loss above `loss_block_pct` in the ±3 s window.
- UniFi: Integration API for version, site and AP names; classic
  `stat/sta/{mac}` for radio values. Roams from AP changes. No events/syslog.
- One PR instead of three: the session can push only one branch.
- No handset provisioning for VAD/DSCP: parameter names are not confirmed.

## Checks

- Passed: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Passed: new core, API, UniFi and worker tests (PostgreSQL 16 local).
- Passed: web `npm run lint`, `npm run check`, `npm test` (40), `npm run build`.
- Passed: browser check with the real server and worker binaries and seeded
  survey data: live dashboard at 390 px (no horizontal scroll) and desktop
  dark mode, result page, playback from a roam (audio seeks), comparison,
  UniFi tab, start dialog.
- `cargo test --workspace --locked --no-fail-fast`: 441 passed, 1 ignored (existing
  Whisper model test), 1 failed: `read_only_fax_directory_rejects_upload_without_queueing`.
  It fails only because this environment runs as root (0555 does not block root).
  The same test binary passed as user `nobody`. Unrelated to this task.
- Not run: Docker images, SIPp/E2E, a real FreeSWITCH call, a real UniFi
  console and a real Wi-Fi handset (not available in this environment).
- STE review: project writing rules applied. The full ASD-STE100 standard
  was not available; full language review is pending.

## Dependencies and open work

- Migration IDs: `20261107000001`, `20261107000002`. Recheck uniqueness at
  integration (open PR #58 has no migrations).
- New crate `talkops-unifi`; no new external service dependency.
- Hardware acceptance test: one survey with a real UniFi console and an
  AX8x handset. Record the Network version in the version table of the
  user guide. Check that the API key works for `stat/sta`.
- Follow-up phase (stage 4): handset provisioning (roaming threshold, VAD,
  DSCP) and UniFi writes with confirmation, snapshot and rollback.

## Next action

Release chat: review PR #59, wait for CI/E2E, integrate. User: run the
hardware acceptance test and report the UniFi version result.

## References

- [Proposal](../wlan-voice-survey-proposal.md)
- [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
- [User guide](../en/wifi-survey.md), [German guide](../de/wlan-begehung.md)
- Related: [fax proposal summary](20261018-codex-fax-sip-proposal.md) (workflow model)
