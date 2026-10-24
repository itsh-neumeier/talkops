# Phone provisioning profiles and templates

[Handbook](index.md) · [Deutsch](../de/provisionierung.md)

## Before you start

You need: phones.manage, a supported device and its reachable provisioning address.

See [manufacturer settings and device copies](manufacturer-provisioning.md)
for the additional vendor profiles and categorized fields.

## Select an address for the phones

Open **Phones → Provisioning transport**. Set the phone provisioning URL and,
if necessary, the SIP server host. Use `/provisioning` as the URL path. The SIP
host has no scheme or port. Empty values keep the existing request address.
The web UI can use a different address.

For a phone that needs a different transport, open the phone and set the URL
under **Device provisioning**. For example, a T41P can use
`http://192.168.1.10:8095/provisioning` while an AX86R uses the server HTTPS URL.
Use HTTP only on a trusted phone LAN. The exception also applies to phonebooks,
firmware, media, status callbacks and softkey files. TalkOps does not disable
certificate checks or change HTTPS to HTTP automatically.

The phone must first reach TalkOps to download this exception. If its initial
HTTPS connection fails, enter the HTTP URL and provisioning credentials on the
phone once, or set that URL in its device-specific DHCP option 66. After a
successful download, the device configuration keeps the selected HTTP address.

DHCP option 66 distributes a server value. Its name in a DHCP UI can be
"TFTP server", but an `http://` or `https://` URL requests that protocol on a
phone that supports it. TalkOps serves HTTP(S), not a TFTP service. Use the URL
with credentials shown in **Auto-provisioning** for DHCP. When you enter the
address manually, use the separate username and password fields on the phone.
Do not publish the URL with credentials.

## Configure keys

The existing graphical line-key editor stays available. Model metadata controls
the offered functions. Additional line functions include voicemail, direct
pickup, DTMF, hold, transfer and conference where the profile permits them.

**Device provisioning** has separate groups for programmable keys, expansion
modules and call-state softkeys. The T41P has 13 programmable controls: four
idle softkeys, the direction keys, OK, cancel, mute, redial and message.
The programmable IDs are 1–10, 13, 17 and 18. The last two are not 11 and 12.
The T41P has no supported expansion module profile.

1. Select a key and its function.
2. Set the fields that the function uses. A local menu needs no SIP account.
3. For speed dial, select a contact number or enter a number such as `**101`.
4. For an XML phonebook, select the internal, global or assigned phonebook.
5. Select **Apply**, then **Save** or **Save & resync** on the phone page.

A direct pickup target such as `101` becomes `*8*101`. An explicit feature
code stays unchanged. A custom mute-key assignment also disables Yealink's
keep-mute setting. A reset restores that setting.

**Inherited / factory default** removes an override. **Unused** explicitly
clears a key. A reset sends the profile default on the next configuration
download, so the old value does not stay on the phone. A call-state layout has
four ordered actions. Its **Default** button removes that state override.

The normal key profile uses Yealink V83+ codes. The explicit `fusionpbx-legacy`
profile uses older source-registry codes for directory and account switching.
The modern XML phonebook action uses XML Group (type 22) and the selected
phonebook index. The source legacy profile uses its older type 47 entry.
Select a legacy profile only when the firmware needs it. Imported comments
and registries contain differences; the source tree is not a compatibility
certificate. All new key and layout behaviour needs a real-device check.
SMS, BroadSoft, shared-line and ACD source entries do not enable those services
in TalkOps.

## Preview and adapt templates

Use **Configuration preview** on a phone to inspect its draft parameters and
the difference from its saved configuration. Generated SIP, provisioning and
phone-admin passwords are replaced before rendering. The existing raw config
view is a separate privileged, audited action and contains real credentials.

Open **Phones → Provisioning templates** to inspect all 433 source files in
the 70 FusionPBX Yealink directories. The source revision is pinned. A source
file can contain parameters copied from another model. Raw Smarty files are
references; TalkOps does not execute them.

To adapt a native template:

1. Select **TalkOps templates**.
2. Enter a name and select a model. Select **Clone**.
3. Edit `common.cfg` or `mac.cfg`. Keep the header as the first line.
4. Use the variables and loops from the original native files. Keep secret
   values as variables; do not insert real passwords into template source.
5. Select **Save new revision**. The active revision does not change.
6. Select **Validate and activate**. Failed validation keeps the old revision.
7. Select the active template on the matching phone, save and resync it.

Each source file is limited to 64 KiB. Rendering has evaluation, recursion and
output limits. Templates have no filesystem or network access. A template is
scoped to its tenant and model. To restore an old version, select its revision
and activate it again. Changing a template does not send a resync automatically.

## Show network data of the phones

With a connection to the Ubiquiti Networks API (see
[Wi-Fi survey](wifi-survey.md#connect-the-ubiquiti-networks-api)), the phone
list can show the network state of each provisioned phone:

1. Open **Settings → Wi-Fi survey → Ubiquiti Networks API**.
2. Under **Phones**, select **Show network data on phones**. The setting
   needs the permission `unifi.manage`.

The phone list then has the column **Network**. A Wi-Fi phone shows the
signal in dBm, the access point, the band, the channel and the SSID. A wired
phone shows the switch, the port and the link speed. "not connected" means
that the console does not report the MAC address of the phone. On narrow
screens, the column is hidden like the other optional columns.

The page of each phone shows the card **Network** with more values: IP
address, network and VLAN, connection time, data sent and received, data
rate and the UniFi experience value.

TalkOps reads all clients with one request and matches them to the phones by
MAC address. TalkOps uses an answer again for 15 seconds and reads again each
30 seconds while a phone page is open. TalkOps only reads and never changes
the console. Users with `phones.view` see the data.

## Diagnose a resync that does not download

Resync sends a SIP NOTIFY. The notification result does not prove that the
phone downloaded its configuration. In **Provisioning transport**, refresh
**Recent configuration requests**. Look for the phone's MAC CFG request
after the notification.

- `200`: TalkOps returned the requested file. Check the phone for application.
- `401`: Check the provisioning username and password on the phone or in DHCP.
  An initial challenge followed by `200` is normal Basic authentication.
- `404`: Check MAC, model, filename and the registered phone.
- `429`: Failed authentication reached the rate limit.
- No new request: Check connectivity, DNS, phone time, TLS/certificate support
  and whether the phone uses the intended provisioning address.

For the reported T41P firmware `36.83.0.120` case, an AX86R works with HTTPS
through DHCP 66, while T41P resync works with internal HTTP. This is not proof
of a general HTTPS failure or missing TFTP support. Compare manual HTTPS and
manual HTTP on the T41P. Then compare the device-specific DHCP replies.
A TLS failure before HTTP reaches TalkOps leaves no request record here.
Use an explicit phone HTTP exception as a controlled LAN workaround while
you check firmware, trust chain, TLS and authentication. Do not disable TLS
verification for all phones.

## Upgrade

Migration `20261105000001_provisioning_profiles` adds phone overrides, transport
settings, template revisions and request records. Existing secrets and account
assignments do not change. Existing API clients that omit `provisioning` keep
the saved overrides. Resync a phone after you change its keys or template.
Do not interpret an old "last seen" value as a successful new resync.

## Check the result

Check the actual device download, account registration and a test call.

For failures, use [troubleshooting](troubleshooting.md). Preserve existing data and configuration during diagnosis.
