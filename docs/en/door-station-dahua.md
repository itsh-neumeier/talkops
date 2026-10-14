# Dahua VTO and VTH

This page tells you how to connect Dahua VTO door stations and Dahua VTH
indoor monitors to TalkOps. The general functions (ring target, unlock
codes, pictures, Home Assistant) are on the page [Door stations](door-station.md).
TalkOps also shows this guide in **Settings → Door stations → Setup guide**.

> **Status:** Menu names differ between models and firmware versions.
> Text marked *not confirmed* is not tested with real devices.

| Model and firmware | State |
|---|---|
| VTO4202F-P-S2, V4.600.0.3 | Registers to TalkOps (field report). |
| VTH5422H, V4.800.0.2 | `403 Forbidden` with a 20-character password. The test with a 15-character password is open. |
| VTO2202F, VTO2211G, VTO3221E | Tested with a SIP simulator and a re-created HTTP API, not with real hardware. |

## Before you start

- Give each device its own extension and its own SIP user name. Do not use
  one SIP account on two devices.
- Keep the SIP password and the device login separate. See
  [SIP password and device password](#sip-password-and-device-password).
- Make sure that each device can reach the TalkOps server on UDP port
  `5060`.

Example plan:

| Device | Extension | SIP user name |
|---|---|---|
| VTO | `8000` | `8000vto` |
| VTH 1 | `8001` | `8001vth` |
| VTH 2 | `8002` | `8002vth` |

## SIP password format

Dahua devices can accept fewer characters than the standard SIP password of
TalkOps (20 characters). The Yeastar guide for Dahua VTH devices sets a
length of 8–15 characters.

- When you add a device of the type **Door station**, TalkOps selects the
  format **Compatible (15 characters)**. You can change the format in the
  dialog.
- For a device that exists: select **New password**, then **Compatible
  (15 characters)**, then **Generate new password**. The old password stops
  working immediately. Enter the new password in the device.
- A change of the format with **Edit** does not change the current
  password. The format applies to the next new password.
- TalkOps does not change the passwords of devices that work.

*Not confirmed:* the 15-character limit for the VTH5422H with firmware
V4.800.0.2. A successful softphone test (for example MicroSIP) does not
replace a test with the real device.

## VTO (door station)

### Settings in TalkOps

1. Open **Settings → Extensions** and create an extension for the VTO, for
   example `8000` "Front door".
2. In the extension, select **Add device**. Set the type to **Door
   station** and the SIP user name ending to `vto` (SIP user name
   `8000vto`).
3. Keep the SIP password format **Compatible (15 characters)**.
4. Save. Copy the SIP user name and the SIP password.
5. Open **Settings → Door stations → New door station**. Set the
   manufacturer to **Dahua** and select the extension of the VTO.
6. Select the ring target in **Ring goes to**. For the door opener,
   pictures and the event log, enter the HTTP access of the VTO (IP
   address, port, web login).
7. Select **Test connection**. TalkOps shows the model of the VTO.

### Settings in the VTO web interface

Sign in to the web interface of the VTO. Go to **Network → SIP Server**
(some firmware: **Network Settings → SIP Server**). The names follow the
Dahua guides (VTO quick start guide v4.5, VTO2311R-WP manual).

| VTO field | Value |
|---|---|
| SIP Server (Enable) | **Off**. TalkOps is the SIP server. |
| Server Type | *ThirdParty* (some firmware: *Asterisk*) |
| IP Addr. | IP address or host name of the TalkOps server |
| Port | `5060` |
| Username + **Enable** | Select **Enable**. Enter the SIP user name from TalkOps, for example `8000vto`. Without *Enable* the VTO uses its own VTO number. |
| Password | the SIP password from TalkOps |
| SIP Domain | `talkops.local` or the Dahua default `VDP`. TalkOps accepts each domain. |
| SIP Server Username | **the same** SIP user name as in *Username*, not `admin` |
| SIP Server Password | **the same** SIP password |

TalkOps accepts a registration only when the authentication user name is
the same as the SIP user name. Thus *Username* and *SIP Server Username*
must be identical.

- **Local Settings → Basic → Call No.** (default `9901`) is the number that
  the VTO calls for a ring. TalkOps sends each call from the VTO to the ring
  target. For a station with more buttons, set a target for each button
  number in TalkOps under **Buttons**.
- Optional: an unlock code in the VTO. Select **only one** DTMF method:
  **RFC 2833**. According to Dahua, no method works when you select RFC 2833
  and SIP INFO together.

### Save, restart and check

1. Select **OK** or **Save**.
2. Restart the VTO when the registration state does not change after one
   minute (for example **Maintenance → Reboot**).
3. In TalkOps, open the **Dashboard**. *Registered devices* shows
   `8000vto`.
4. Push the button on the VTO. The ring target rings.

## VTH (indoor monitor)

Each VTH registers to TalkOps as its own SIP device. The VTO and all phones
can then call the VTH.

**Important:** the VTH sends its **Room No.** as the SIP user name. Thus the
Room No. must be identical to the SIP user name in TalkOps, for example
`8001vth`. Use the same upper-case and lower-case letters.

### Settings in TalkOps

1. Open **Settings → Extensions** and create an extension for each VTH, for
   example `8001` "Indoor monitor hall".
2. In the extension, select **Add device**. Set the type to **Door
   station** and the SIP user name ending to `vth` (SIP user name
   `8001vth`). TalkOps has no separate type for indoor monitors. The type
   only sets the default user name and the password format.
3. Keep the SIP password format **Compatible (15 characters)**.
4. Save. Copy the SIP user name and the SIP password.

### Settings in ConfigTool

1. Activate the VTH with ConfigTool and set its device login.
2. Open **ConfigTool → Device Config**. Select the IP address of the VTH and
   **Get Device Info**. Sign in with the device login, not with the SIP
   password.
3. Enter these values:

   | ConfigTool field | Value |
   |---|---|
   | Room No. | the SIP user name from TalkOps, exactly, for example `8001vth` |
   | VTH | **Main VTH**. Each VTH that registers to TalkOps is a main VTH. |
   | Sip Server IP | IP address or host name of the TalkOps server |
   | Sip Server Port | `5060` |
   | Sip Register Pwd | the SIP password from TalkOps |
   | Domain (if shown) | `talkops.local` or `VDP` |
   | Initiative Mode (Yeastar guide: "Initiale Mode") | On. The VTH registers to the SIP server. |
   | Login Password / user name of the main VTH | **Not** the SIP password. ConfigTool fills in these values. Do not change them for TalkOps. |

4. Select **OK**. ConfigTool shows a message when the VTH accepts the
   settings.

### Settings on the VTH screen

Some firmware versions show the same values on the VTH (**Settings →
project password → Network** or **SIP Server**).

- Make a change in ConfigTool or on the VTH screen, not in both at the same
  time.
- Make sure that the VTH screen shows the same Room No., SIP server and
  port as ConfigTool.

*Not confirmed:* the menus on the VTH screen for firmware V4.800.0.2.

### Main and extension devices

- Set each VTH to **Main VTH** with its own Room No. and its own SIP
  account.
- To ring more than one VTH, use a ring group (see
  [Door stations](door-station.md#ring-target-and-ring-group)).
- A Dahua extension VTH (Room No. with `-1`, `#1` or similar) connects to its
  main VTH in the Dahua system, not to TalkOps.

*Not confirmed:* extension VTH devices that connect through a main VTH are
not tested with TalkOps.

### Save, restart and check

1. Select **OK** in ConfigTool.
2. Restart the VTH (ConfigTool or power supply).
3. In TalkOps, open the **Dashboard**. *Registered devices* shows the SIP
   user name, for example `8001vth`.
4. Call the extension of the VTH from a phone. The VTH rings.

## SIP password and device password

| Password | Where to enter it | Purpose |
|---|---|---|
| SIP password (from TalkOps) | VTO: *Password* and *SIP Server Password*. VTH: ConfigTool → *Sip Register Pwd* | SIP registration and calls to TalkOps |
| Web login of the VTO | TalkOps: door station → **HTTP access** | Door opener, pictures and event log. Not for SIP. |
| Device login of the VTH | ConfigTool sign-in, web interface | Configuration of the VTH. TalkOps does not use it. |
| Login Password (main VTH) | ConfigTool → Local Info | Connection between Dahua devices. Not the SIP password. |

- Use different passwords. A new SIP password in TalkOps does not change the
  device login. A new device login does not change the SIP password.
- The number of masking characters (`•••`) in ConfigTool does not show the
  real length of a stored password.

## Troubleshooting for Dahua

| What you see | What to do |
|---|---|
| No SIP packets reach TalkOps | Check the SIP server address and port in the device. Check network, VLAN and firewall. VTO: *SIP Server (Enable)* must be off. Restart the device. |
| `401 Unauthorized` only | This is the normal first answer. The device must send a second REGISTER with the password. If it does not, enter the SIP password again and restart the device. |
| `401`, then `403 Forbidden` | (1) Wrong or shortened password: generate a new password with the format **Compatible (15 characters)** and enter it. (2) Unknown SIP user name: the Room No. of the VTH is different from the SIP user name. (3) VTO: *SIP Server Username* is different from *Username*. (4) TalkOps banned the address (see [SIP login protection](door-station.md#sip-login-protection)). |
| `200 OK`, but a function does not work | See [Troubleshooting](door-station.md#troubleshooting). |
| Call from the VTO: audio works, the VTH shows no picture | TalkOps offers only the video codec of the VTO (H264) to the called devices. FreeSWITCH does not convert video. A device that supports only VP8 gets no picture. Make sure that the VTO sends H264. |
| Call to the VTO ends after 30 seconds | The VTO V4.600 does not accept the ACK when the caller name has umlauts. TalkOps sends caller names in ASCII on calls to and from door stations (`ü` → `ue`). The call history keeps the full name. Older TalkOps versions: use a name without umlauts. |

The TalkOps log shows the SIP user name and the address of each failed
login. A line `SIP authentication for unknown or disabled user` with the
user `8001` tells you that the VTH sends `8001` as the Room No., but TalkOps
expects a different user name.

```sh
docker compose logs -f talkops | grep "SIP authentication"
```

## Sources

- [Yeastar: Register Dahua VTH indoor monitor](https://help.yeastar.com/en/p-series-appliance-edition/integrations/dahua/register-dahua-vth-indoor-monitor-to-yeastar-p-series-pbx-system.html)
  (Room No. = extension number, *Sip Register Pwd*, 8–15 characters; the
  tested models are not named)
- Dahua VTO quick start guide v4.5, VTO2311R-WP manual
