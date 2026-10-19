# Changelog

All notable changes to TalkOps. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [1.8.4] – 2026-10-19

### Added

- Add SIP fax sending from PDF and image files. Require a page preview and a
  separate send action. Add saved header/footer text and role-based access.
- Add dedicated incoming fax destinations, shared inboxes and optional PDF
  email delivery. Support own, shared and global sender numbers.
- Add attachment-only mail-to-fax through a verified IMAPS mailbox. Require an
  exact sender whitelist and an international number as the complete subject.
  Exclude body text, HTML, quotes and inline signatures. Send rejection notices.
- Add fax status reports, cancellation, explicit resends, retention and backup
  support. Do not retry uncertain delivery automatically.
- Add G.711 and negotiated T.38 fax media round-trip checks to E2E.

### Upgrade notes

- Back up before upgrading. Apply migrations 20261106000001 and 20261106000002
  before starting the new binary. Do not downgrade after assigning fax routes.
- Deploy the updated Compose file. All three services need the shared fax volume.
  New image packages provide Python, document conversion tools and fonts.
- Fax stays disabled until configured. Assign permissions, sender numbers and
  dedicated receive destinations. Both directions default to 30-day retention.
- Mail input needs outgoing SMTP and an IMAPS mailbox whose server restricts
  authenticated users to their assigned sender addresses. A From header alone
  does not authenticate a sender. Verify the deployment provider and mail server.
- Users and numbers referenced by fax records cannot be hard-deleted. Remove
  document references and reassign destination ownership first.

## [1.8.3] – 2026-10-17

### Added

- Add multi-vendor provisioning from the complete pinned FusionPBX library.
  Preserve 1,037 original source files. Adapt 385 usable source profiles and
  the explicit GXP1630 variant. Keep existing Yealink model identifiers.
- Add categorized manufacturer settings, native key functions, language,
  time-zone and NTP controls. Protect saved secrets and managed identity fields.
- Add authenticated native file delivery, configuration downloads and QR exports.
- Add same-model phone copies with independent identities and SIP credentials.
  Exclude secrets and runtime values. Reject stale previews and create accounts
  in the same transaction. Defer copied keys until an account is assigned.

### Fixed

- Correct source credential bindings, account selectors, auxiliary file links
  and contact number types in generated configurations. Preserve original files.

### Upgrade notes

- No new migration is added. Migration 20261105000001 from 1.8.2 is required.
- Existing Yealink assignments and credentials remain valid. Provisioning
  changes need a configuration download. Native profiles have no remote
  resync or reboot command without a manufacturer adapter.
- Device and firmware validation is pending. Spectralink and Grandstream Wave
  remain blocked because their source templates are empty. Firmware files are
  not bundled. Some old Cisco phones require a separate TFTP service.

## [1.8.2] – 2026-10-17

### Added

- Add model-based Yealink programmable keys, supported expansion module
  profiles and ordered call-state softkeys. Keep the existing graphical line
  keys. Select contact numbers, local actions and XML application URLs.
- Add the pinned FusionPBX Yealink reference library: 433 unchanged files
  from 70 source folders. Show original source text and provenance. Keep raw
  Smarty separate from the native templates; it is not executed.
- Clone native templates per tenant and model. Save immutable revisions,
  validate before activation and restore earlier revisions. Failed validation
  keeps the active version. Limit evaluation, recursion and output size.
- Preview unsaved phone and account-label changes with generated credentials
  redacted before rendering.
- Set provisioning addresses and SIP hosts per server or phone. Keep explicit
  device URLs in configuration, assets and callbacks. Show bounded download
  diagnostics separately from the SIP NOTIFY result.
- Add authenticated boot and softkey files. Add vendor dispatch and setting
  metadata for future adapters. Unsupported vendors receive no Yealink fallback.

### Fixed

- Keep saved provisioning overrides when an older API client omits the field.
- Keep an explicit phone HTTP address when the web UI uses HTTPS.
- Map T41P programmable IDs 1–10, 13, 17 and 18 without expansion modules.
  Apply mute-key gating, directed pickup codes and explicit default resets.
