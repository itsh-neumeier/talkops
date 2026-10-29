# Manufacturer settings and device copies

[Handbook](index.md) · [Deutsch](../de/hersteller-provisionierung.md)

## Before you start

You need: phones.manage and the exact device model and firmware documentation.

Select the exact model when you add a phone. The source library contains the
complete pinned FusionPBX provisioning tree. Spectralink and Grandstream Wave
have no usable configuration there. They remain visible as blocked entries.

## Edit manufacturer settings

Open a phone and find **Model settings**. Fields are grouped by function:
language and time, network, SIP, calls, audio, display, keys, contacts,
provisioning, maintenance and security. Search matches parameter names,
comments and source files. Open a category to show its fields. Use **Show more
fields** for the next page. **Only device overrides** filters configured values.

Each field shows its source parameter and file. Source comments describe
options where available. An empty value is an explicit empty value. **Use
template default** removes the override. Leave a saved secret field unchanged
to preserve it. Use its reset button to remove it. List settings accept a JSON
array of strings, for example `["112", "110"]`.

TalkOps manages SIP credentials and identity through their own controls.
Those fields are read-only in the manufacturer form. Source coverage is not
a complete firmware manual. Check the device manual for supported values and
additional firmware features. Hardware verification is shown separately from
source conversion. Successful rendering still requires device testing.

Set common language, IANA time zone and NTP values in **Provisioning transport**.
A phone can override them. Manufacturer fields expose device-specific locale
formats. A manufacturer field override takes precedence. Some phones need a
language dictionary. Use the manufacturer field and model manual where no
common-format mapping exists.

The account count follows source constraints. Without a source bound, the
profile starts with one account. Enter the count documented for the device.
Remove higher account assignments before you reduce this count.

## Deliver files

Native profiles list their provisioning directory and all configuration files.
Use the directory address or primary file as specified by the device manual.
The files require provisioning credentials. Configuration views and QR exports
require phone management permission and create audit entries.

App devices receive a generated ID if their identity field is empty. Assign a
SIP account before exporting an app profile. Cloud Softphone also needs its
cloud code. QR codes contain credentials. Treat them as secrets.

Profiles without remote command adapters use scheduled downloads or the device
menu. Their UI does not offer remote resync or reboot. TalkOps serves HTTP(S).
A phone that requires TFTP needs a separate delivery service. Swissvoice has
no directory artifact in the source. Its directory URL fields start empty.
Use a compatible external directory if required. Firmware files are not bundled.

## Copy a phone

1. Select **Use settings from a phone**.
2. Select the source and review the copy. The model stays fixed.
3. Enter a new name and MAC address. An app can generate a new ID.
4. Select new extension assignments if required.
5. Create the phone.

The copy gets independent SIP credentials. It excludes source credentials,
secret parameters, runtime state, source account labels and device-specific
fields. Keys without an assigned account stay inactive. Later source changes
do not update the copy. The operation sends no resync. Reload a stale preview
before creating a copy.

The source revision and license are in the source library. Native template
copies retain the model artifact manifest. Activation checks file structure,
credential bindings and empty, single and full account fixtures.

See [provisioning profiles](provisioning.md) for transport and template controls.

## Check the result

Check downloaded files and device behavior; a rendered template alone is insufficient.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.

## Cisco 7800 MPP phones

Select **7811 MPP / 3PCC**, **7821 MPP / 3PCC**, **7841 MPP / 3PCC** or
**7861 MPP / 3PCC**. These profiles use the Cisco MPP XML format.
The documentation baseline is MPP 11.3(1). All profiles are **untested on hardware**.
The stable model IDs end in `-mpp`.

| Model | Independent SIP accounts | Account line assignment |
| --- | --- | --- |
| 7811 | 1 | No programmable line button |
| 7821 | 2 | Buttons 1 to 2 |
| 7841 | 4 | Buttons 1 to 4 |
| 7861 | 16 | Buttons 1 to 16 |

