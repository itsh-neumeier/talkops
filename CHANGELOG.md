# Changelog

All notable changes to TalkOps. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [1.1.3] – 2026-10-08

### Added

- Softphone: dial with the keyboard and numpad (tones during a call),
  Backspace, Enter to call or answer, Esc to hang up.
- Smart Attendant editor: right-click menu on steps (edit, replace with
  another step, remove with the steps below) and on keys (remove branch).

## [1.1.2] – 2026-10-08

### Fixed

- Browser softphone calls hung up when answered (`INCOMPATIBLE_DESTINATION`):
  browsers hide their address in ICE candidates behind random `*.local` names
  that FreeSWITCH cannot resolve. TalkOps now replaces them with the
  browser's address (from `X-Forwarded-For` of a reverse proxy on the local
  network, or the connection itself).

## [1.1.1] – 2026-10-08

### Fixed

- Browser softphone behind an HTTPS reverse proxy (e.g. Zoraxy): the
  registration got no answer from FreeSWITCH (transport `WSS` in Via); it
  now registers. Softphone sessions are logged.
- Softphone origin check accepts `X-Forwarded-Host`/`Forwarded` from reverse
  proxies that rewrite `Host`.
- The softphone page shows only a hint (no "not found") when the user has no
  extension yet.

## [1.1.0] – 2026-10-13

Upgrading from 1.0 is automatic; voice menus become Smart Attendants.

### Added

- **Audio clips** for every prompt: generate with the computer voice (two
  voices per language: Thorsten/Kerstin, Linda/Joe) and listen before saving,
  record in the browser, or upload WAV/MP3/OGG/M4A (converted in the
  browser). Waveform player with seeking and 1×/1.5×/2× speed.
- Voicemail greeting *None* (straight to the beep) and greetings from clips.
- **Smart Attendant** flow editor replacing voice menus: keypress menu, ring
  phones (continues when nobody answers), play audio, schedule, voicemail for
  several recipients, forward, park in a free slot, go to step, hang up.
- **Queues** like UniFi Talk: own page with General / Schedule / Call
  handling; greeting, music on hold, business hours with after-hours
  destination, maximum waiting callers with overflow, voicemail for several
  recipients when nobody answers.
- **Dashboard**: calls, missed calls, answer rate and average talk time; call
  chart for 1 hour / day / week / month; calls in progress; recent calls;
  system box with services, devices, trunks and last backup.
- Yealink **AX83H and AX86R** Wi-Fi handsets (4 accounts, firmware 180.86+).
- API: `/api/v1/audio/*`, `/api/v1/attendants`, `/api/v1/stats/calls`,
  `/api/v1/telephony/calls`.

### Changed

- **Breaking (REST API):** `/api/v1/ivr-menus` is replaced by
  `/api/v1/attendants` (flow as JSON). Existing menus are converted by the
  migration; their greetings become audio clips.
- The media worker image ships two more Piper voices (de_DE-kerstin-low,
  en_US-joe-medium).

### Fixed

- Yealink common configuration `y000000000108.cfg` (AX handsets) is served.

## [1.0.0] – 2026-10-08

First stable release. Database migrations are forward-only; later 1.x
releases upgrade a 1.0 installation automatically.

### Telephony (phases 0–1)

- FreeSWITCH 1.10 (Debian 13) configured entirely from PostgreSQL via
  `mod_xml_curl`, controlled via the Event Socket.
- Extensions with multiple devices, internal and external calls, emergency
  numbers, dialing rules, call log.
- SIP trunks from 28 versioned provider presets (LEONET, Telekom, Vodafone,
  sipgate, easybell, FRITZ!Box, Twilio, …); several accounts and numbers per
  trunk; trunk and SIP credentials encrypted at rest.

### Phones (phase 2)

- Yealink auto-provisioning for desk phones and DECT, templates per model,
  firmware management, XML phonebook, BLF keys, MWI, resync.
- Feature codes for DND, call forwarding and voicemail.

### Voicemail and speech (phase 3)

- Voicemail in Rust over the outbound Event Socket, PIN, greetings.
- Local text-to-speech with Piper (German and English) for greetings and
  system prompts; e-mail notification with the message attached.

### Call routing (phase 4)

- Ring groups, opening hours with public holidays per German state, voice
  menus (IVR) edited in the web UI, queues (mod_callcenter), call parking.

### Recording and transcription (phase 5)

- Call recording per extension or direction, with optional announcement.
- Local transcription with whisper.cpp (calls and voicemail), full-text
  search, retention periods.

### Door stations (phase 6)

- Dahua VTO door stations: video ring, multi-button routing, door opener
  (web UI, feature codes, token webhook), snapshots, event log, Home
  Assistant webhooks.

### WebRTC and identity (phase 7)

- Browser softphone with video (SIP over WebSocket through TalkOps, SIP.js).
- Two-factor login (TOTP, recovery codes), OIDC single sign-on, LDAP/Active
  Directory logins with group-based roles and hourly sync.

### Operations (phase 8)

- Backups: daily archive of database and data volumes with retention,
  download in the web UI, `talkops backup` / `talkops restore`.
- Prometheus metrics at `/metrics` (opt-in with `TALKOPS_METRICS_TOKEN`).
- SIP login protection: addresses with too many failed SIP logins are banned
  for a while; trusted networks; bans visible and removable in the UI.
- Hardening: security headers and Content Security Policy, FreeSWITCH
  endpoints only for local peers, containers without capabilities,
  `no-new-privileges`, read-only root file systems for server and media worker.
- Getting-started checklist for admins on the dashboard.

### Known limitations

- Not yet verified with real hardware: LEONET live trunk, Yealink phones,
  Dahua door stations (protocols are covered by simulators and SIPp).
- No SIP over TLS (port 5061) yet.
- No country (GeoIP) filters.
- Playwright's Chromium lacks H.264; browser video with H.264-only door
  stations needs a browser with H.264 (Chrome, Edge, Safari).

[1.0.0]: https://github.com/itsh-neumeier/talkops/releases/tag/v1.0.0
