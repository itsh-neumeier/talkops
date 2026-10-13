# Dahua VTO/VTH setup, SIP password format and door station vendors

## Task state

- Task ID: 20261013-claude-dahua-vth-setup
- Date: 2026-10-13
- Tool: Claude Code
- Status: Completed (PR submitted for review)
- Branch: `claude/busy-fermat-sy3r41` (worktree `../talkops-dahua`)
- Base commit: `bfc4642` (origin/main, TalkOps 1.7.9)
- Related commits or PR: Pending

## Goal

Two Dahua VTH5422H (firmware V4.800.0.2) answer the SIP challenge and get
`403 Forbidden`. A VTO4202F-P-S2 (V4.600.0.3) registers. The same VTH
account registers with MicroSIP. TalkOps generated 20-character SIP
passwords. Yeastar documents 8–15 characters for Dahua VTH (models not
named). The user wants:

- a device-specific option for compatible SIP passwords;
- a step-by-step guide for VTO and VTH in Settings → Door stations;
- door stations structured by vendor, in the UI and in the database, so
  that other vendors can follow.

## Result

- `devices.sip_password_format`: `standard` (20, default, unchanged) or
  `compact15` (15 letters and digits, same alphabet). Create and reset use
  the format. An update stores the format and keeps the password. The reset
  endpoint takes an optional body `{"sip_password_format": …}` (admin only
  for a change). Browser devices: standard only. Audit records the format,
  never the password.
- `door_stations.vendor` (`door_vendor` enum, value `dahua`, default).
  `DoorCtx::vto` selects the client by vendor.
- Log lines for failed SIP authentications (`SIP authentication failed`)
  and unknown users (`SIP authentication for unknown or disabled user`),
  with user and IP, never the password.
- Web: tabs "Door stations" / "Setup guide" with URL hash
  `#guide/<vendor>/<device>`; guides are data in `web/src/lib/doors/`;
  manufacturer field in the door station form; password format in the
  device dialog (preselected `compact15` for type `door`), badge, reset
  dialog with format choice.
- Docs: vendor-neutral overview and a Dahua page in EN and DE, ADR 0026.

## Changes

- `migrations/20261030000001_device_sip_password_format.sql`,
  `migrations/20261030000002_door_station_vendor.sql`
- `crates/talkops-core/src/extensions.rs`: `SipPasswordFormat`, create,
  update, `reset_device_password(…, format)` in a transaction.
- `crates/talkops-core/src/doors.rs`: `DoorVendor`, `vendor` in station,
  input, SQL and `Connection`.
- `crates/talkops-api/src/routes/extensions.rs`: reset body, audit details.
- `crates/talkops-api/src/routes/doors.rs`, `src/doors.rs`: vendor audit and
  dispatch.
- `crates/talkops-api/src/routes/fs.rs`, `src/sip_guard.rs`: log lines,
  `log_value`.
- Tests: `crates/talkops-core/tests/domain.rs` (`sip_password_formats`,
  vendor), `crates/talkops-api/tests/http.rs` (`sip_password_formats`),
  `tests/doors.rs` (vendor), `web/tests/doorGuide.test.ts`.
- Web: `web/src/lib/doors/{guide,vendors}.ts`, `guides/{dahua,common}.ts`,
  `components/doors/{DoorSetupGuide,GuideSectionView}.svelte`,
  `DeviceList.svelte`, `DoorStationForm.svelte`, `routes/door-stations`,
  `api.ts`, `i18n/en.ts`, `i18n/de.ts`.
- Docs: `docs/adr/0026-door-station-vendors.md`, `docs/adr/README.md`,
  `docs/en/door-station.md`, `docs/en/door-station-dahua.md`,
  `docs/de/tuersprechstelle.md`, `docs/de/tuersprechstelle-dahua.md`,
  `docs/architecture.md`, `README.md`.

## Decisions

- User: vendor in UI and database (not only in the UI).
- User: `compact15` for Dahua VTO and VTH. The UI preselects it for new
  devices of type `door`; the API default stays `standard` (compatibility).
- User: the guide states that the VTH Room No. must be the SIP user name.
  The user can set the Room No. to `8001VTH`; TalkOps looks up users
  case-insensitively, but the guide asks for the same spelling.
- User: diagnostic log lines are wanted.
- No common vendor interface (trait) before a second vendor exists.
- Existing passwords are never changed or shortened.

## Findings

- Confirmed in code: 20-character passwords on create, reset and browser
  devices; `inbound-reg-force-matching-username=true`;
  `force-register-domain` makes the domain free.
- FreeSWITCH answers `403` after `401` for a wrong password, an unknown
  user and a user name mismatch. The 403 alone does not prove a password
  length problem.
- Hypotheses, not confirmed: (1) VTH limit of 15 characters; (2) VTH sends
  the Room No. as SIP user. The new log lines separate both cases.

## Checks

- Passed: `cargo fmt --all -- --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace` with local PostgreSQL 16 (270 passed).
- Passed: `npm run lint`, `npm run check`, `npm run build`, `npm test`
  (6 passed) in `web/`.
- Passed: manual UI check with Playwright against a local server (desktop
  and 390 px): door type preselects `compact15`, created password 15
  characters, reset to standard gives 20, no horizontal overflow.
- Not run: `tests/e2e/run.sh` (Docker/SIPp stack with FreeSWITCH).
- Not run: hardware test with Dahua VTO/VTH. A MicroSIP test does not
  replace it.
- STE review: manual review against `docs/documentation-style.md`; the
  official standard was not available. Full review pending.

## Dependencies and open work

- Migration IDs: `20261030000001_device_sip_password_format`,
  `20261030000002_door_station_vendor`. Check uniqueness at integration.
- Hardware test: VTH5422H with a 15-character password and Room No. equal
  to the SIP user name; read the new log lines when it fails.
- A second vendor needs a common HTTP client interface (ADR 0026).

## Next action

Release chat: review and integrate the PR. User: test the VTH with
**New password → Compatible (15 characters)** and Room No. = SIP user name.

## References

- `docs/ai-memory/20261013-claude-door-sip-names-dtmf.md`
- `docs/adr/0013-tuersprechstellen-dahua.md`, `docs/adr/0026-door-station-vendors.md`
- https://help.yeastar.com/en/p-series-appliance-edition/integrations/dahua/register-dahua-vth-indoor-monitor-to-yeastar-p-series-pbx-system.html