- Preserve query parameters in XML application URLs. Provisioning base URLs
  still reject query parameters.

### Upgrade notes

- Back up the database before upgrading. Apply migration
  20261105000001_provisioning_profiles before starting the new API.
- Existing credentials, accounts and line-key assignments are retained.
  Template activation alone does not send a resync. Activate and assign a
  template, then let the phone download its configuration.
- A legacy phone can use an explicit HTTP exception on a trusted LAN while
  other phones use HTTPS. Set a reachable initial device or DHCP option 66
  address and credentials once. A server setting cannot repair a TLS failure
  that prevents the first download. No automatic downgrade is added.
- Enable existing MiniJinja loader/fuel features and direct serde_json/url
  use in provisioning. No dependency version upgrade is included.
- The source library adds about 13 MB. Third-party notices and the MPL-1.1
  reference license are included in the repository and server image.

### Known limits

- Real T41P/AX86R firmware, key, expansion module and transport checks have not
  run here. Check them after deployment. The reported T41P HTTPS failure cause
  remains unconfirmed; successful NOTIFY does not prove a configuration download.
- The 70 source folders do not imply 70 active supported models. The existing
  catalog remains the active model set. Additional vendor adapters are future work.
- Earlier backup API, notification and Dahua follow-ups remain separate.

## [1.8.1] – 2026-10-16

### Added

- Show local country flags for inbound callers and outbound destinations in
  recent calls, active calls, the call log and softphone history. Resolve
  shared calling codes with bundled numbering data. Show localized country
  names for screen readers. No external lookup is required.
- Show loading placeholders immediately. Load dashboard call cards separately.
  Keep existing content during refreshes. Show loading, empty and error states
  separately. Respect reduced motion settings.
- Add ten Yealink desk phone models: T41P, T41S, T46G, T46S, T48G, T48S,
  T53, T73W, T85W and T87W. All 21 supported desk models now have graphical
  function key setup, a list view and local device graphics.
- Select speed dial targets from assigned phonebooks or internal extensions.
  Select work, mobile or other numbers. Save the phone and account text before
  resync. Report save, offline and resync failures separately.

### Fixed

- Correct documented key capacities for T33G, T46U and VP59. Keep unchanged
  unsupported overrides visible for explicit review. Reject new or modified
  unsupported assignments. Keep Default and Unused distinct.
- Accept the longer common configuration filenames of newer Yealink models.
- Ignore stale call-list and softphone history responses after navigation,
  filtering or identity changes. Load softphone history independently of SIP
  registration.

### Upgrade notes

- No database migration is added. Phone model layout fields are additive.
  Existing line_keys storage and call routing stay compatible.
- Resync phones to apply saved layouts. Review old unsupported key overrides
  before reassigning them. Older paged models reserve the bottom-right key
  for page control.
- Country flags identify a number range, not a caller's current location.
  Raw national softphone numbers without country metadata have no flag.
- Initial call placeholders stay visible for at least 300 ms, then content
  fades in over 180 ms. Reduced motion skips this interval and animation.
- Web packages libphonenumber-js 1.13.15 and flag-icons 7.6.0 are pinned.
  No unrelated dependency upgrade is included.

### Known limits

- New phone presets and firmware behavior have not been checked on physical
  phones. Touch models use a labeled DSS overview, not a firmware screen copy.
- This release does not change the documented 1.8.0 backup API upgrade steps
  or its separate notification, live-service and Dahua follow-ups.

## [1.8.0] – 2026-10-16

### Added

- Settings → System has Status, Diagnostics, Sessions and Maintenance tabs
  with direct links. Backup and Storage have separate settings sections. Old
  settings hash links still work.
- Dashboard recent calls show resolved phonebook names, the local extension,
  shared-number matches and the own inbound or outbound number.
- Sort and filter the phonebook by name, company, section and number type.
  Search names or formatted numbers. Keep filters in the page address and
  keep contact links working.
- Sort and filter transcription history by time, processing duration, result,
  direction and model. Show phonebook parties. History contains up to the
  newest 1,000 jobs from seven days and refreshes every 30 seconds.
