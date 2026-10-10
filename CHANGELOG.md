# Changelog

All notable changes to TalkOps. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- **Resync without restart:** *Resync* now only reloads the phone's
  configuration (SIP NOTIFY `check-sync;reboot=false`,
  `sip.notify_reboot_enable = 0`); the new *Restart* button restarts the
  phone as well. Phones still configured by 1.7.4 or older restart one last
  time on the first resync.

## [1.7.4] – 2026-10-10

The tags 1.7.2 and 1.7.3 were set without raising the version in the code,
so those images showed 1.7.1; their changes are listed here as well.

### Added

- **Comfort settings for phones** under *Phones → Comfort settings* and on
  each phone's page: key tone, charging tone, vibration, answer by lifting
  from / hang up in the charger, call waiting, display backlight and
  brightness, 12/24 h clock, missed-call and voicemail notices, noise filter
  and acoustic shield – for the AX83H/AX86R so far. Unset values leave the
  phone's own setting alone; a phone's value wins over the one for all
  phones. The catalog (`settings:` in `presets/phones/yealink.yaml`) lists
  each Yealink parameter with its allowed values.
- **Phone book import (CSV)** under *Phone book → Import*: preview before
  importing, row errors with line numbers, sections created from a
  `Section`/`Bereich` column, optional update of existing contacts. Reads the
  TalkOps sample file (download in the dialog) as well as Outlook and Google
  Contacts exports, UTF-8 or Windows encoding.

- **AX83H/AX86R:** 16 line keys (line and BLF with pickup, keys 1–4 on the
  idle screen), custom ringtone and wallpaper – as documented by Yealink for
  firmware 180.87.0.15. Speed dial keys are refused for these handsets, which
  do not have them.

- **Yealink dial now:** phones dial emergency numbers, fixed feature codes
  and internal numbers after one second instead of waiting for the
  inter-digit timeout. Numbers that start a longer dialable number are left
  out.

### Changed

- **Internal numbers start at 100.** `*1`–`*99` are reserved for system
  codes and `*<number>` reaches internal numbers from `*100`. New extensions,
  groups, time conditions, menus and queues need a number from 100 (the web
  UI says so at every number field); existing 2-digit numbers are kept.
- Phone numbers in contacts may be written as `+49 (0)89 …` or with dots.

### Fixed

- Phone configuration failed (HTTP 500) as soon as a ring group, time
  condition, menu or queue had no number (dial-now rules).
- **Calls from a trunk reach every phone.** FreeSWITCH offered phones only the
  trunk's codec (e.g. just G722); a phone without it – seen with a Yealink
  AX86R – answered `488 Not Acceptable Here` and the call went straight to
  voicemail (the browser softphone still rang). Phones are now offered
  G722, PCMA, PCMU and Opus; the trunk leg keeps its codec.
- Softphone: no more "Invalid session state Establishing" when a call is
  answered twice (answer button and Enter, double click) or put on hold
  before it is set up.

## [1.7.1] – 2026-10-10

### Added

- **Wi-Fi handset** as device type (Yealink AX83H/AX86R) next to desk phone
  and DECT handset.
- **Label and display name per account on provisioned phones:** what the
  line key and idle screen show and which caller name the phone sends – on
  the phone's page (*Accounts on this phone*) or in the device dialog.
- **Ringtones and wallpapers** for Yealink phones (*Settings → Phones*):
  upload any audio file or picture – the browser converts it (WAV 8 kHz /
  JPEG up to 1280 × 800) – and choose it per phone. Only offered on models
  with documented support; ringtones larger than a model takes (100 KB on
  T42U/T43U/T53W) cannot be chosen there.
- **Phone book sections:** contacts are global (every phone) or belong to a
  section such as *Family*; each phone shows up to three assigned sections
  as extra phone books.
- **Internal numbers 100–9999 freely:** 11x numbers are allowed now; only
  115 and numbers that start or are part of an emergency number (110, 112,
  1120 …) stay blocked. **`*<number>`** (e.g. `*610`) calls an internal number
  just like `610`; feature codes keep working.

### Fixed

- The detail page of an extension (and other pages that look up data while
  typing) failed with "500 Internal Error" since 1.7.0.
- The version reported by 1.7.0 images was still 1.6.0.

## [1.7.0] – 2026-10-10

### Added

- **Voicemail control editable** (*Settings → Voicemail*): every menu key,
  every voicemail prompt per language and the voice; the menu prompts name
  the chosen keys.
