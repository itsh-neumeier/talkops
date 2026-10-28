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

Enterprise firmware is **unsupported** by this integration. Its bootstrap uses
Cisco configuration and trust services, usually through TFTP or CUCM HTTP
services. TalkOps does not provide CUCM, CTL/ITL signing or a TFTP server.
An existing Cisco 8800 template does not prove 7800 Enterprise compatibility.
A plain authenticated TalkOps file URL does not replace these services.
No Enterprise profile is selectable. No conversion or firmware update is included.

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
MPP profile. Stop the MPP test at the firmware check. A factory reset does not
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