A SIP account is one registration with its own credentials. A line button can
show that account. Multiple calls on one account do not add registrations.
TalkOps assigns each account to the button with the same number. Unassigned
slots are disabled and their credentials are cleared. TalkOps does not expose
custom button functions, BLF or contacts for these profiles.

### Check the firmware first

MPP and Enterprise/CUCM firmware use different configuration formats.
A `3PCC` PID identifies the MPP product. Check the installed load as well.
An Enterprise PID alone does not prove which firmware is installed.
Do not select an MPP profile for Enterprise firmware.

Enterprise firmware uses the separate experimental profiles described below.
Do not use an MPP XML file for an Enterprise phone. Existing Cisco 8800 profiles
do not establish compatibility with the Enterprise 7800 series.

### Set up an MPP phone

You need an unlocked MPP phone, device administration access, DHCP, DNS and
network access to the TalkOps provisioning and SIP endpoints. For HTTPS, the
phone must trust the server certificate. TalkOps does not disable validation.
A manual HTTP endpoint is an option only on a trusted network.

1. Add the phone with the exact MPP model and its MAC address.
2. Assign extensions to the required account numbers.
3. Open the phone configuration files in TalkOps.
4. Copy the full primary file URL, without embedded credentials.
5. Open the phone web interface as administrator in advanced mode.
6. Open **Voice > Provisioning**.
7. Enter a rule with the provisioning username and password shown by TalkOps:

   ```text
   [--uid PROVISIONING_USER --pwd PROVISIONING_PASSWORD] https://pbx.example/provisioning/devices/805ec0010203/CP-7861-3PCC805ec0010203.cfg
   ```

8. Replace the host, credentials, model number and MAC with the actual values.
9. Submit the changes and start a resync from the phone.
10. Check the TalkOps provisioning request list for a successful file request.

Use the **provisioning** credentials, not the web login, SIP account or device
admin password. The explicit URL determines the filename. `$MA` can replace
the lowercase MAC in the filename, but keep the actual MAC in the device path.
Do not enter only the directory URL. Do not use DHCP TFTP defaults as this
bootstrap. Keep Profile Rule B/C/D empty on a dedicated TalkOps phone.
The generated file preserves the initial Profile Rule and contains no download
references. It sets the TalkOps phone admin password. Keep that password before
you resync. Later downloads use the same stored Profile Rule and authentication.

SIP uses the assigned username, distinct password, display name and configured
registrar host and port. The current TalkOps native account context uses UDP.
The proxy field contains the destination port; `SIP_Port` is the phone local
port and is not the registrar port. This profile leaves the local port unchanged.

NTP and voicemail use the common TalkOps settings. Time-zone mapping supports
UTC, Europe/London and the Central European zones Berlin, Vienna, Zurich,
Paris, Rome, Madrid and Amsterdam. Other zones produce an explicit render error.
The supported European zones include their summer-time rule. Manufacturer
fields can override the emitted time values.

English uses the built-in language. German selects `German` and `de-DE`.
German display text also needs a firmware-matched Cisco dictionary. Host the
licensed dictionary separately and set **Dictionary_Server_Script** in model
settings, for example `serv=https://locales.example/;d1=German;x1=ACTUAL_FILE.xml`.
TalkOps does not bundle dictionaries or firmware. That external server has its
own access controls; TalkOps device-directory authentication does not apply there.

### Test a device

Record the exact PID, hardware revision and active load first.
The available device is `CP-7861-K9= V09`, with active load
`sip78xx.14-2-1-0001-14` (Enterprise 14.2(1)). Its firmware does not accept this
MPP profile. Use the Enterprise test below instead. A factory reset does not
establish MPP compatibility. Do not change firmware as part of this test.

For a phone with a verified MPP load:

