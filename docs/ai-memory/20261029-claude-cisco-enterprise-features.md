# Cisco Enterprise SIP resync, contact status, ringtones and directory

## Task state

- Task ID: 20261029-claude-cisco-enterprise-features
- Date: 2026-10-29
- Tool: Claude Code
- Status: Completed (hardware test open)
- Branch: claude/cisco-enterprise-features-1d55ff
- Base commit: 9e9adbf (main, v1.8.16)
- Related commits or PR: Pending

## Goal

Implement the Cisco Enterprise SIP follow-up from the user's CP-7861 test
(firmware `sip78xx.14-2-1-0001-14`): resync and restart, last contact, ringtones,
contacts and UI fixes on the phone page. Deliver a PR only, no release. German
locale files and line keys only as a plan.

## Result

- Resync: `sofia profile internal check_sync <REGISTER Call-ID>`. mod_sofia
  sends `Event: service-control` with `action=restart` and the Call-ID to Cisco
  user agents (source: FreeSWITCH 1.10.12 `sofia_reg_send_reboot`). Restart: a
  NOTIFY event over ESL with `call-id` header and body `action=reset`,
  `RegisterCallId={…}` and zero stamps. The ESL client got
  `sendevent_with_body` (`content-length` framing). The resync endpoint now
  accepts the Enterprise profiles; capabilities report `resync`, `reboot` and
  `delivery: tftp`.
- `versionStamp` is `talkops-<md5>` of the SEP file with an empty stamp. The API
  hashes again after parameter overrides.
- Last contact: the registration poller updates `last_seen_at`, `last_ip` and
  `last_firmware` for Cisco user agents on Enterprise models, at most once a
  minute per user (`phones::record_registration`).
- Ringtones: Enterprise catalog models accept 160 KB WAV. The TFTP exporter
  converts to `.rwb` (16 kHz, linear interpolation, cut at 160800 samples),
  writes `Ringlist-wb.xml` with the selected tone first, hard-links shared
  files, caps 50 tones and 8 MB per phone. The tmpfs grows to 64 MB. The TFTP
  map accepts `Ringlist-wb.xml`, `talkops-<8 hex>.rwb` and a leading slash.
- Directory: `<directoryURL>` → `/provisioning/cisco/directory/<phone>/<token>`.
  Token = first 128 bits of SHA-256 over phone ID and provisioning password
  (regeneration revokes). 32 rows per page, paging, search form, no auth header.
  Without an endpoint (TFTP export host `tftp.invalid`) the element is omitted.
- UI: TFTP file card (file names, client IP, DHCP option 150, no credential
  note), new status text (provisioning and registration passed, calls open),
  readable labels for XML path parameters and real file names, ringtone hint.

## Changes

- `crates/talkops-esl/src/lib.rs`: `sendevent_with_body`, `sendevent_frame`.
- `crates/talkops-api/src/telephony.rs`: `cisco_service_control`, Call-ID in
  registrations, `record_cisco_contacts`, `cisco_firmware`.
- `crates/talkops-api/src/routes/phones.rs`: resync for Enterprise, capability
  and detail `delivery`.
- `crates/talkops-api/src/routes/provisioning.rs`: `phone_entries`, directory
  token and URL, `cisco/directory/` route before basic auth, stamp after overrides.
- `crates/talkops-api/src/cisco_tftp.rs`: binary exports, `wav_to_rwb`, ring
  list, hard links, unit tests.
- `crates/talkops-core/src/phones.rs`: `record_registration`.
- `crates/talkops-provisioning/src/{vendors,phonebook}.rs`, Enterprise template:
  stamp, ring list, Cisco directory XML, `directory_url` in `Common`.
- `presets/phones/cisco7800.yaml`, `docker/talkops/cisco-tftp.map`,
  `docker-compose.cisco-tftp.yml`.
- `web/`: phone page, `PhoneVendorSettings`, `phoneParameters.ts`, `api.ts`, i18n.
- Tests: `crates/talkops-provisioning/tests/cisco7800.rs`,
  `crates/talkops-api/tests/phones.rs`, `crates/talkops-core/tests/domain.rs`,
  `tests/provisioning/tftp.py`, `tests/browser/cisco7800.mjs`,
  `web/tests/phoneParameters.test.ts`.
- Docs: ADR 0051, EN/DE handbook, `docs/research/cisco-7800.md`.

## Decisions

- Use the mod_sofia Cisco path for resync (proven code, correct Call-ID). Never
  call `check_sync` without an argument: it deletes all registrations.
- Restart uses our NOTIFY path without Call-ID header reuse by mod_sofia
  (`notify_callback`); a hardware test must confirm it.
- No `apply-config`: no source confirmed its body for 14.2(1).
- Last contact from SIP, not from the sidecar: the sidecar keeps no database
  or network access to TalkOps.
- Ringtones and ring list come from the exporter, not profile artifacts, so
  existing custom template manifests stay valid.
- Directory over plain HTTP endpoint on the LAN is the documented fallback,
  because Enterprise firmware checks HTTPS against its own trust list.
- Keep `device_validation` `not_tested`.

## Checks

- Passed (rust:trixie container): `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings`, workspace
  tests with PostgreSQL 17 (all passed except the fax read-only directory test,
  which fails only as root in the container; unrelated).
- Passed (Debian trixie, tftpd-hpa, root): `tests/provisioning/tftp.py`, 6 tests.
- Passed (node:22): prettier check of changed files, `npm run check`,
  `npm test` (94), reference and native library checks, `npm run build`,
  browser tests `cisco7800`, `manufacturer-provisioning`, `provisioning`,
  `phone-keys`, `side-panel` with Playwright 1.56.1.
- Not run locally: full `npm run lint` on the Windows checkout (CRLF files);
  CI runs it on LF. SIPp end-to-end stack.
- Not run: hardware test on the CP-7861 (user action).
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Hardware test on the CP-7861: resync, restart, ringtone list, directory,
  last contact. Then calls, audio, DTMF, hold, transfer, voicemail.
- Plan, German locale (not implemented): add optional `<userLocale>`
  (`<name>German_Germany</name>`, `<langCode>de</langCode>`, `<version>`),
  `<networkLocale>Germany</networkLocale>` and `<networkLocaleInfo>` to the
  template, only when locale files exist. Add an admin upload for a Cisco
  locale archive into the provisioning directory; the exporter copies
  `German_Germany/*` and `Germany/g3-tones.xml` per phone and the TFTP map
  admits these exact paths. Do not ship Cisco files.
- Plan, speed dial and BLF (second PR): `<line button="n">` entries with
  `featureID` 2 (`speedDialNumber`, `featureLabel`) and 21 (BLF,
  `featureOptionMask` 1, `speedDialNumber`, `contact`). Add `key_groups` and
  key functions for the Enterprise profiles so that the key editor works;
  buttons start after line 1.

## Next action

Merge the PR after CI. The user builds an image and runs the hardware test
plan from the PR on the CP-7861.

## References

- docs/adr/0051-cisco-enterprise-service-control-ringtones-directory.md
- docs/ai-memory/20261029-claude-cisco-enterprise-tested-config.md
- docs/ai-memory/20261028-claude-cisco-tftp-privileges.md
- https://usecallmanager.nz/ring-tones.html
- https://usecallmanager.nz/phone-services.html