- **Caller announcement before each message:** name from the phone book or
  extension list (else the provider's name), the number digit by digit in
  groups, and date and time – each switchable.
- **Softphone: hide the own number once** – a checkbox for the next call
  only (dials `*31` in front, also before SIP addresses), then visible again.
- **Main menu only for everyday use:** Extensions, Users and Call routing
  moved to *Settings → Setup*, My phones next to My account. The sidebar
  keeps Dashboard, Softphone, Calls, Voicemail, Door, Phone book, Search and
  Settings.
- **Skeleton loading:** lists, detail pages, dashboard and settings show
  placeholders while their data loads instead of an empty page.

### Fixed

- Softphone: the idle area ("Ready for calls") follows the light/dark theme.

## [1.6.0] – 2026-10-09

### Added

- **SIP addresses:** extensions dial `name@domain` (softphone or phone);
  through a trunk account of the same domain, otherwise directly over the
  internet (*Settings → Telephony*, on by default). Trunk accounts without a
  phone number (e.g. sip2sip/SIP Thor) get their own destination for
  inbound calls.
- **Test calls** under *Settings → System*: echo, key test, time and ring
  test by sip5060.net, to check audio and NAT without a provider.

## [1.5.0] – 2026-10-08

### Added

- **Transcription in two passes:** a quick first transcript right after the
  call (shown at once and sent with the voicemail e-mail), then optionally a
  more accurate second pass (e.g. large-v3 or an AI API) that replaces it.
  Until then the transcript is marked *preliminary* and refreshes by itself;
  each transcript shows the model that made it.
- **AI API for transcription:** any OpenAI-compatible
  `/audio/transcriptions` endpoint – OpenAI (`whisper-1`,
  `gpt-4o-transcribe`), Groq, Mistral (Voxtral) or an own server such as
  Speaches – for the first or second pass, with connection test. The key is
  stored encrypted; models without timestamps get each part of the
  conversation on its own. **The media worker now needs
  `TALKOPS_SECRET_KEY`** (set in `docker-compose.yml`; Portainer stacks
  using the shipped file pick it up from the stack variables).
- **Settings → System → Sessions with the provider:** end all calls (a BYE
  for every leg) and/or sign all trunks off and on again, for providers that
  reject calls with "Too many simultaneous sessions".
- **Session timers on trunks** (RFC 4028, 600 s): a call the provider still
  counts after TalkOps vanished without a BYE ends at the provider within
  10 minutes. Takes effect after the trunk profile restarts (restart
  FreeSWITCH once).
- **Copy log:** the debug log under Settings → System can be copied to the
  clipboard (the lines shown, respecting the filter), also when the UI is
  opened over plain HTTP.

### Changed

- **Leaner main menu:** setup pages – Trunks, Phone numbers, Phones
  (provisioning) and Audit log – moved under *Settings → Setup*. The main
  menu keeps the everyday pages.
- **Video only where switched on:** extensions and trunks get a *Video
  calls* switch (off by default). Internal calls carry video only between
  two extensions that both have it on; calls to a provider only if the trunk
  and the caller allow it. Everything else is audio-only, even from the
  video button (which the softphone hides without video). Door stations
  always send their video. **Switch video on for the extensions that use
  it after updating.**

## [1.4.0] – 2026-10-08

### Added

- **Settings in sections** (Telephony, Call handling, E-mail, Sign-in &
  security, System, My account) with a side menu like UniFi Talk.
- **Settings → System:** status and uptime, **debug logging** of FreeSWITCH
  for 1–60 minutes with optional **SIP trace**, live view with filter and
  download, active channels, and **restart** of FreeSWITCH or all services
  from the browser.
- Transcription accuracy (*Fast*, *Accurate* = large-v3 compressed, *Best* =
  large-v3, *German-optimized* = primeLine large-v3-turbo German) and a list
  of names and terms; audio is normalized and VAD keeps word edges.
  `talkops-media-worker transcribe` compares levels on a file.
- **Call blocking** (Call routing → Call blocking): own list of numbers and
  prefixes, anonymous callers, and the PhoneBlock community spam list
  (phoneblock.net, own API key, cached, fails open).

## [1.3.0] – 2026-10-08

### Added

- **Music on hold:** holding a call now plays music to the other party
  (FreeSWITCH's built-in pieces ship with the image). Settings → *Music on
  hold*: all pieces shuffled, one piece (with preview) or an own clip
  (upload, recording or computer voice); also the default for queues.
- **Softphone conference:** add participants to the current call
  (internal or external); the call becomes a conference that ends when the
  initiator hangs up.
- Softphone shows the caller's name and number, the other party, call
  duration and hold state; the number field is cleared after a call and an
  empty field redials.
- `*31` / `#31#` before a number hides the caller ID for that call.
- Users set how long their phone rings before voicemail answers
  (voicemail settings).
- Transcription: default model `large-v3-turbo-q5_0` (much more accurate
  than `base`) with Silero voice activity detection, which skips silence and
  hold music; more models selectable (`medium`, `large-v3-turbo` variants).
  If you set `TALKOPS_WHISPER_MODEL=base` yourself, change or remove it.
- `localStorage['talkops.sipDebug'] = '1'` logs the softphone's SIP traffic.

### Fixed

- Hold only muted the softphone: the held party heard silence.
- Key tones from the softphone did not reach voicemail and menus; they are
  now sent as RTP telephone events (RFC 2833).
- Hidden caller ID could still show the number on trunks that carry it in
  the From header; now From is anonymous with P-Asserted-Identity and
  `Privacy: id` (RFC 3325).

### Upgrade notes

- Pull and redeploy **all** images (FreeSWITCH carries the music and the
  conference profile, the media worker the new models). The first
  transcription downloads the new model (~550 MB).

## [1.2.0] – 2026-10-08

### Added

- **TURN relay for the browser softphone:** optional coturn service
  (`COMPOSE_PROFILES=turn`) so softphone media runs over one port (3478)
  instead of the RTP range – from outside without VPN, through strict
  firewalls or a reverse proxy's stream forwarding. TalkOps issues
  short-lived credentials per login (`TALKOPS_TURN_URLS`,
  `TALKOPS_TURN_SECRET`, optional `TALKOPS_TURN_RELAY_ONLY`); coturn only
  relays to FreeSWITCH (`TALKOPS_TURN_PEER_IP`).

### Changed

- The softphone no longer asks a public STUN server (Google) for its address.

## [1.1.4] – 2026-10-08

### Fixed

- Browser softphone calls still hung up when answered: FreeSWITCH only
  accepts public ICE candidates by default (`wan.auto`), so browsers in the
  local network or over VPN were rejected. The internal profile now also
  accepts local and private (RFC 1918) addresses. FreeSWITCH reads this when
  the profile starts: restart the FreeSWITCH container after updating.

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