1. Record its PID, firmware, model ID and test date.
2. Check that a request without authentication receives HTTP 401.
3. Resync and check HTTP 200 for the declared primary filename.
4. Check that another MAC filename in this device directory receives HTTP 404.
5. Check registration for two different assigned accounts, if the model permits it.
6. Make and receive a call on each assigned account.
7. Check bidirectional audio, hold, transfer and DTMF.
8. Check the account button labels and disabled unassigned slots.
9. Check voicemail access with the messages key and record the MWI result.
10. Check NTP time, summer time and the configured display language.
11. Restart the phone and check that authenticated resync and registration still work.
12. Save the results without credentials and leave failed steps explicit.

BLF, custom keys, contacts and remote commands are outside this profile test.
Successful XML rendering is not a hardware test.

See [Cisco evidence and implementation limits](../research/cisco-7800.md).

## Cisco 7800 Enterprise SIP phones (experimental)

The separate `cisco-7811-enterprise`, `cisco-7821-enterprise`,
`cisco-7841-enterprise` and `cisco-7861-enterprise` profiles target nonsecure
Enterprise SIP provisioning. The initial firmware target is 14.2(1), including
`sip78xx.14-2-1-0001-14`. A `CP-7861-K9` with this load downloaded the SEP
file and `dialplan.xml` and registered over SIP TCP on 2026-10-28. Calls,
audio, DTMF and voicemail are **not tested yet**. 7811, 7821 and 7841 remain
**untested on hardware**.
These are TalkOps integration profiles, not Cisco certification for FreeSWITCH.

Each Enterprise profile permits **one independent SIP account in slot 1**.
The phone can have more physical line buttons. Those buttons do not prove
support for independent digest credentials per line. TalkOps does not share or
weaken account passwords to add more lines. MPP account limits remain unchanged.
Custom buttons, BLF, firmware updates and locale downloads are absent.
TalkOps supplies resync, restart, ringtones and the phone book (see below).
The installed phone language stays unchanged. NTP and the supported UTC,
Central European and London time zones use the common settings.

### Enable the optional TFTP deployment

Use a Linux Docker host and a server image built from this contribution.
Older released images do not contain this feature. TFTP has no authentication
or encryption. Enable this deployment only on a trusted voice LAN. Source-IP
admission does not prevent an attacker on that LAN from spoofing or observing
traffic. Keep the regular authenticated HTTP provisioning service unchanged.

1. Reserve a fixed IPv4 DHCP address for the phone.
2. Add the exact **Enterprise SIP (experimental)** model in TalkOps.
3. Enter the phone MAC and assign an extension to account 1.
4. Set the SIP host to the PBX address reachable from the phone.
5. In **Model settings**, open provisioning and set **TFTP client IPv4 address**
   to the reserved phone address. Each exported phone needs a unique address.
6. Set `TALKOPS_TFTP_BIND_IP` in the deployment `.env` to the trusted LAN address
   of the PBX host. This is the server address, not the phone address.
7. Start the optional deployment:

   ```sh
   docker compose -f docker-compose.yml -f docker-compose.cisco-tftp.yml up -d
   ```

8. Restrict UDP 69 and UDP 30000–30031 to the phone LAN through the host firewall.
9. Permit SIP TCP on the configured phone SIP port (default 5060) and the
   existing FreeSWITCH media port range.

The sidecar maps host UDP 69 to unprivileged container UDP 1069. Its transfer
ports use UDP 30000–30031. tftpd-hpa must start as root to drop privileges for
each request. The sidecar keeps only the `SETUID`, `SETGID` and `DAC_READ_SEARCH`
capabilities, uses `no-new-privileges` and a read-only filesystem. Each transfer
runs as UID 10001. Docker must preserve the real LAN client IP. A proxy or
NAT that replaces all client addresses cannot use this admission rule.

