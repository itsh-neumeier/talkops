# Extensions and devices

[Handbook](index.md) · [Deutsch](../de/nebenstellen-und-geraete.md)

An extension is an internal number and its call settings. A user can own
extensions. A device is a SIP identity assigned to an extension. A provisioned
phone can contain several accounts. Do not use a web login as a SIP password.

## Before you start

An administrator can perform all steps. Other roles need `extensions.view` and
`extensions.manage` for extension administration, `users.manage` for user changes,
`phones.manage` for provisioning and `trunks.manage` for incoming number changes.
Users can manage permitted own devices and call settings in **Settings → My phones**.
See [users and roles](users-and-roles.md).

## Create an extension

1. Create the user under **Settings → Users**, if needed.
2. Open **Settings → Extensions → New extension**.
3. Enter an unused internal number and a descriptive name.
4. Assign the user and choose an outgoing number or the system default.
5. Set ring time, recording and video options as required.
6. Save and open the extension.

New internal numbers have 3–8 digits, start at 100 and cannot start with zero.
Emergency numbers, their prefixes and reserved service numbers are excluded.
The form reports conflicts. Existing short numbers can remain after an update.
Use [first steps](first-steps.md) for the current dialing rules.

## Extension side panel

Select an extension in the list to open its side panel: **Overview** (user,
outgoing number, state with DND and forwarding), **Devices** (with a link to
a provisioned phone) and **Settings** (**Open all settings**, **Edit** and
**Delete**). The full extension page keeps all editors.

## Add a manual SIP device

1. Select **Add device** on the extension or **My phones**.
2. Select its device type and save.
3. Open **Show credentials** and copy the SIP server, username and password to the device.
4. Check registration, then call a second extension.
5. Confirm ringing and speech in both directions.

Expected result: the device is online and calls the other extension.
All devices of an extension ring together unless call settings prevent it.
Changing a SIP username or generating a new password requires reconfiguration
of the device. Credentials and exported QR codes must stay private.
For device password limits, use the offered format and the
[Dahua instructions](door-station-dahua.md) where applicable.

For automatic configuration, follow [provisioning](provisioning.md) and
[manufacturer settings](manufacturer-provisioning.md). Avoid creating a second
manual identity for an account that provisioning already manages.
See [codec profiles](codec-profiles.md) for audio and video settings.

## Change DND and forwarding

1. Open **Settings → My phones** and find the extension.
2. Set **Do not disturb**, or enter **Forward all calls to**.
3. Save and call the extension from a second phone.
4. Clear the forwarding field or disable DND when no longer needed.

An internal number or an external number can be a forwarding target. External
forwarding needs a usable trunk and may create provider charges.
The equivalent codes include `*78`/`*79` for DND and `*72<number>`/`*73`
for forwarding. The [feature code table](yealink.md#4-feature-codes) lists more.

## Incoming numbers and common failures

In the extension edit dialog, select **Incoming numbers that ring here**.
Review the displayed current target: adding the extension can replace an
existing group or other destination. Use [trunks and numbers](trunks-and-numbers.md)
for number routing.

- Empty **My phones**: check the user assignment on the extension.
- Offline device: check SIP credentials, address, port and SIP login bans.
- Online but silent: check DND, forwarding, ringing time and route.
- No media or video: check [codecs](codec-profiles.md) and [network diagnostics](troubleshooting.md).
