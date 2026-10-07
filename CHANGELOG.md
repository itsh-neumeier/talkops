# Changelog

All notable changes to TalkOps. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [1.0.0] – unreleased

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
