# Door stations

TalkOps connects door stations and indoor monitors as SIP devices: the
station registers like a phone, its ring goes to an extension, ring group
or another destination, and the camera picture (H.264) reaches video phones.
The door opens from a phone, the web interface or Home Assistant.

SIP, ring routing, video and unlock codes are the same for all vendors. The
HTTP integration (door opener, pictures, events) and the device settings
depend on the vendor (ADR 0026).

| Vendor | Devices | Setup |
|---|---|---|
| Dahua | VTO door stations, VTH indoor monitors | [Dahua VTO and VTH](door-station-dahua.md) |

The web interface shows a step-by-step guide for each vendor and device
type: **Settings → Door stations → Setup guide**.

> **Status:** tested with a SIP simulator and a re-created Dahua HTTP API.
> Field reports with real devices are in the vendor page. Feedback with
> model and firmware version is welcome.

## 1. Create an extension for each device

**Settings → Extensions → New extension**, e.g. `8000` "Front door". Inside it,
**Add device**, type **Door station**. The device needs the SIP credentials
shown (user name `8000vto`, password).

The SIP user name is the extension number plus an ending. For a door station
TalkOps uses `vto` if you do not enter an ending. Dahua VTO and VTH devices do
not accept `-` in the user name. You can set the ending yourself, e.g. one
extension for each device:

| Device | Extension | SIP user name |
|---|---|---|
| Outdoor station (VTO) | `8000` | `8000vto` |
| Indoor monitor 1 (VTH) | `8001` | `8001vth` |
| Indoor monitor 2 (VTH) | `8002` | `8002vth` |

Permitted: letters, digits, `.`, `_` and `-`. The ending must not start with a
digit. An admin can change the user name of a device later (**Edit**). After
the change, the device must register again with the new user name.

### SIP password format

| Format | Length | Use |
|---|---|---|
| Standard | 20 letters and digits | all devices (default) |
| Compatible | 15 letters and digits | devices that accept at most 15 characters, e.g. Dahua VTO and VTH |

- TalkOps selects **Compatible (15 characters)** when you add a device of the
  type **Door station**. You can change it in the dialog.
- The format applies when TalkOps generates a password. **Edit** stores the
  format for the next new password and keeps the current password.
- **New password** generates a password at once. An admin can select the
  format in this dialog. The old password stops working immediately; enter
  the new password in the device.
- TalkOps stores SIP passwords encrypted. The audit log records the format,
  never the password.

## 2. Configure the device

Follow the vendor page, e.g. [Dahua VTO and VTH](door-station-dahua.md).
Check the registration: the **Dashboard** shows the SIP user name under
*Registered devices*.

## 3. Door station in TalkOps

**Settings → Door stations → New door station** (admin; the **Door** menu item for opening appears once a door station exists):

| Field | Meaning |
|---|---|
| Manufacturer | selects the HTTP integration and the setup guide (at the moment: Dahua) |
| Extension | the extension of the door station from step 1 – every call from it is a ring |
| Ring goes to | extension, ring group, time condition, Smart Attendant, queue or voicemail |
| Buttons | for multi-tenant stations: dialed number (e.g. `9902`) → its own destination; `9902#0` and `9902` count as the same |
| HTTP access | IP/port and web login of the station; without a host only rings, no door opener/picture/log |
| Locks | `2` if a second lock is connected through the RS-485 module (e.g. DEE1010B) |
| Unlock code door 1 / door 2 | DTMF code that TalkOps detects during a call, see below |
| Save a picture with every ring | stores a snapshot in the event log |
| Event log | TalkOps follows the station's event stream (door open/closed, unlocks, tamper) |
| Webhook | every event as JSON to this URL (Home Assistant) |

**Test connection** shows the reported model. The station password is
stored encrypted.

### Ring target and ring group

To ring more than one indoor monitor or phone, open **Call routing → Ring
groups** and create a ring group, e.g. with `8001` and `8002`. Then set
**Ring goes to** to this ring group.

## Opening the door

| Way | How |
|---|---|
| During the call | type the TalkOps unlock code on the phone (see below) or the station's own unlock code (see the vendor page) |
| Feature code | `*85` (first door station) or `*85<extension>`, e.g. `*858001`; `*86…` for the second lock. An announcement confirms. Works well as a speed-dial key. |
| Web interface | **Door → Open door** (all users, with confirmation) |
| Automation | `POST /hooks/door/<id>/open` with a token, see below |

Every opening is in the event log with who triggered it; openings from the
web interface are also in the audit log.

### Unlock codes in TalkOps (DTMF)

