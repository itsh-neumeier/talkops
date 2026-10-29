# Cisco Enterprise template from the first hardware test

## Task state

- Task ID: 20261029-claude-cisco-enterprise-tested-config
- Date: 2026-10-29
- Tool: Claude Code
- Status: Completed
- Branch: claude/cisco-enterprise-tested-config
- Base commit: 145f3bc2699f2dbb84f2a9ea1b21dcafd6365597 (main, v1.8.15)
- Related commits or PR: Pending

## Goal

Make the Enterprise SIP template produce a file that a real CP-7861 accepts.
The user asked for a PR only, without a release.

## Result

The user ran 1.8.15 with a `CP-7861-K9` (`sip78xx.14-2-1-0001-14`). Findings:

- The TFTP sidecar delivered the SEP file to the correct client IP.
- The phone then reported "Error Verifying Config Info", repeated the CTL, ITL
  and SEP requests, never requested `dialplan.xml` and sent no SIP.
- A hand-written SEP file, served by a temporary in.tftpd container, was
  accepted. The phone requested `dialplan.xml` and registered over SIP TCP.
  Its log shows `MGMT_STATE_REGISTERED` for line 1.

The template now emits the elements of that accepted file: `versionStamp`,
`sipCallFeatures/remoteCcEnable`, `sipStack/remotePartyID`,
`ethernetPhonePort`, `securedSipPort` and `commonProfile`. NTP mode is
lowercase `unicast`. `webAccess` is `0`, which enables the read-only status
pages (Cisco inverts this flag). The template no longer emits `enableVad`,
`dtmfOutofBand`, `kpml`, `natEnabled` and the per-line ring, MWI and call
limit elements, because the accepted file did not contain them.
`messagesNumber` stays for voicemail.

The rendered file is not byte-identical with the tested file: it keeps
`messagesNumber` and uses TalkOps labels. The test did not isolate which single
element the firmware requires.

UI save problem: during the test, the TFTP client IP and the global SIP server
were empty in the database. The user set them with SQL. Code review found no
defect: the vendor settings component, the phone page save request and the
server merge keep the value, and API tests store it. The Cisco browser test
filled the field but never saved. It now saves and checks the request body.
If CI passes, the probable cause is UI discoverability (field on the detail
page in a collapsed category), not a defect.

## Changes

- `crates/talkops-provisioning/templates/talkops/cisco7800/enterprise.cnf.xml.jinja`
- `crates/talkops-provisioning/tests/cisco7800.rs`: assert the new elements,
  `commonProfile` and the XML declaration at the start.
- `tests/browser/cisco7800.mjs`: save the Enterprise TFTP field and check the
  PUT body.
- `docs/en/manufacturer-provisioning.md`, `docs/de/hersteller-provisionierung.md`,
  `docs/research/cisco-7800.md`: hardware result, error message meaning and
  phone status pages.

## Decisions

- Keep `device_validation` as `not_tested`. Only provisioning and registration
  passed; calls and media are open, and no other validation value exists.
- Follow the accepted file instead of the previous untested element set.

## Checks

- Passed: on hardware, the hand-written file (same structure) was accepted and
  the phone registered.
- Passed (rust:trixie container): `cargo fmt --all -- --check`,
  `cargo clippy -p talkops-provisioning --all-targets --locked -- -D warnings`
  and `cargo test -p talkops-provisioning --locked` (29 tests).
- Not run locally: workspace tests with PostgreSQL/LDAP. Web lint, check and browser tests
  need CI; npm is not installed on the development host.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- Retest the rendered file on the CP-7861 before release. Until then, the user
  keeps the temporary `cisco-tftp-test` container.
- Calls, audio, DTMF, hold, transfer and voicemail are not tested.
- The user must rotate the SIP password of extension 999; it appeared in a
  terminal output during the test.
- Remove `/root/cisco-test` on the host after the retest.

## Next action

Merge the PR, build an image and retest the CP-7861 with the normal sidecar.

## References

- docs/ai-memory/20261028-claude-cisco-tftp-privileges.md
- docs/ai-memory/20261028-claude-release-1.8.15.md
- https://usecallmanager.nz/sepmac-cnf-xml.html