TalkOps exports only the declared SEP configuration, `dialplan.xml` and, with a
selected ringtone, `Ringlist-wb.xml` and the `.rwb` ringtones into a
private, volatile tmpfs volume. The sidecar mounts that volume read-only.
Files use mode 0600 and directories use 0700. The exporter refreshes snapshots
every 30 seconds. Duplicate client IPs, missing SIP hosts and invalid profiles
are omitted. Removed assignments and phones lose their old export on refresh.
Database errors revoke the served snapshot. The snapshot lease expires after
90 seconds, including after a TalkOps crash. The guard checks each second and
allows five seconds for shutdown before it kills the TFTP process group.
The volume is separate from persistent provisioning storage and backup data.
The client IP is excluded from phone copies.

### Use DHCP for Yealink and Cisco on the same network

Keep one DHCP server for the voice network. Supply two different options:

| Client | DHCP option | Value |
| --- | --- | --- |
| Existing Yealink phones | 66 | Keep the existing HTTP(S) `/provisioning` URL unchanged |
| Cisco Enterprise phones | 150 | The IPv4 address of the optional TFTP server, without a URL path |

Cisco recommends option 150 for the 7800 series. UniFi permits additional
custom DHCP options by code. Add option 150 in the voice network's DHCP
settings. Encode the value as an IPv4 address list, not an ASCII URL or host
name. Exact field names depend on the UniFi version. Preserve the Yealink
option lookup order; do not switch those phones to option 150.

After the optional TFTP deployment is running, leave **Alternate TFTP** disabled
on the Cisco so that DHCP can supply the server. Renew its lease or restart it.
Check that the phone reports the intended TFTP Server 1 address. Verify a
Yealink resync as a regression check. If a DHCP server cannot provide separate
options, use the Cisco manual override below. Do not replace the shared option
66 URL with a bare TFTP address.

The TFTP client IP reservation is still required for export admission. DHCP
server discovery and the per-phone file admission rule are separate controls.
Routing and firewall rules must permit traffic if the phone and PBX use
different VLANs. TalkOps does not change the DHCP server or gateway settings.

### Set the phone bootstrap address

1. Press **Applications > Admin settings > Network setup > IPv4 setup**.
2. Set **Alternate TFTP** to **Yes**.
3. Set **TFTP Server 1** to the LAN IP from `TALKOPS_TFTP_BIND_IP`.
4. Apply the settings and restart the phone manually.

Menu names depend on the installed firmware. DHCP option 150 can provide the
same server IP instead of the manual setting. Do not enter an HTTP URL or
TalkOps provisioning credentials in the TFTP address field.

For example, a phone with MAC `80:5E:C0:01:02:03` requests
`SEP805EC0010203.cnf.xml` and then `dialplan.xml`. It uses SIP TCP and its own
TalkOps digest credentials. The HTTP exports and redacted previews still work
through the normal managed device routes. TFTP downloads do not appear in the
HTTP provisioning request list. Use phone status messages and SIP registration
to check the hardware result. Export logs confirm file generation only.

The sidecar refuses uploads, path traversal, other phones' filenames, generic
fallback profiles, firmware and signed/trust files. Missing CTL/ITL requests
are not answered with fake files. A phone with an existing CUCM trust list can
reject unsigned configuration. Stop if that occurs. Use the administrator's
approved Cisco migration procedure. TalkOps does not clear trust lists or
replace certificates. This integration does not implement secure CUCM mode,
CAPF, TLS certificates or a firmware conversion.

### Resync, last contact, ringtones and contacts

**Resync and restart.** The **Resync** and **Restart** buttons on the phone
page send a SIP NOTIFY with `Event: service-control` to the registered phone.
Resync sends `action=restart`: the phone loads all its files again over TFTP
and registers again. Restart sends `action=reset`: the phone does a full
restart. The phone accepts the NOTIFY only with the Call-ID of its current
REGISTER, so the phone must be registered. If it is not registered, the page
shows that nothing was sent. Each request writes an audit entry.

