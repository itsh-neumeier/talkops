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
(user name `8001-1`, password).

## 2. Configure the station (VTO web interface)

Menus differ by firmware; names follow Dahua's guides (VTO quick start
guide v4.5, VTO2311R-WP manual).

1. **Network → SIP Server:**
   - **Do NOT enable "SIP Server"** – otherwise the station acts as a PBX
     itself.
   - Server type: *Third Party* / *Asterisk* (depending on firmware).
   - Server address: IP of the TalkOps host, port `5060`.
   - SIP number / user: `8001-1`, registration password: the password from
     step 1, domain: `talkops.local`.
2. **Local Settings → Basic:** the **Call No.** (default `9901`) is what the
   station dials when someone rings. On multi-button stations every button
   has its own number.
3. **Unlock by key tone (optional):** set an unlock code and choose
   **RFC 2833** – **only one** method (RFC 2833 *or* SIP INFO); according to
   Dahua neither works when both are selected.

The station then shows up on the **Dashboard** under *Registered devices* as `8001-1`.

## 3. Door station in TalkOps

**Settings → Door stations → New door station** (admin; the **Door** menu item for opening appears once a door station exists):

| Field | Meaning |
|---|---|
| Extension | the extension from step 1 – every call from it is a ring |
| Ring goes to | extension, ring group, time condition, Smart Attendant, queue or voicemail |
| Buttons | for multi-tenant stations: dialed number (e.g. `9902`) → its own destination; `9902#0` and `9902` count as the same |
| HTTP access | IP/port and web login of the station; without a host only rings, no door opener/picture/log |
| Locks | `2` if a second lock is connected through the RS-485 module (e.g. DEE1010B) |
| Save a picture with every ring | stores a snapshot in the event log |
| Event log | TalkOps follows the station's event stream (door open/closed, unlocks, tamper) |
| Webhook | every event as JSON to this URL (Home Assistant) |

**Test connection** shows the reported model. The station password is
stored encrypted.

## Opening the door

| Way | How |
|---|---|
| During the call | type the station's unlock code on the phone (key tone, step 2.3) |
| Feature code | `*85` (first door station) or `*85<extension>`, e.g. `*858001`; `*86…` for the second lock. An announcement confirms. Works well as a speed-dial key. |
| Web interface | **Door → Open door** (all users, with confirmation) |
| Automation | `POST /hooks/door/<id>/open` with a token, see below |

Every opening is in the event log with who triggered it; openings from the
web interface are also in the audit log.

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

- **Station does not register:** "SIP Server" on the station off? Domain
  `talkops.local`, user `8001-1`? Is the PBX's SIP port reachable?
- **No picture on the phone:** the phone must support video (H.264), e.g.
  Yealink T58W/VP59. Audio always works.
- **Key tone does not open the door:** select only one DTMF method on the
  station (RFC 2833).
- **"not reachable" in TalkOps:** check the HTTP access (IP, port, login)
  with **Test connection**. Some firmware only allows the web login over HTTP
  on the local network.
