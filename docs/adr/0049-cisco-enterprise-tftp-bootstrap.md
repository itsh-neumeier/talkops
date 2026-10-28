# 0049 - Optional IP-scoped TFTP bootstrap for Cisco Enterprise SIP

- Status: Accepted
- Date: 2026-10-28
- Extends: [0038](0038-vendor-provisioning-profiles.md),
  [0039](0039-source-derived-multi-vendor-provisioning.md) and
  [0048](0048-authored-provisioning-profiles.md)

## Context

The available Cisco 7861 runs Enterprise SIP 14.2(1). MPP XML and authenticated
HTTP provisioning do not configure this firmware. The user requested Enterprise
support without a firmware conversion. Cisco documents a TFTP bootstrap and
phone-level digest authentication. A physical line count does not prove that
this firmware accepts independent credentials on each line.

## Decision

Add separate experimental Enterprise profiles for 7811, 7821, 7841 and 7861.
Limit the first implementation to one independent account in slot 1. Use TCP
SIP, a device-specific SEP XML file and a declared dialplan. Keep the MPP
profiles and their account bounds. Keep all profiles hardware-untested.

Add one explicit exception to ADR 0039's authenticated HTTP delivery rule.
An optional tftpd-hpa sidecar serves only these Enterprise artifacts on a
trusted voice LAN. The default deployment starts no TFTP service. The optional
Compose file requires a LAN bind address. UDP 69 maps to unprivileged UDP 1069.
The sidecar and TalkOps both run as UID 10001 with all capabilities dropped.
The sidecar has no database credentials and mounts its export volume read-only.

Use a fixed client IPv4 address per exported phone. The map prefixes requests
with that actual source address. It permits only exact SEP and dialplan names.
It rejects uploads, directory paths and all other names. The exporter writes
only the matching phone's declared files into each address directory. Omit all
phones with a duplicate address. This is network admission, not cryptographic
authentication. It requires DHCP reservations and a trusted, restricted LAN.
Docker must preserve the real client address.

Use tftpd-hpa for the wire protocol. Do not implement a TFTP server in Rust.
Use filename admission and a read-only root filesystem without adding chroot
capabilities to the sidecar. Test these rules against the actual daemon.

TalkOps renders the existing managed configuration into a dedicated tmpfs root.
Keep files private and exclude the client IP from device copies. Publish
complete snapshots every 30 seconds. Revoke exports after generation failures.
A separate guard checks the 90-second snapshot lease each second. It stops the
TFTP process group on expiry and forces termination after five seconds if needed.
Exported plaintext is not persistent backup data.
The existing encrypted database, preview masking and authenticated HTTP exports
remain in use. HTTP authentication is not bypassed.

Serve no firmware, generic fallback files, fake CTL/ITL files or fake signatures.
Do not clear trust lists or install certificates. Existing CUCM trust can block
unsigned files. A recorded device test must establish acceptance, registration
and calls before the hardware state changes.

## Consequences

The optional transport makes nonsecure Enterprise testing possible without
converting the firmware. It does not implement CUCM secure mode or claim Cisco
certification for FreeSWITCH. Installed language resources remain unchanged.
Custom keys, BLF, contacts and multiple independent accounts are deferred.

The server image adds the Debian tftpd-hpa package and a small lease supervisor.
No new Rust dependency or database migration is required. Operators must use an
image containing this contribution before enabling the optional Compose file.

## References

- [Cisco research](../research/cisco-7800.md)
- [Setup and device test](../en/manufacturer-provisioning.md#cisco-7800-enterprise-sip-phones-experimental)
- [Debian tftpd-hpa manual](https://manpages.debian.org/trixie/tftpd-hpa/in.tftpd.8.en.html)
