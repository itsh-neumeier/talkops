# Browser softphone (WebRTC)

The browser softphone supports audio, video, hold, mute and key tones.
It rings with the selected extension's other phones after you first open
**Softphone**. It stays connected when you navigate within TalkOps.

## Requirements

- **HTTPS**: browsers only give microphone and camera to secure pages. The
  easiest way is the Caddy option (`COMPOSE_PROFILES=caddy`,
  `TALKOPS_DOMAIN=pbx.example.com`, see [installation](installation.md)).
  On the TalkOps machine itself `http://localhost:8080` works too.
- Regular users and operators need an assigned **extension** (admin: extension → user).
  Administrators can select an active extension without an assignment.
- A current Chrome, Edge, Firefox or Safari.

On first use TalkOps creates a separate *Browser* device for the user on the
selected extension. Administrators and regular users do not share SIP
credentials. You can disable or delete the device in the extension settings.

## Select a call identity

Administrators must select an extension in **Call identity**. The list shows
all active extensions in the same tenant. Regular users and operators see
only their assigned active extensions. One assigned extension is selected
automatically. With several assignments, the user can select an extension.

The list shows the internal number, name and outgoing public number.
**Default number** identifies the configured fallback. **No outbound route**
means that public calls have no usable trunk route. Internal calls can still
work. Caller ID hiding and emergency calls follow the existing extension and
dialplan rules.

The identity also determines which incoming calls ring in this browser.
Selection does not change the extension's assigned owner or call settings.
You cannot change identity during a call. The old identity unregisters before
the new identity registers.

## Reachability and audio

Open **Softphone** once to start it. You can then navigate to other TalkOps
pages. A small panel shows its state and lets you answer, decline, mute or
end calls. Return to **Softphone** for all controls. Logout, a page reload or
closing the tab disconnects it. A suspended browser can also stop receiving
calls. Keep the tab open and the browser active when you need incoming calls.

**Audio devices and tests** lets you select a microphone and an output device.
Allow microphone access to see device names. **Test microphone** shows the
input level. The test stops when a call starts or you leave the softphone
view. Microphone changes apply to the next call. **Test sound** plays a short
tone on the selected output. If output selection is unavailable, use the
operating system's audio settings. A removed device falls back to the system
default; check the audio before the next call.

Enable **Audible ringtone** for incoming calls. Click **Enable ringtone /
test sound** if the browser blocks playback. Muting this ringtone does not
set the extension to Do Not Disturb and does not silence its other phones.
**Enable call notifications** requests browser notification permission.
Notifications do not make the softphone reachable when the tab is closed.

TalkOps attempts to reconnect after a transport failure. The panel shows a
connection error while it is disconnected. Use **Reconnect** if automatic
reconnection does not restore registration.

## Contacts, favorites and recent calls

Search the shared phonebook by name, company or number. Mark a contact with
the star to put it first. Select a number to put it in the dial field. The
phone button starts a call directly. Favorites and audio preferences are
stored in this browser separately for each user.

**Recent calls** shows up to 30 calls for the selected extension. Filter by
**Missed incoming** to show unanswered incoming calls. An unanswered outgoing call is
marked **Unanswered**. The callback number is the other party's number,
including internal calls. Use **Refresh** to update contacts and recent calls.

## Network

- Signalling (SIP over WebSocket) goes through TalkOps itself
  (`/api/v1/webrtc/ws`, logged-in users only); FreeSWITCH listens for it on
  `127.0.0.1:5066` only.
- Voice and video flow directly between browser and FreeSWITCH (UDP, RTP
  port range from `.env`). This works in the local network and over VPN
  without further settings. **On the road without VPN** or with blocked UDP,
  the TURN server helps (see below).

## Keyboard

Digits, `*` and `#` – also on the numpad – dial; during a call they are sent
as tones (e.g. for voice menus). **Backspace** deletes, **Enter** calls or
answers, **Esc** hangs up, declines or clears the number.

## Behind a reverse proxy (e.g. Zoraxy)

- Enable WebSocket forwarding, a timeout of at least 3600 s that is
  refreshed on activity, and HTTP/1.1 to the upstream.
