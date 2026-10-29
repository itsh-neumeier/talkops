# 0051 - Cisco Enterprise SIP resync, contact status, ringtones and directory

- Status: Accepted
- Date: 2026-10-29
- Extends: [0049](0049-cisco-enterprise-tftp-bootstrap.md)

## Context

A CP-7861 with Enterprise firmware 14.2(1) loads its SEP file over the TFTP
sidecar of ADR 0049 and registers over SIP TCP. The user found these gaps:

- The phone page cannot resync or restart the phone. The Yealink adapter sends
  `Event: check-sync`, which Cisco Enterprise firmware does not use.
- The SEP file has a fixed `versionStamp`. The phone uses the stamp to detect
  a changed configuration.
- "Last contact", IP address and firmware stay empty. TalkOps records them
  only for HTTP provisioning requests. The TFTP sidecar has no database access
  and mounts its volume read-only.
- Ringtones are not available, and the phone shows no TalkOps contacts.

## Options

1. Resync: send our own NOTIFY event, or use the FreeSWITCH command
   `sofia profile internal check_sync <call-id>`. mod_sofia sends
   `Event: service-control` with `action=restart` and the REGISTER Call-ID to
   user agents that contain "cisco".
2. Last contact: report TFTP requests from the sidecar, or use the SIP
   registrations that TalkOps already polls.
3. Ringtones and directory: add new profile artifacts, or generate the files in
   the TFTP exporter and serve the directory from a new HTTP endpoint.

## Decision

**Resync** uses `sofia profile internal check_sync <REGISTER Call-ID>`. This is
the mod_sofia code path for Cisco phones (`action=restart`, zero version
stamps, same Call-ID). The phone loads all files again and registers again.
**Restart** sends a NOTIFY event through ESL with a `call-id` header and the
text body `action=reset`, `RegisterCallId={…}` and zero stamps. The ESL client
gets a `sendevent` variant with a `content-length` body. Both actions need the
Call-ID of a current registration with a Cisco user agent. TalkOps validates
the Call-ID characters before it builds the API command. The resync endpoint
keeps its role check and audit entry.

The full `check_sync` without an argument is not used: it deletes all
registrations of the profile.

TalkOps replaces the text of `<versionStamp>` with `talkops-` and the MD5 hash
of the SEP file with an empty stamp. The hash is computed again after
parameter overrides. The stamp is not a security value. The NOTIFY uses zero
stamps, as mod_sofia does.

**Last contact** comes from the registration poll. For each registration with
a user agent that starts with "Cisco", TalkOps updates `last_seen_at`,
`last_ip` and `last_firmware` (version after the last `/`) of the phone that
carries the SIP user. Only Cisco Enterprise models are updated, at most once a
minute per user. The sidecar stays without database credentials and without
network access to TalkOps.

**Ringtones.** The Enterprise catalog models accept ringtones up to 160 KB
(10 s of 8 kHz WAV). The exporter converts uploaded WAV files to `.rwb` (raw
signed 16-bit little-endian PCM, mono, 16 kHz, 240 to 160800 samples, linear
interpolation, longer tones cut). It writes `Ringlist-wb.xml` (at most 50
entries, names cut to 25 characters) with the phone's selected ringtone first.
The SEP file cannot select a custom ringtone; the user selects it on the
phone. Without a selected ringtone there is no ring list. Each phone gets at
most 8 MB of ringtones. The exporter writes each ringtone once per snapshot and
hard-links it into each address directory. The tmpfs volume grows from 16 MB
to 64 MB.

The TFTP map now permits `Ringlist-wb.xml` and `talkops-<8 hex>.rwb` in
addition to the SEP file and `dialplan.xml`, each with an optional leading
slash. The phone requests some files as `/name`. Source-IP admission and the
read-only, upload-denying rules of ADR 0049 stay unchanged.

**Directory.** The SEP file contains `<directoryURL>` as a child of
`<device>`. The URL is `<endpoint>/cisco/directory/<phone id>/<token>`. The
endpoint returns `CiscoIPPhoneDirectory` XML with the phone's phone book
(enabled extensions with `**`, global contacts, selected sections), 32 rows
per page, paging and search soft keys and a `CiscoIPPhoneInput` search form.
The phone cannot send provisioning credentials. The token is the first
128 bits of SHA-256 over the phone ID and the provisioning password, so a
credential regeneration revokes all tokens. Wrong tokens count against the
provisioning rate limit. All values are XML-escaped. The directory contains
no secrets. The redacted preview masks the token.

The TFTP export renders with an invalid host. Without a configured
provisioning endpoint the SEP file thus has no `directoryURL`. Enterprise
firmware checks HTTPS against its own trust list. The documentation thus
recommends a plain HTTP endpoint on the trusted voice LAN and states that the
phone book and the token are then visible on that LAN.

Profile artifacts stay unchanged, so existing custom templates keep a valid
manifest.

## Consequences

Resync, restart, last contact, ringtones and contacts are available for the
Enterprise profiles. All four need a hardware test on the CP-7861 before the
device validation status changes. Restart uses our NOTIFY path, which is not
the mod_sofia Cisco path; a hardware test must confirm it.

No database migration is necessary. Operators must restart the optional
Compose deployment to get the larger tmpfs volume.

German locale files and line keys (speed dial, BLF) are not part of this
decision. Locale files need Cisco files that TalkOps does not ship.

## References

- [ADR 0049](0049-cisco-enterprise-tftp-bootstrap.md)
- [Setup and device test](../en/manufacturer-provisioning.md#resync-last-contact-ringtones-and-contacts)
- [Cisco research](../research/cisco-7800.md)
- FreeSWITCH `sofia_reg_send_reboot` in `mod_sofia/sofia_reg.c` (v1.10.12)
- <https://usecallmanager.nz/ring-tones.html>
- <https://usecallmanager.nz/phone-services.html>
