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
  without further settings. **On the road without VPN** you need a public IP
  with open RTP ports or a TURN server – not built in yet.

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

## Video

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