Each SEP file contains a `versionStamp` that is a hash of the file content.
A changed setting thus also changes the stamp.

**Last contact.** The phone loads its files over TFTP, which TalkOps does not
see. TalkOps thus records the SIP registration of a Cisco Enterprise phone as
its last contact. The phone page and the side panel show the time, the IP
address and the firmware version from the user agent (for example `14.2.1`).
TalkOps updates these values at most once a minute.

**Ringtones.** Upload a ringtone under **Settings → Phones** (WAV, 8 kHz,
mono, 16 bit, at most 160 KB). Select it on the phone page. TalkOps converts
all uploaded ringtones into the Cisco wideband format (`.rwb`, 16 kHz, at most
10 seconds; longer tones are cut). The TFTP export adds `Ringlist-wb.xml` and
the `.rwb` files, with the selected ringtone first. The configuration file
cannot select a custom ringtone. Select it on the phone under
**Settings → Ringtone** after the next resync. Without a selected ringtone,
TalkOps exports no ring list. At most 50 ringtones and 8 MB per phone are
exported.

**Contacts.** The SEP file contains a `directoryURL`. The **Directories** or
**Contacts** key on the phone then shows the TalkOps phone book: the enabled
extensions (dial with `**`), the global contacts and the contacts of the
phone book sections selected for this phone. The list has pages of 32
entries and a search key.

The phone cannot send the provisioning user name and password. The URL thus
contains a per-phone token instead. **Regenerate** the provisioning credentials
to make all old directory URLs invalid. The URL contains no SIP password.

TalkOps needs an address that the phone can reach. The TFTP export does not
know the address of TalkOps, so set a phone provisioning URL. Without this
URL, the SEP file has no `directoryURL`. Enterprise firmware checks HTTPS
certificates against its own trust list. A certificate of a public CA or of
the TalkOps domain can thus fail. Use a plain HTTP endpoint on the trusted
voice LAN, for example `http://192.168.140.30:8080/provisioning`, in the
phone's **Phone provisioning URL** setting. HTTP is not encrypted: the phone
book and the token are visible on the LAN. Use this only on a trusted LAN.

**German display and tones** need Cisco locale files that TalkOps does not
supply. They are not available yet.

### Test the available Enterprise phone

The available test model is `CP-7861-K9= V09`, with Enterprise 14.2(1).
Steps 1 to 5 passed on 2026-10-28. The other steps are open.

The status message "Error Verifying Config Info" means that the phone rejected
the content of the SEP file. Releases up to 1.8.15 produce such a file. Use a
release that contains the corrected Enterprise template. The template enables
the phone's read-only status web pages at `http://<phone-ip>`. Use **Device Logs**
there to find the cause of a provisioning or registration problem. SSH stays off.

1. Record the PID, hardware version and active load without publishing the MAC.
2. Select `cisco-7861-enterprise` and complete the fixed-address setup above.
3. Preview the configuration and confirm that passwords are masked.
4. Check a successful TFTP download of the exact SEP file and `dialplan.xml`.
5. Confirm SIP TCP registration for the assigned account in TalkOps.
6. Make an outgoing internal call with `**` and receive an incoming call.
7. Check audio in both directions, DTMF, hold and transfer.
8. Check voicemail access, time and the assigned line label.
9. Restart the phone and check configuration retrieval and registration again.
10. Click **Resync** and check that the phone loads its files and registers again.
11. Click **Restart** and check that the phone restarts.
12. Check **Last contact**, IP address and firmware on the phone page.
13. Select a ringtone, resync, and select it on the phone under **Settings → Ringtone**.
14. Open **Contacts** on the phone, page through the list and search a name.
15. Remove the TFTP client IP, wait for a refresh, and confirm that downloads fail.
16. Record results and failures before changing any hardware validation status.

Do not change the firmware or trust lists during this test. Registration,
calling, MWI and feature behavior remain open until this test is performed.
