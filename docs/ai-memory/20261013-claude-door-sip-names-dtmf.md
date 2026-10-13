# Door station SIP user names and DTMF unlock codes

## Task state

- Task ID: 20261013-claude-door-sip-names-dtmf
- Date: 2026-10-13
- Tool: Claude Code
- Status: Completed
- Branch: claude/sharp-davinci-0w18ol
- Base commit: 8fe0192 (origin/main, TalkOps 1.7.8)
- Related commits or PR: see the PR of branch `claude/sharp-davinci-0w18ol`

## Goal

Dahua VTO/VTH devices do not accept `-` in the SIP user name. TalkOps
generated `<extension>-<n>` and did not let the admin select the name. The
user also wants unlock codes (DTMF) that can be set freely in TalkOps. Check
the VTO SIP fields from the user's screenshot against TalkOps.

## Result

- SIP user name = extension number + free ending (`8000vto`, `8001vth`).
  The ending must not start with a digit. Door stations get `<ext>vto`
  (then `vto2`, ...) when no ending is given; other kinds keep `<ext>-<n>`,
  now with the lowest free `n` (before: `count + 1`, which could collide
  after a deletion).
- Admins set the ending when they add a device and can rename a device.
  Browser devices (`web-...`) cannot be renamed. Renames are audited with
  the old and new name.
- Door stations have one DTMF unlock code per lock (2-8 keys of `0-9*#`),
  stored encrypted. The dialplan marks the answering leg of a ring
  (`export nolocal:talkops_door_dtmf=<id>`) and the caller leg of a direct
  internal call to the door extension (`set talkops_door_dtmf=<id>`).
  `door_dtmf.rs` follows CHANNEL_ANSWER, DTMF and CHANNEL_HANGUP_COMPLETE
  over the inbound ESL connection and opens the door through the HTTP API.
- External legs (not `sofia/internal/…`) may use the codes only when their
  number is in `door_stations.dtmf_allowed_numbers` (E.164, normalized with
  the tenant dial plan, max. 20). Outbound legs (forwarded mobile) use the
  dialed number; inbound legs use `talkops_caller_number`/caller ID. The
  public dialplan marks a direct inbound call to the door extension only
  when the caller is allowed.
- User docs (EN/DE) have a VTO field table that matches the screenshot.

## Changes

- `migrations/20261029000001_door_unlock_codes.sql`: `unlock_code_1_enc`,
  `unlock_code_2_enc`.
- `crates/talkops-core/src/extensions.rs`: `validate_sip_username`,
  `default_username`, rename in `update_device`.
- `crates/talkops-core/src/doors.rs`: codes in input/output, validation,
  `unlock_codes`, `detects_unlock_codes`.
- `crates/talkops-api/src/door_dtmf.rs`: new DTMF watcher (`Matcher`, `spawn`).
- `crates/talkops-api/src/fsxml/dialplan.rs`: leg marks.
- `crates/talkops-api/src/routes/{extensions,doors}.rs`: audit details.
- `web/src/lib/components/DeviceList.svelte`, `DoorStationForm.svelte`,
  `web/src/lib/api.ts`, i18n `en.ts`/`de.ts`: form fields.
- `docs/en/door-station.md`, `docs/de/tuersprechstelle.md`.

## Decisions

- User decision: SIP name = extension number + free ending; door default
  `vto`.
- User decision: TalkOps detects the code itself and opens through HTTP, in
  ring calls and in direct calls to the door station.
- User decision: external callers only from an allow list of numbers.
  Internal phones are always allowed. Caller ID can be falsified; the docs
  recommend codes with at least 4 keys when external numbers are allowed.
- Codes are encrypted (secret rule in CLAUDE.md, ADR 0008) and never returned
  or audited; the API returns `has_unlock_code_1/2`.
- Detection by ESL events and a per-leg map (CHANNEL_ANSWER carries channel
  variables). This does not need `verbose_events`.
- The station's own leg is not marked (`nolocal:`), so keys at the door do
  not open. At most 40 keys per call are evaluated; a pause > 4 s starts a
  new code. Codes of the two doors must not end with each other.
- `inbound-reg-force-matching-username=true` stays: the VTO must use the
  same value for *Username* and *SIP Server Username*. The docs say this.

## Checks

- Passed: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace` with local PostgreSQL 16 (259 passed).
- Passed: `npm run lint`, `npm run check`, `npm run build` in `web/`.
- Not run: `tests/e2e/run.sh` (Docker/SIPp stack) and tests with a real
  FreeSWITCH and real Dahua hardware. The DTMF event path is tested with
  synthetic ESL events only.
- STE review: manual review against `docs/documentation-style.md`.

## Dependencies and open work

- Migration ID: `20261029000001_door_unlock_codes` (codes and
  `dtmf_allowed_numbers`).
- Real-hardware check: VTO registration with `8001vto`, DTMF detection on a
  real FreeSWITCH (RFC 2833 events on the B-leg of a ring).
- Queue and voice-menu targets of a ring: the mark is exported to bridged
  legs; legs created through loopback may not receive it.

## Next action

Release chat: review and integrate the PR. User: test with the VTO.

## References

- `docs/adr/0013-tuersprechstellen-dahua.md`
- `docs/en/door-station.md`
