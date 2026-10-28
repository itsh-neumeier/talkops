# Cisco 7800 Enterprise SIP integration

## Task state

- Task ID: 20261028-codex-cisco-7800-enterprise
- Date: 2026-10-28, Europe/Berlin
- Tool: Codex
- Status: Completed implementation; device acceptance pending
- Branch: codex/cisco-7800-profiles
- Base commit: 684805cd7c2540f2ca4b60866598d9dfb5f1d4c7
- Related PR: https://github.com/itsh-neumeier/talkops/pull/89 (draft)

## Goal

Extend the existing contribution to support experimental Cisco 7800 Enterprise
SIP provisioning. Target the available 7861 with Enterprise 14.2(1). Do not
convert firmware, update devices, merge, publish or change release versions.

## Result

Added four separate experimental Enterprise profiles and an optional TFTP
deployment. The MPP contribution remains intact. Draft PR 89 includes both
variants. No deployment or device update was performed. Private device and
network identifiers are excluded from repository artifacts.

## Changes

- `profiles/cisco7800.json`, the model catalog and authored templates: separate
  Enterprise IDs, one account, TCP, SEP XML and a declared dialplan.
- `crates/talkops-api/src/cisco_tftp.rs`: private, bounded exports through the
  existing tenant-scoped rendering and encrypted credential paths.
- `docker-compose.cisco-tftp.yml` and `docker/talkops/cisco-tftp.*`: opt-in,
  unprivileged tftpd-hpa delivery, exact source-IP mapping and lease supervision.
- German/English model hints, translated client-IP field and copy exclusion.
- ADR 0049, research sources, bilingual setup and device-test instructions.
- CI checks and targeted XML, API, TFTP and browser regression coverage.

## Decisions

- Reuse this task's clean, dedicated worktree and draft PR 89.
- Separate Enterprise model IDs and templates. Do not reuse 8800 templates.
- One independent account per Enterprise phone. Do not treat physical line
  buttons as proof of independent authentication identities.
- Use optional tftpd-hpa delivery with source-IP-specific files. Keep defaults
  disabled and preserve authenticated HTTP device directories.
- Require a dedicated, volatile export root and a fixed client DHCP address.
- Serve no firmware, generic fallback configuration, fake trust lists or signatures.
- Require a recorded device test before any hardware-verification claim.
- Source-IP admission is not cryptographic authentication. Require a trusted
  LAN, source-preserving Docker networking and a fixed DHCP address.
- Keep the existing Yealink URL in DHCP option 66. Supply the Cisco TFTP IPv4
  address through additional option 150. Do not modify the user's DHCP server.
- Refresh exports every 30 seconds. Use a private tmpfs volume and revoke
  exports after errors. Check the 90-second lease each second. Force process-group
  termination if shutdown does not finish within five seconds.

## Checks

- Passed: `cargo test -p talkops-provisioning --locked`: 29 tests, including
  XML escaping, capacities, account bindings, declared files and variant limits.
- Passed: `cargo test -p talkops-api --test multi_vendor --test phone_clone
  --test phones --test provisioning_profiles --locked`: 24 PostgreSQL tests.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings` and formatting.
  An initial test initializer triggered clippy; the corrected initializer passed.
- Passed: `python3 tests/provisioning/tftp.py`: five tests against the actual
  unprivileged daemon, including multiblock transfers, source isolation,
  traversal, denied uploads and invalid, expired or linked snapshot leases.
- Passed: web lint, Svelte check with zero errors/warnings, 91 tests and build.
  A new test needed Prettier formatting; the repeated lint passed.
- Passed: Cisco MPP/Enterprise DE/EN browser checks, including the IP field.
  Existing provisioning and manufacturer browser flows passed across three
  widths and both themes.
- Passed: all 13 Wiki tests on Linux and host Wiki check: 75 pages, 690 links.
  Windows cannot create the Wiki symlink fixture. The Linux CLI cannot resolve
  the Windows worktree Git link. Host CLI and Linux unit results cover both.
- Passed: pinned source integrity, both generator `--check` commands and
  optional Compose validation with a documentation-range test address.
- Not run locally: full workspace runtime/LDAP/backup suites, production image
  build and SIPp calls. Targeted checks cover the changed paths; remote CI and
  hardware tests remain separate requirements.
- STE review: repository clarity rules applied. Full official standard was
  unavailable; full language review remains pending.

## Dependencies and open work

- All profiles remain `not_tested`. The available 7861 runs Enterprise 14.2(1).
  Acceptance of unsigned XML, SIP registration, calls and voicemail is untested.
- CUCM trust can block unsigned configuration. Do not fabricate trust files or
  clear trust lists. Secure CUCM mode is outside this implementation.
- Enterprise BLF, contacts, extra accounts and locale downloads remain deferred.
- No firmware conversion, release, version change or merge occurred.
- The optional deployment requires an image containing this contribution.
- Review the pushed head's remote CI before integration.

## Next action

Review draft PR 89. Install a reviewed build in a separate deployment task.
Then follow the documented 7861 device test and verify a Yealink resync with
the shared DHCP options. Record results before changing hardware status.

## References

- [MPP contribution](20261028-codex-cisco-7800-profiles.md)
- [Cisco research](../research/cisco-7800.md)
- [ADR 0049](../adr/0049-cisco-enterprise-tftp-bootstrap.md)
- [Setup and hardware test](../en/manufacturer-provisioning.md#cisco-7800-enterprise-sip-phones-experimental)
