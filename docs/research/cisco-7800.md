# Cisco 7800 provisioning evidence

Research date: 2026-10-28. These are independently authored TalkOps profiles.
No FusionPBX source file, checksum or generated derivative is changed.
No Cisco 8800 template is reused.

## Firmware and capacity

| Model | MPP availability | SIP registrations | Enterprise availability | TalkOps variant |
| --- | --- | --- | --- | --- |
| 7811 | Yes | 1 | Yes | `cisco-7811-mpp` |
| 7821 | Yes | 2 | Yes | `cisco-7821-mpp` |
| 7841 | Yes | 4 | Yes | `cisco-7841-mpp` |
| 7861 | Yes | 16 | Yes | `cisco-7861-mpp` |

The [MPP data sheet](https://www.cisco.com/c/en/us/products/collateral/collaboration-endpoints/unified-ip-phone-7800-series/datasheet-c78-738031.html)
explicitly gives registration counts. These are account bounds, not counts of
simultaneous calls. The [MPP user guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/MPP/7800/english/userguide/pa2d_b_7800-user-guide-mpp-11/pa2d_b_7800-user-guide-mpp-11_chapter_00.html)
separates lines from fixed function keys. A button can hold one line or feature.
7811 has no programmable line buttons. TalkOps configures only direct account
assignments for the other models. The catalog `line_keys: 0` means no editable
TalkOps function keys. It does not describe the physical button count.

The [Enterprise data sheet](https://www.cisco.com/c/en/us/products/collateral/collaboration-endpoints/unified-ip-phone-7800-series/data-sheet-c78-729488.html)
lists all four Enterprise models. The [MPP support index](https://www.cisco.com/c/en/us/support/collaboration-endpoints/ip-phone-7800-series-multiplatform-firmware/products-maintenance-guides-list.html)
lists all four under the MPP 11.3(1) administration guide. This release is the
documentation baseline, not a claim of hardware certification or all-release support.

## Configuration and transport

The [7800 MPP provisioning format guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/MPP/7800/english/provisioning/pa2d_b_mpp-7800-provisioning-guide/pa2d_b_mpp-7800-provisioning-guide_chapter_01.html)
defines open XML with a `flat-profile` root. It documents HTTP, HTTPS and TFTP,
explicit Profile Rule paths, lowercase `$MA` expansion and `--uid`/`--pwd`
authentication options. TalkOps uses HTTP Basic authentication inside its existing
`/provisioning/devices/<mac>/` route. It adds no public MAC lookup or TFTP service.
The initial Profile Rule must be set manually. The fetched profile does not
replace it. HTTPS needs a certificate chain accepted by the phone.

The selected filenames are `CP-<model>-3PCC<lowercase-mac>.cfg`.
They follow the [Cisco per-phone profile example](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/MPP/7800/english/provisioning/pa2d_b_mpp-7800-provisioning-guide/pa2d_b_mpp-7800-provisioning-guide_chapter_011.html).
The explicit rule makes the name deterministic. It is not a claim that every
firmware requests that filename automatically. Each profile declares one file.
The XML has no device-file references. Existing route checks reject other MAC
filenames and undeclared files. Tenant provisioning credentials are shared by
the existing service; this change does not introduce per-device HTTP secrets.

The [7800 MPP sample configuration](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/MPP/7800/english/provisioning/pa2d_b_mpp-7800-provisioning-guide/pa2d_b_mpp-7800-provisioning-guide_appendix_0101.html)
provides the indexed SIP, registration and button parameter names. TalkOps binds
each assigned account to its numbered slot, clears unassigned credentials and
disables registration and line use. The proxy uses `hostname:port`. The account
context supplies UDP. The local SIP port remains a phone setting.
The [MPP troubleshooting FAQ](https://www.cisco.com/c/dam/en/us/td/docs/voice_ip_comm/cuipph/MPP/common/faq/MPP-Troubleshooting-FAQ-V1_2_1.pdf)
documents dial-plan syntax. The profile accepts digits, star, hash and leading
plus. This permits TalkOps internal `**` numbers and service codes. It does not
change TalkOps dialing permissions.

The [regional setup guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/MPP/7800/english/AG/pa2d_b_7800_mpp_ag_new/pa2d_b_7800_mpp_ag_new_chapter_010110.html)
defines NTP, GMT offsets, summer-time rules and dictionary setup. TalkOps maps
its supported Central European zones, London and UTC explicitly. Other zones
fail rendering. German requires a firmware-matched external dictionary.
No dictionary or firmware download is performed. Voicemail uses the documented
messages-key number. MWI behavior still needs a device test.
BLF and contacts are not exposed. Firmware support alone does not establish
TalkOps SIP interoperability for these additional features.

## Enterprise boundary and observed device

The [CUCM administration guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/7800-series/english/admin-guide/pa2d_b_7800-series-admin-guide-cucm.pdf)
describes CUCM-generated configuration and TFTP bootstrap. The
[security-by-default guide](https://www.cisco.com/c/en/us/support/docs/voice-unified-communications/unified-communications-manager-callmanager/116232-technote-sbd-00.html)
describes CTL/ITL and signed `SEP<MAC>.cnf.xml.sgn` files. CUCM also offers
[HTTP retrieval](https://www.cisco.com/c/en/us/support/docs/unified-communications/unified-communications-manager-callmanager/200769-Two-Ways-to-Obtain-a-Phone-s-Configurati.pdf)
through its TFTP service. That does not establish support for authenticated
TalkOps device URLs. TalkOps does not implement this bootstrap or trust service.
The initial MPP contribution did not implement this path. The follow-up below
adds a separate experimental, nonsecure Enterprise bootstrap. It does not
implement the CUCM trust service or certify third-party interoperability.

The user identified the available phone as `CP-7861-K9= V09` and then read
`sip78xx.14-2-1-0001-14` from Active Load after a factory reset.
The [Cisco individual-upgrade guide](https://www.cisco.com/c/en/us/support/docs/unified-communications/unified-communications-manager-callmanager/213288-upgrade-ip-phone-firmware-individually.pdf)
shows this exact load for Enterprise 7861 firmware. The device cannot validate
the MPP profiles in its current state. It is the Enterprise hardware test target. No configuration was sent to the phone.
No conversion, licensing or upgrade was performed.

## Implementation and verification

`profiles/cisco7800.json` is the explicit authored manifest. The shared authored
XML template lives under `templates/talkops/cisco7800/`. Build-time embedding is
separate from the pinned FusionPBX templates. Profiles reuse bounded rendering,
manufacturer schemas, encrypted settings, preview masking and device copies.
Account and identity fields remain managed and excluded from copies.
See [ADR 0048](../adr/0048-authored-provisioning-profiles.md).

The dedicated fixture tests parse XML, check all four capacities and sparse
assignments, compare credentials per slot, reject overflow and duplicate slots,
check locale mappings and verify declared filenames. Existing all-profile
fixtures cover every artifact and its references. API tests cover authenticated
retrieval, cross-MAC filename rejection, preview masking and independent copies.
Hardware validation remains `not_tested` for every profile.

## Enterprise follow-up

The user requested the Enterprise integration after identifying the firmware.
[ADR 0049](../adr/0049-cisco-enterprise-tftp-bootstrap.md) records the optional
transport exception. Four separate Enterprise IDs use an independently authored
`device/devicePool/sipProfile` XML contract. No 8800 template is reused.
The profile is experimental. Cisco's public administration documentation does
not certify this hand-authored configuration with FreeSWITCH. Automated fixtures
validate TalkOps bindings and file structure; a real device must validate the
firmware's acceptance of the XML and its SIP behavior.

The [7800 CUCM administration guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/7800-series/english/admin-guide/pa2d_b_7800-series-admin-guide-cucm.pdf)
describes DHCP TFTP discovery and the Alternate TFTP setting. The
[CME localization guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cucme/admin/configuration/manual/cmeadm/cmelocal.html)
shows MAC-specific uppercase `SEP<MAC>.cnf.xml` bindings and separate dialplan
files. TalkOps declares both its SEP file and `dialplan.xml`. Requests for
signed or trust files remain errors. An existing trust list can block this
nonsecure deployment; no bypass or automatic deletion is implemented.

The [Cisco phone security guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cucm/security/12_5_1SU5/cucm_b_security-guide-1251su5/cucm_m_phone-security_reorg.html)
associates the digest user with the phone. It does not establish independent
per-line identities for this integration. The one-account bound is a conservative
TalkOps limit, not a claim that the hardware has only one line. The
[CME phone feature guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cucme/feature/phone_feature/phone_feature_support_guide.html)
identifies 7811, 7821, 7841 and 7861 as supported Enterprise SIP models, including
16 lines for 7861. That line count is not used as a credential limit.

The Enterprise transport selector uses TCP and the registrar port from TalkOps.
The existing internal FreeSWITCH SIP profile already accepts TCP. It uses the
assigned username for the line and digest identity, with the assigned password.
No credentials are reused between devices. The profile omits firmware selectors,
external services and locale downloads. Device language remains unchanged.
Voicemail, NTP and the supported European/UTC time mapping are emitted.
The [Cisco SIP dialplan guide](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/7960g_7940g/sip/7_5/english/administration/guide/ver7_5/sipins75.html)
defines the `DIALTEMPLATE` format. TalkOps supplies a permissive five-second
collection rule without number rewriting. Acceptance on 14.2(1) remains a
hardware test; this reference is not evidence for all 7800 call features.

The [Debian tftpd-hpa manual](https://manpages.debian.org/trixie/tftpd-hpa/in.tftpd.8.en.html)
defines source-IP expansion, GET/PUT map constraints and transfer-port ranges.
The optional sidecar uses those controls with two exact file patterns. It has
no database access and mounts the private tmpfs export root read-only. It starts
as root with only `SETUID`, `SETGID` and `DAC_READ_SEARCH`, because tftpd-hpa
calls `setgroups()` before it serves each request as UID 10001. Plaintext TFTP requires a trusted LAN; source-IP mapping cannot stop
LAN spoofing or passive capture. Existing HTTP provisioning remains authenticated.

The exporter uses the existing tenant-scoped phone and credential paths.
It omits duplicates, invalid profiles and phones without a configured SIP host.
It replaces complete snapshots and revokes stale exports after errors. A
90-second lease stops the wire service if the exporter crashes. Tests cover
real TFTP transfers that drop to an unprivileged user, IP separation, malformed filenames, denied
uploads, private file permissions, duplicate mapping, removal and failure revocation.
No phone has yet downloaded these files or registered in a hardware test.

### Shared DHCP scope

The [Cisco 7800 network protocol table](https://www.cisco.com/c/en/us/td/docs/voice_ip_comm/cuipph/7800-series/english/admin-guide/pa2d_b_7800-series-admin-guide-cucm/pa2d_b_7800-series-admin-guide-cucm_chapter_010.html)
recommends DHCP option 150 with a TFTP server IP. The
[UniFi DHCP server guide](https://help.ui.com/hc/en-us/articles/360012097513-UniFi-DHCP-Server)
identifies its built-in TFTP field as option 66 and permits extra options by
code. Keep the existing Yealink HTTP(S) URL in option 66. Add a binary IPv4
address list in option 150 for Cisco. The phone must have Alternate TFTP
disabled to use DHCP discovery. Both real-device checks remain necessary.
No DHCP or gateway change has been performed by this task.