- Pass the original host or set `X-Forwarded-Host: <domain>`, and
  `X-Forwarded-For` (the browser's address for the media connection).
- Do not add your own permission policy – TalkOps allows microphone and
  camera itself.

## TURN server (media over one port)

Without TURN, media runs directly over UDP on the RTP ports (16384–16999).
Where that is not possible – on the road without VPN, strict firewalls, guest
Wi-Fi – a TURN server (coturn) relays it over **one** port (3478). TalkOps
ships coturn as an optional service and issues own credentials for every
softphone login, valid for 24 hours.

1. Set in `.env` or the stack variables:
   ```sh
   COMPOSE_PROFILES=turn
   TALKOPS_TURN_SECRET=$(openssl rand -hex 32)   # long random value
   TALKOPS_TURN_PEER_IP=192.168.140.30           # LAN address of this server
   TALKOPS_TURN_URLS=turn:talk.example.com:3478?transport=udp,turn:talk.example.com:3478?transport=tcp
   ```
   `TALKOPS_TURN_URLS` is where browsers reach coturn (domain or IP).
2. Redeploy the stack; the log shows `TURN enabled for softphones`.
3. Reachability: allow port **3478 (UDP and TCP)** to the server – in the
   server's firewall for the LAN, plus a port forwarding in the router for use
   on the road. With Zoraxy this is a **stream proxy** (TCP/UDP 3478 →
   `192.168.140.30:3478`).
4. Optional `TALKOPS_TURN_RELAY_ONLY=true`: media always goes through TURN,
   even when the direct path would work (for testing or blocked UDP).

coturn relays only to `TALKOPS_TURN_PEER_IP`, never into the rest of the
network. The relay ports 49160–49200 are only used internally between coturn
and FreeSWITCH and need no forwarding.

## Hide your number once

The checkbox *Hide my number once* below the keypad applies to the next call
only: TalkOps dials `*31` in front (also before SIP addresses), then the box
is cleared and your number is shown again. To hide it permanently, use the
extension's setting.

## During a call

- **Display**: incoming calls show the caller's name and number; during a
  call you see the other party, the duration and "on hold". The number field
  is cleared after the call; **Call** with an empty field redials the last
  number.
- **Hold**: the other party hears the music on hold (Settings → *Music on
  hold*: all built-in pieces, one piece or own music/announcement).
- **Conference**: during a call choose **Conference**, enter a number,
  **Add**. The call becomes a conference and the new participant is called
  (internal or external, like a normal call from your extension). Add more
  participants the same way. When the initiator hangs up, the conference
  ends for everyone.
- **Key tones** (voicemail, menus) are sent as RTP telephone events
  (RFC 2833).
- **Hide your number** for one call: put `*31` or `#31#` before the number.
  Permanently: extension → *Hide caller ID*.

## Video

Video is switched on per extension (extension → *Video calls*, off by
default). A call can use video only if the caller sends video. TalkOps then
offers video only to the phones that the call allows:

| Call | Video |
| --- | --- |
| Extension to extension | when both extensions have video |
| Extension to ring group | to the members with video |
| Door station | to all called phones (local call) |
| From or to a trunk | only if the trunk has *Video calls via this trunk* and the extension has video |
| Queues, voicemail, menus, conferences, SIP addresses without a trunk | never |

Most providers have no video. Keep *Video calls via this trunk* off for them.
Then a call from outside never asks a video phone to answer with video.
A Wi-Fi handset with the codec profile *HD* or *Robust* gets no video.

The softphone shows the video button only for an extension with video, and
*Answer with video* only when the incoming call has video.


Video to desk phones (e.g. Yealink T58W, VP59) and door stations uses H.264;
Chrome, Edge, Firefox and Safari offer H.264. Between two browsers VP8 works
as well.

## Troubleshooting

| Shown | Cause |
|---|---|
| Hint about a secure connection | page opened via HTTP instead of HTTPS |
| "offline" | TalkOps or FreeSWITCH not reachable; reload the page |
| Rings, but no audio | UDP/RTP between browser and server blocked (firewall, network without VPN) |
| No microphone | check the browser permission for the page |

For detailed troubleshooting set `localStorage['talkops.sipDebug'] = '1'` in the
browser console and reload the page: the softphone then logs all SIP messages.