- Send HTML and plain-text mails with an instance name, logo, accent color,
  header and footer. Edit subjects and texts per language with allowed
  placeholders, preview and test send. A fixed TalkOps version line stays in
  every mail.
- Manage shared storage connections in Settings → Storage. Backup jobs select
  a storage target and optional subfolder. Saved connection tests show their
  last result; targets in use cannot be deleted.
- Filter, sort and page the audit log. Download CSV or JSON. Optionally
  export new records to a storage target with SHA-256 checksum files and file
  retention. Database retention keeps unexported records while export is
  enabled.
- Configure operations mail notifications for supported telephony, backup,
  storage, security, service and configuration events. Set recipients, delay
  with recovery mail, digest and rate limit. Review the delivery log.

### Upgrade notes

- Back up the database before upgrading. Four forward migrations run in ID
  order: 20261103000001_mail_templates, 20261103000002_notifications,
  20261104000001_storage_targets and 20261104000002_audit_export.
- **Backup API clients must change.** POST and PUT /api/v1/backup-targets now
  take storage_target_id and folder, rather than connection fields. Draft
  browse, folder and test endpoints move from /api/v1/backup-targets/* to
  /api/v1/storage-targets/*. Existing connection fields stay read-only in job
  responses. Setup restore endpoints stay unchanged.
- Existing backup connections move automatically, with their IDs, names and
  encrypted secrets. Jobs keep their IDs, run history and archives. Remote
  backup jobs still require an archive password. Use a separate subfolder for
  each job.
- Roles with backups.manage also receive storage.manage during migration. New
  audit.manage and notifications.manage permissions default to
  administrators. Audit viewing remains under audit.view. Audit list default
  page size changes from 200 to 50; the response stays an array with an added
  user_id.
- Some notification events are enabled by default after upgrade. With SMTP
  configured, administrator accounts with an email address receive those
  mails. Review Settings → Notifications for recipients and rules.
- Audit exports are not encrypted archives. Use protected storage and
  encrypted transport. Checksum files detect damage; they are not signatures.
- minijinja 2.24 with the fuel feature is now a direct core dependency. Its
  package was already locked. No unrelated dependency version upgrade is
  included.

### Known limits

- Five notification types remain planned and have no producer:
  trunk.outbound_failures, freeswitch.xml_curl_failed,
  freeswitch.profile_down, freeswitch.channels_high and
  services.update_available.
- Delivery and layout in real mail clients, live FreeSWITCH gateway
  transitions, live SMB and live Portainer were not tested here. Use the
  documented automated coverage and verify those integrations in the
  deployment.
- The VTO-to-VTH missing-picture cause and percent-encoded FreeSWITCH CDR
  spool import defect remain separate work. Preserve rejected spool files.

## [1.7.14] – 2026-10-16

### Added

- Filter the call log by date, direction, caller, destination, result and
  recording. Sort by time or duration. Show phonebook names and distinguish
  busy calls from declined calls. Keep filters in the page URL.
- Create roles with individual permissions. Assign roles directly or through
  LDAP and OIDC groups. The role editor prevents users from assigning rights
  that exceed their own rights. Existing users keep their default access.
- Transcribe a recording again with a selected local or cloud model. Show
  time and cost estimates. The transcription jobs page shows progress, worker
  resources and controls for pause, resume, stop and priority. Local jobs use
  checkpoints on resume; cloud jobs start a new request.
- Send encrypted backups to SMB, S3, WebDAV, SFTP, FTP/FTPS or local targets.
  Set schedules and retention for each target. Browse targets and restore
  through the setup assistant or CLI. Use mounted local paths for NFS and USB.
- Capture Compose and environment files automatically. Optionally read the
  selected Portainer stack before each backup, including backups sent to a
  target. Encrypt the saved Portainer token and support certificate pins.
  Exclude the backup password from captured environment files.

### Changed

- Use consistent page, menu and dialog motion. Delay loading skeletons to
  avoid short flashes. Respect the reduced-motion preference.
- Include checksum-pinned rclone v1.75.2 in the server image for backup
  targets. No Cargo or web dependency upgrade is included.

### Fixed

- Apply the new `backups.manage` permission to backup targets and Portainer
  settings. Refresh Portainer data before archives are made in target staging.

### Upgrade notes

- Back up the database before the update. Run the server and media worker at
  the same version. Four migrations run in this order:
  `20261101000001_roles_permissions`,
  `20261101000002_transcription_jobs`,
  `20261102000001_backup_targets`, and
  `20261102000002_portainer_connection`.
- Existing access defaults and SIP settings stay. Grant the new transcription
  permissions explicitly to non-admin roles. Checkpoint resume can change
  wording near the cut point.
- Production Compose mounts the deployment folder read-only. Keep unrelated
  private files outside that folder. Make `.env` readable by UID 10001.
  An explicit deployment file list keeps its previous behavior.
- Remote backup targets require an encrypted archive. Use a separate target
  folder for each TalkOps server so retention applies to the correct archives.

### Known limitations

- Real SMB storage and a live Portainer installation were not available for
  release tests. Protocol services, a Portainer test server and certificate
  pin tests provide automated coverage. Extra files from Git-based Portainer
  stacks and backup failure email notifications are not included.
- The VTO-to-VTH missing-picture cause and the percent-encoded FreeSWITCH CDR
  spool import defect remain separate work. Preserve rejected CDR spool files.

## [1.7.13] – 2026-10-15

### Added

- Select video per device: follow the codec profile, allow H264 and VP8,
  allow H264 only, allow VP8 only, or disable video. The call video policy
  still applies. Add the G.711-only audio profile (PCMA and PCMU, no jitter
  buffer). A device can combine an audio profile with an explicit video mode.
- Show audio and video codecs and RTP packet counters for each call leg at
  hangup in diagnostics captures. Extend the Dahua guides with video settings
  and tests for VTH indoor monitors.
- Show all phonebook entries for a shared number in active calls. Select a
  name from the local extension's phonebooks first, then use alphabetical
  order. A "+N more" card lists the phonebook, name and entry link. A name
  link opens the phonebook section and highlights the entry. Keyboard, touch
  and narrow-screen use are supported.

### Fixed

- Let the caller inherit the answering device's codecs on video calls when
  the caller has no fixed audio profile. This permits H264 for browser calls
  to H264-only devices instead of selecting VP8 on the browser leg.
- Keep all door-call targets ringing until a device answers. Ignore early
  media on door calls and send ringing to the station before the bridge.
  A VTH early-media response no longer selects a winner on its own.

### Upgrade notes

- Migration `20261031000001_device_video_and_g711` adds the device video
  column and the new codec values. Update the server and worker together.
  Back up the database before the update. No dependency upgrade is included.
- Existing device settings keep their defaults: standard audio permits
  video, and other audio profiles default to video off. A call must still
  permit video. Existing SIP names and passwords stay.
- Door calls no longer provide SIP early-media previews. On video calls,
  callers without a fixed audio profile inherit the answering leg's audio
  codec too. For example, a browser can use G.722 instead of Opus.
- Active-call party objects add `contact_id` and `matches`. The `ambiguous`
  field stays. The displayed name for a shared number is a selection; the
  card shows the other possible entries.

### Known limitations

- The VTO-to-VTH missing-picture cause remains open. Configure the main VTO
  on the VTH, compare the device video modes, and inspect the new RTP counters.
  This release does not claim a verified Dahua hardware fix.
- The percent-encoded FreeSWITCH CDR spool import defect remains separate
  work. Preserve files in `cdr-spool/rejected/` for a later recovery.

## [1.7.12] – 2026-10-14

### Changed

- Set device SIP user names independently of the extension number. Names accept
  3 to 64 ASCII letters, digits, dots, underscores and hyphens. The device editor
  has one field for the full name. Numeric names can be used for Dahua VTO and
  VTH devices. Names are unique across tenants without regard to letter case.
- Generate a random six-character SIP user name when no name is supplied.
  Each generated name has at least one letter. Existing device names stay.

### Fixed

- Resolve extension numbers before device SIP user names in the active call
  overview. A numeric device name cannot select a different extension there.
- Use the SIP user name returned by the API in the Yealink provisioning E2E
  check. The check no longer assumes the old extension-based name format.
- Keep database errors when generating a SIP user name. Retry only name
  conflicts, so a database outage does not return a name conflict error.

### Removed

- Remove the "Test calls" card (sip5060.net targets) from *Settings → System*.
  The targets did not work reliably. Dialing SIP addresses and the softphone
  link `/phone?dial=…` remain available.

### Upgrade notes

- No migration or dependency upgrade. Existing SIP names and passwords stay.
  API clients must read `sip_username` from the device response. They must not
  construct a name from the extension number. The `web-` prefix stays reserved
  for browser devices. Browser devices cannot be renamed.
- To use a numeric name on a Dahua device, change the TalkOps device name and
  set the same SIP user name on the device. Test its registration and calls.
  Dahua VTO/VTH hardware checks with the new names remain pending.
- The percent-encoded FreeSWITCH CDR spool import defect remains separate work.
  Preserve files in `cdr-spool/rejected/` for a later recovery.

## [1.7.11] – 2026-10-14

### Fixed

- Limit door station video offers to the station's advertised video codecs.
  Dahua H264 offers no longer add VP8 on the called legs. Rejected SDP streams
  and payloads outside the active media list cannot add codecs.
- Send ASCII caller names on calls from door stations and direct calls to their
  extensions. Convert umlauts and Latin accents. Keep the full caller name in
  call records; ordinary calls retain their existing names. Ring-group prefixes
  on door calls also remain ASCII.

### Upgrade notes

- No migration, dependency upgrade or device password change. Update the server
  to use the revised dialplan on new calls. No FreeSWITCH source patch is used.
- The ASCII name workaround addresses a suspected cause of Dahua VTO calls
  ending after 30 seconds. Its effect on VTO4202F-P-S2 V4.600.0.3 and VTH5422H
  V4.800.0.2 still needs a hardware test. Check the VTO-to-VTH picture and a
  VTH-to-VTO call longer than 30 seconds after updating.

### Known limitations

- FreeSWITCH video direction and rtcp-fb behavior are unchanged. This release
  does not force a sendonly video offer or claim verified Dahua hardware results.
- The percent-encoded FreeSWITCH CDR spool import defect remains separate work.
  Preserve files in `cdr-spool/rejected/` for a later recovery.

## [1.7.10] – 2026-10-14

### Added

- Add an English and German setup guide for Dahua VTO door stations and VTH
  indoor monitors in Settings → Door stations. Links keep the vendor and device
  selection. The guide includes field mappings and SIP login troubleshooting.
- Select a SIP password format per device: standard (20 characters) or
  compatible (15 random letters and digits). New door devices use the compatible
  format in the web UI. API clients without the new field keep the old default.
- Store the manufacturer of a door station and select its HTTP integration.
  Existing stations use Dahua.
- Log the SIP user and address of failed SIP logins and unknown-user lookups.
  Passwords are not logged.

### Fixed

- Validate the password format when a device changes to the browser type.
  Browser devices keep the standard format.

### Upgrade notes

- Migrations `20261030000001_device_sip_password_format` and
  `20261030000002_door_station_vendor` add the new fields. Existing devices get
  the standard format; existing stations get Dahua. Existing passwords stay.
- Editing a format keeps the current password. Select New password to generate
  a password in the stored format, then update the device. A reset without an
  API body continues to use the stored format. Only administrators can change
  the format; browser devices use standard passwords.
- The 15-character workaround and Room No. settings for VTH5422H firmware
  V4.800.0.2 still need a real-device test. A softphone test does not verify them.

### Known limitations

- The percent-encoded FreeSWITCH CDR spool import defect is not changed.
  Preserve files in `cdr-spool/rejected/` for a later recovery.

## [1.7.9] – 2026-10-13

### Added

- Set a SIP user name ending for each device, for example `8000vto`.
  New door devices use a `vto` ending. Administrators can rename devices;
  browser devices keep their managed names.
- Set an encrypted DTMF unlock code for each door lock. TalkOps detects codes
  on the answering phone of a ring call or the caller of a direct door call.
  External calls require a number on the station's allow list.

### Changed

- Offer video on local calls and on trunks that allow video. The caller must
  offer an active video stream. Ring groups select video per member.
  Queues, voicemail, menus and direct SIP addresses use audio only.
  Door station ring calls retain local video; HD and Robust device profiles stay
  audio only. The softphone offers a video answer only when video was offered.

### Fixed

- Accept CDR variable names with a colon, including exported codec and door
  DTMF variables. Trunk records submitted over HTTP are stored again.
- Choose the lowest free device name number after a device deletion.

### Upgrade notes

- Migration `20261029000001_door_unlock_codes` adds encrypted codes and the
  external-number allow list. Update the server before using these controls.
- Restart FreeSWITCH to load the internal audio-only profile and external
  late negotiation. This ends active calls. Enable video on trunks that need it.
- Existing SIP names remain unchanged until renamed. After a rename, update
  the phone's credentials and register it again. For Dahua VTO, SIP Server
  Username/Password must match the device Username/Password.
- External caller ID can be falsified. Use the documented allow-list and code
  guidance. DTMF on real Dahua hardware is not directly verified. Queue/menu
  loopback legs may not receive the unlock marker.

### Known limitations

- Import of percent-encoded FreeSWITCH spool files remains a separate task.
  Records already moved to `cdr-spool/rejected/` are not imported again.
  Keep these files for a later recovery. The direct HTTP CDR fix above does
  not repair this separate spool format issue.

## [1.7.8] – 2026-10-13

### Added

- Select a local or cloud transcription engine separately for live previews,
  recordings and voicemails. Recordings and voicemails can use a second pass.
  Add several OpenAI or compatible connections with encrypted API keys.
- Stream live previews through the OpenAI Realtime API. The worker sends speech
  from each channel and updates provisional text while the caller speaks.
- Show cloud audio minutes by purpose and model, with an estimated cost.

### Fixed

- Build transcript lines from word timestamps and map local Whisper token times
  back through VAD. Short lines from both speakers now appear in time order.
  API servers can fall back to segment timestamps. Existing transcripts stay.
- Keep the selected light or dark theme after a reload. Scrollbars and native
  controls follow the app theme. Settings use the available content width on
  phones and tablets.
- Preserve word timing and cloud usage accounting in the combined worker.
  Keep the new transcription controls and usage table within the shared layout.

### Changed

- Use shared page headers, tabs, dialogs and table layouts across the web UI.
  Hide less important table columns on narrow screens. Add a styled error page.

### Upgrade notes

- Update the server and media worker together. Migration
  `20261028000001_transcription_profiles` copies existing transcription settings
  into provider connections and profiles for recordings and voicemails.
  Local live previews remain the default. Cloud use requires an enabled profile.
- API clients must replace `/api/v1/settings/transcription-api` with the new
  `/api/v1/settings/transcription` provider and profile endpoints.
  `TenantSettings` no longer returns `transcription_quality` or
  `transcription_refine`.
- Cloud profiles send audio to the configured provider. Realtime WebSocket
  connections do not use `HTTPS_PROXY`. Cost figures are estimates; the
  provider's bill is authoritative. Real OpenAI account access was not tested.
- Restarting FreeSWITCH during an image update ends active calls.

## [1.7.7] – 2026-10-13

### Added

- Select Standard, HD or Robust audio profiles per device. HD uses G.722;
  Robust uses iLBC with G.711 fallback. FreeSWITCH transcodes between parties.
  Profiled device legs use a jitter buffer during bridged calls.
- Select a browser softphone identity. Administrators can select any enabled
  extension in their tenant. Other users select from their assigned extensions.
  The softphone stays connected during navigation and adds audio device tests,
  ringtone, optional notifications, contact search, favorites and callback.
- Recover an instance through the first-setup assistant and offline host tool.
  Complete recovery backups can include deployment files and application secrets
  in password-encrypted archives. Existing data-only backups remain supported.
- Show resolved extensions, trunk numbers and phonebook names in active calls.
  Recorded calls can show a local, provisional live transcript with speakers,
  speech times and reduced-motion support.

### Fixed

- Yealink phonebooks use explicit internal numbers such as `**101`. Contacts
  and CSV imports preserve these numbers. Unknown internal targets no longer
  fall back to an external call.
- FreeSWITCH keeps undelivered call records in the shared `cdr-spool` volume.
  TalkOps imports them every minute. Malformed records are kept for inspection.
  Backup and restore include this spool, including pending and rejected records.

### Changed

- Store shared task memory for earlier Claude Code contributions.

### Upgrade notes

- Wi-Fi handsets now default to HD. Resync Yealink AX83H/AX86R phones to enable
  G.722 and iLBC. Update FreeSWITCH together with the server for `mod_ilbc`.
  HD and Robust profiles offer audio codecs only. Omitting `codec_profile`
  from a device update resets it to the automatic profile.
- **Call pickup changes:** `**<number>` now makes an internal call. Directed
  pickup uses `*8*<extension>`. Resync provisioned phones and reload phonebooks.
  Update manually configured BLF, pickup keys and saved pickup speed dials.
- Update all three images and the Compose configuration together. The new
  `cdr-spool` volume is shared by the server and FreeSWITCH. Updates restart
  FreeSWITCH and end active calls.
- Before replacing an older FreeSWITCH container, copy any undelivered records
  from `/usr/local/freeswitch/var/log/freeswitch/xml_cdr`. Import them into the
  new spool after updating. See the installation guide.
- Forward migrations add browser account ownership
  (`20261026000002_browser_account_owners`) and live preview state
  (`20261026000003_live_transcript_previews`). Admin API clients must send
  `extension_id` to `POST /api/v1/me/webrtc`.
- Migration `20261027000001_device_codec_profile` adds device codec profiles.
- Complete recovery backups require a separate backup password and captured
  deployment files or an encrypted Portainer import. Older binaries cannot
  read encrypted format-2 archives. Keep the password outside captured files.
- Live previews use the local `base` model, about 148 MB on first download.
  Preview text and times are provisional. Existing recording policies apply.

## [1.7.6] – 2026-10-12

### Added

- Set speech speed from 0.5× to 2.0× for each voicemail box. Administrators
  can set a speed for each voicemail prompt and language.
- Use inline tags such as `<speed:0.7>Text</speed>` in generated audio.
  The audio editor previews the selected speed. Recorded and uploaded audio
  keep their original speed.
- Add shared development, handoff and release instructions for Claude Code
  and Codex. Store task summaries in the repository.

### Changed

- Voicemail announces new messages as part of the total. Playback announces
  the message position and total. Empty and singular counts use the correct
  forms in English and German.

### Upgrade notes

- Update the server and media worker together. Migration
  `20261026000001_voicebox_speech_speed` adds speed fields with a default of
  1.0. Apply it before the updated services start.
- Generated greetings that are shared with another box or attendant are
  copied when the box needs a different speed. Existing API clients can omit
  the new speed fields.

## [1.7.5] – 2026-10-10

### Changed

- **Phones and extensions:** extensions are assigned on the phone's page
  (*Assign extension*, ✕ to remove) and optionally right when adding a
  phone; the phone list shows the extensions on each phone and has a
  filter per column. The menu item is now *Phones (provisioning)*.
- **Door stations under Settings:** setting them up moved to *Settings →
  Door stations*; *Door* (open, live view, events) only appears in the menu
  once a door station exists.
- **Resync without restart:** *Resync* now only reloads the phone's
  configuration (SIP NOTIFY `check-sync;reboot=false`,
  `sip.notify_reboot_enable = 0`); the new *Restart* button restarts the
  phone as well. Phones still configured by 1.7.3 or older restart one last
  time on the first resync.
- **Several extensions per number:** a number routed to an extension can
  ring further extensions at the same time – ticked in the extension's
  dialog (*Incoming numbers that ring here*) or on the trunk / *Phone
  numbers* page (*Ring more extensions …*). The extension chosen as the
  destination stays the main one: unanswered calls go to its voicemail.

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