Each lock can have its own unlock code: 2 to 8 keys of `0-9`, `*` and `#`,
e.g. `*1` for door 1 and `*2` for door 2. TalkOps detects the code in these
calls and opens the door through the HTTP access:

- the call after a ring (the phone that answers);
- a direct call from an internal phone to the door station's extension
  (e.g. `8001`);
- external calls only from the numbers in **External numbers for unlock
  codes**: a ring that is forwarded to a mobile phone, or a call from
  outside to a number that goes to the door station.

Internal phones can always use the codes. If the list is empty, no external
call can open the door with a code. Enter one number per line, in any usual
format (`0170 1234567` or `+491701234567`). TalkOps stores the numbers in
E.164.

Keys pressed at the door station itself are not evaluated. A pause of more
than 4 seconds starts a new code. TalkOps evaluates at most 40 keys in each
call.

Requirements and notes:

- The HTTP access of the door station must be configured.
- TalkOps stores the codes encrypted. The web interface does not show a
  stored code. Leave the field empty to keep the code. Use **Remove unlock
  codes** to delete them.
- The two codes must not end with each other (e.g. `#1` and `1`).
- Use a different code than the unlock code stored in the station. If both
  are identical, the door opens two times.
- For a call from outside, TalkOps compares the caller ID. A caller can
  falsify the caller ID. Use a code with at least 4 keys when you allow
  external numbers.

## Live picture and log

**Door** shows all users the stations, the camera picture on request
(refreshed every 2 seconds) and the event log with pictures. Log and
pictures are deleted after the retention period of call recordings
(**Settings → Call recording**, default 90 days).

## Home Assistant

**Receiving events:** create an automation with a *Webhook* trigger in Home
Assistant and enter its URL (e.g.
`http://homeassistant.local:8123/api/webhook/front-door-ring`) as the door
station's webhook. TalkOps sends:

```json
{
  "event": "ring",
  "event_id": "…",
  "door_station": { "id": "…", "name": "Front door" },
  "detail": { "dialed": "9901" },
  "has_snapshot": true,
  "at": "2026-10-06T16:18:03Z"
}
```

`event` is one of `ring`, `open_command`, `opened`, `door_open`,
`door_closed`, `unlock_failed`, `alarm`, `online`, `offline`.

**Opening the door:** create a **Token for automations** at the door
station (shown only once) and in `configuration.yaml`:

```yaml
rest_command:
  open_front_door:
    url: "http://talkops.local:8080/hooks/door/<id>/open"
    method: post
    headers:
      authorization: !secret talkops_door_token   # "Bearer <token>"
    content_type: "application/json"
    payload: '{"door": 1}'
```

## SIP login protection

The SIP login protection bans addresses after failed logins
(**Settings → Sign-in & security → SIP login protection**). It is not
necessary to switch it off for a door station.

- If TalkOps banned the address of a device during the setup, select
  **Unban** for this address.
- If a device fails often during the setup, add only its IP address to
  *Trusted networks* (e.g. `192.168.1.50`), not the full network.
- Remove the exception after a successful setup if you do not need it.

## Troubleshooting

- **`401`, then `403 Forbidden`:** TalkOps rejected the login. The causes
  are a wrong or shortened password, an unknown SIP user name, an
  authentication name that is different from the SIP user name, or a banned
  address. The TalkOps log shows the user name and the address:
  `docker compose logs -f talkops | grep "SIP authentication"`. The vendor
  page has the device-specific checks.
- **Station does not register:** "SIP Server" on the station off?
  *Username* (with *Enable*) and *SIP Server Username* both set to the SIP
  user name (e.g. `8001vto`) and both passwords set? Is the PBX's SIP port
  reachable?
- **The station rejects the user name:** use a user name without `-`, e.g.
  `8001vto` (device **Edit** → SIP user name).
- **Unlock code does nothing:** is the HTTP access configured (**Test
  connection**)? Does the phone send DTMF as RFC 2833? The event log shows
  every opening with the source `dtmf`. For an external call: is the number
  in **External numbers for unlock codes**?
- **No picture on the phone:** the phone must support video (H.264), e.g.
  Yealink T58W/VP59. Audio always works. A queue as the target and a Wi-Fi
  handset with the codec profile *HD* or *Robust* get no video. Calls from
  the door station send video to all called phones; the extension flag
  *Video calls* is not necessary for this local call.
- **Key tone does not open the door:** select only one DTMF method on the
  station (RFC 2833).
- **"not reachable" in TalkOps:** check the HTTP access (IP, port, login)
  with **Test connection**. Some firmware only allows the web login over HTTP
  on the local network.
