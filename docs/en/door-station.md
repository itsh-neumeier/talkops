# Door station (Dahua VTO)

TalkOps integrates Dahua VTO door stations (e.g. VTO2202F, VTO2211G,
VTO3221E, VTO4202F) directly: the station registers like a phone, its ring
goes to an extension, ring group or any other destination, and the camera
picture (H.264) reaches video phones. The door opens from a phone, the web
interface or Home Assistant.

> **Status:** tested with a SIP simulator and a re-created Dahua HTTP API,
> not yet with real hardware. Dahua's HTTP interface is not publicly
> documented; TalkOps uses the requests established open-source integrations
> use. Feedback with model and firmware version is welcome.

## 1. Create an extension for the door

**Settings → Extensions → New extension**, e.g. `8001` "Front door". Inside it,
**Add device**, type **Door station**. The station needs the SIP credentials shown
(user name `8001vto`, password).

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

## 2. Configure the station (VTO web interface)

Menus differ by firmware; names follow Dahua's guides (VTO quick start
guide v4.5, VTO2311R-WP manual).

1. **Network → SIP Server:**

   | VTO field | Value |
   |---|---|
   | SIP Server – Enable | **off** (otherwise the station acts as a PBX itself) |
   | Server Type | *ThirdParty* (some firmware: *Asterisk*) |
   | IP Addr. | IP address or host name of the TalkOps host |
   | Port | `5060` |
   | Username + **Enable** | set **Enable**, then the SIP user name from step 1, e.g. `8001vto`. Without *Enable* the station uses its own VTO number. |
   | Password | the SIP password from step 1 |
   | SIP Domain | `talkops.local` (the Dahua default `VDP` also works) |
   | SIP Server Username | **the same** SIP user name as in *Username* – not `admin` |
   | SIP Server Password | **the same** SIP password |

   TalkOps accepts a registration only if the authentication user name is
   the same as the SIP user name. Thus *Username* and *SIP Server Username*
   must be identical.
2. **Local Settings → Basic:** the **Call No.** (default `9901`) is what the
   station dials when someone rings. On multi-button stations every button
   has its own number.
3. **Unlock by key tone in the station (optional):** set an unlock code and
   choose **RFC 2833** – **only one** method (RFC 2833 *or* SIP INFO);
   according to Dahua neither works when both are selected. As an
   alternative, TalkOps can detect its own unlock codes (see section 3).

The station then shows up on the **Dashboard** under *Registered devices* as `8001vto`.

## 3. Door station in TalkOps

**Settings → Door stations → New door station** (admin; the **Door** menu item for opening appears once a door station exists):

| Field | Meaning |
|---|---|
| Extension | the extension from step 1 – every call from it is a ring |
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

## Opening the door

| Way | How |
|---|---|
| During the call | type the TalkOps unlock code on the phone (see below) or the station's own unlock code (step 2.3) |
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

## Troubleshooting

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
  Yealink T58W/VP59. Audio always works.
- **Key tone does not open the door:** select only one DTMF method on the
  station (RFC 2833).
- **"not reachable" in TalkOps:** check the HTTP access (IP, port, login)
  with **Test connection**. Some firmware only allows the web login over HTTP
  on the local network.
