# Browser softphone (WebRTC)

Every user with an extension can make calls right in the browser – with
video, hold, mute and key tones. The softphone rings together with the
extension's other phones while the **Softphone** page is open.

## Requirements

- **HTTPS**: browsers only give microphone and camera to secure pages. The
  easiest way is the Caddy option (`COMPOSE_PROFILES=caddy`,
  `TALKOPS_DOMAIN=pbx.example.com`, see [installation](installation.md)).
  On the TalkOps machine itself `http://localhost:8080` works too.
- The user needs an **extension** (admin: extension → user).
- A current Chrome, Edge, Firefox or Safari.

On first use TalkOps creates a device "Browser" (type *Browser*) on the
extension. It shows up like other devices and can be disabled or deleted
there.

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
default). Internal calls carry video only when both extensions have it on;
calls via a trunk only if the trunk's *Video calls via this trunk* is on as
well (only for providers with video). Otherwise calls are audio-only – also
from the video button, which the softphone hides without video. Door stations
always send their picture.


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
