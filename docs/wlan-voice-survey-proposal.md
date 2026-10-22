# Wi-Fi voice survey and roaming analysis: proposal for user review

Status: Accepted by the user on 2026-10-22 ("Freigabe"). Implemented in the same PR.
Date: 2026-10-22 (Europe/Berlin).
Base: `origin/main`, `b1fa28350dc1830e222a27ee770641edb1935ccc` (TalkOps 1.8.5).
ADR draft: [ADR 0041](adr/0041-wifi-voice-survey-and-unifi-correlation.md).

## Implementation notes

The user approved the proposal without changes to the open points. The
implementation uses the recommended answers: `*39` for self-dial, read
access to the classic `stat/sta` endpoint, one tone each second with an
announcement every 30 s, a recording for each survey, 30 days retention
and no survey permission for the operator role.

Differences from the draft below:

- Delivery: one PR, with one commit for each part. The session can push
  only one branch, so a split into three PRs was not possible.
- The survey owns its recording file (`recordings/wifi-surveys/<tenant>/<id>.wav`).
  It is not a row of the `recordings` table. A survey row has
  `recording_file` instead of `recording_id`.
- The test signal plays with `playback` in the socket session.
  `endless_playback` does not read the DTMF queue, so FreeSWITCH fires no
  `DTMF` events during it (source check of `mod_dptools` and
  `switch_ivr_play_say.c`, v1.10.12).
- No confirmation tone after a DTMF marker. The handset plays its own key
  tone, and a second audio stream would change the measured signal.
- A loss block is more than `loss_block_pct` loss in the ±3 s window, not
  in one second. With 20 ms packets, a 50 ms gap is already 5 % of one
  second, so a per-second rule would rate every audible gap as bad.
- Roam events from `stat/event` and syslog/SIEM are not read. Roams come
  from AP changes in the samples.
- Handset provisioning (VAD, DSCP) is not added. The Yealink AX8x
  parameter names are not confirmed by official documentation in this
  repository. The guide gives manual steps. This stays in stage 4.
- Migration IDs: `20261107000001_unifi_connections.sql`,
  `20261107000002_wifi_surveys.sql`.
- New prompts `wifi_survey_running` and `wifi_survey_not_armed` (rendered
  by the media worker at start like the other system prompts).

## Goal and review boundary

An administrator walks through the site with a Wi-Fi handset
(`DeviceKind::Wifi`, for example Yealink AX83H/AX86R). TalkOps calls the
handset and keeps the call open with a test signal. During the call,
TalkOps measures the voice stream, records the call and reads the Wi-Fi
state of the handset from the UniFi Network application. After the call,
a job analyses each roaming event and gives a rating and recommendations.

The user asked for a proposal before implementation. This revision contains
only this proposal, an ADR draft and a task summary. Code, migrations and UI
start after the user approves the proposal.

## Verified baseline

These facts come from the current code (`b1fa283`):

- `DeviceKind::Wifi` exists (`crates/talkops-core/src/extensions.rs`). A device
  belongs to one extension. A device links to a phone through `phone_id` and
  `account_index`. The MAC address is in `phones.mac` (12 lowercase hex
  digits, unique). `phones::find_by_mac` and `normalize_mac` exist.
- Recording (ADR 0012) starts in the dialplan with `record_session` and
  `RECORD_STEREO=true`. The `recordings` table links a file to a call UUID.
  `GET /api/v1/recordings/{id}/audio` sends the full file. It has no HTTP
  `Range` support. `AudioPlayer.svelte` loads the full file and seeks in the
  browser.
- `talkops-esl` has `api`, `bgapi`, event subscription and an outbound socket
  server (`OutboundSession`). Fax and voicemail use the outbound socket.
  `door_dtmf.rs` already reads `DTMF` events. No test call or echo test exists.
- The CDR parser reads only `talkops_*` variables and call times. The `cdr`
  table has no quality fields. `diagnostics.rs` reads RTP packet counters for
  a log line only.
- The job queue (ADR 0005) uses string job kinds. A new kind needs a constant,
  a worker lane and a match arm. Piper TTS runs in the media worker.
- `talkops_core::crypto::SecretBox` encrypts secrets (ADR 0008). The Portainer
  connection (ADR 0032) is the model for an encrypted API key that the API
  never returns.
- Permissions come from the `permissions!` list in `roles.rs` (ADR 0029).
  `audit::record` writes audit entries.
- `retention.rs` runs an hourly cleanup loop.
- Feature codes `*1` to `*99` are reserved for system codes. `*39` is free.
- `reqwest` with rustls is already a dependency. The web UI has no chart
  library. `CallChart.svelte` draws SVG by hand.
- Yealink provisioning has no VAD, DSCP or Wi-Fi roaming parameters.
  Phone settings come from the catalog `settings:` list in
  `presets/phones/yealink.yaml`.
- The latest ADR is 0040. The latest migration is `20261106000003`.
  Open PR #58 changes only handbook files. It has no ADR or migration.

## Prototype results

The prototype for this proposal was a source analysis of FreeSWITCH
v1.10.12 (the pinned version) and desk research on the UniFi API.
No live UniFi controller and no Wi-Fi handset were available in this
environment. The hardware prototype is step 0 of the implementation.

### FreeSWITCH: what is available during a call

| Data | Source in FreeSWITCH 1.10.12 | Resolution | Direction |
| --- | --- | --- | --- |
| Packet counters, flaws, MOS, jitter variance min/max | API `uuid_set_media_stats <uuid>` or JSON API `mediaStats` (mod_commands). The call refreshes the `rtp_audio_in_*` and `rtp_audio_out_*` channel variables. | Each poll; values are cumulative. TalkOps calculates the difference to the last poll. | Handset → TalkOps (uplink) |
| RTCP receiver reports from the handset: fraction lost, cumulative lost, interarrival jitter | Event `RECV_RTCP_MESSAGE` (`switch_core_media.c`, headers `SourceN-Fraction`, `SourceN-Lost`, `SourceN-Jitter`) | Interval of the handset. TalkOps can set `rtcp_audio_interval_msec=1000` for its own reports. | TalkOps → handset (downlink) |
| Error periods (start, stop, flaws, consecutive flaws, duration) | XML CDR `<call-stats><audio><error-log><error-period>` (`switch_ivr.c`) | Microsecond timestamps. Only at hangup. | Uplink |
| DTMF digits | Event `DTMF` (already used by `door_dtmf.rs`) | Event time | Handset → TalkOps |

Result: a 1 s poll of `mediaStats` gives the expected and received packet
count for each second. At 20 ms packet time, a second has 50 packets.
Missing packets give the gap length in milliseconds. The CDR error log
gives exact start and stop times of loss periods. Together, these values
give the gap at each roam to approximately 20 ms.

Limits:

- FreeSWITCH gives the jitter variance only as a minimum and maximum since
  the call start. It does not give a jitter value for each second.
  Per-second uplink jitter needs a packet capture.
- MOS in FreeSWITCH is an estimate from the flaw count (`do_mos` in
  `switch_rtp.c`). It is not an ITU-T P.862 or E-model value. The UI must
  label it as "estimated MOS".
- Downlink values depend on the RTCP interval of the handset. If the handset
  sends RTCP only every 5 s, downlink resolution is 5 s.
- When the handset uses VAD or silence suppression, missing packets are not
  loss. The test therefore requires VAD off (see "Handset preparation").

Decision proposal: stage 1 uses only FreeSWITCH data. A packet capture is
an optional later extension. It needs `CAP_NET_RAW` and a new container
permission. Add it only if the hardware prototype shows that per-second
uplink jitter is necessary.

### UniFi Network API: what is available

| Function | Integration API (official, API key) | Classic API (`/proxy/network/api/s/{site}/...`) | Notes |
| --- | --- | --- | --- |
| Authentication | `X-API-KEY` header, key from *Settings → Control Plane → Integrations* | The same API key works on current UniFi OS for read requests (to verify) | No user password storage |
| Sites | `GET /proxy/network/integration/v1/sites` | `GET /api/self/sites` | |
| Find client by MAC | `GET .../v1/sites/{siteId}/clients` with filter, then `/clients/{clientId}` | `GET .../stat/sta/{mac}` | |
| Connected AP | `uplinkDeviceId` in the client object (to verify) | `ap_mac`, `bssid` | |
| Band, channel, RSSI, data rate | Not in the published client schema (to verify) | `radio`, `channel`, `rssi`, `signal`, `tx_rate`, `rx_rate`, `essid` | Main source for radio values |
| AP names | `GET .../v1/sites/{siteId}/devices` | `GET .../stat/device` | |
| Roam events | Not published | `stat/event` (`EVT_WU_Roam`, `EVT_WU_RoamRadio`) or v2 system log | Format changes between versions |
| Syslog / SIEM | — | CEF syslog export in newer Network versions | Needs a UDP/TCP listener |

Desk research found the Integration API reference for Network 10.3.58 on
`developer.ui.com`. That reference lists client list and detail endpoints,
but it does not publish radio fields. Community references confirm the
classic `stat/sta` fields. The minimum Network version for API keys, and
for the use of API keys on classic endpoints, is not confirmed. The
prototype must record these facts for each tested version.

Decision proposal:

1. Read sites, devices (AP names) and the client ID through the Integration
   API where possible.
2. Read radio values for the one survey MAC through classic `stat/sta/{mac}`
   (read only), once each second during the survey.
3. Derive roams from a change of `ap_mac`/`bssid`. A change of `radio` or
   `channel` at the same AP is a band change, not a roam.
4. Read `stat/event` after the survey as an optional source of more exact
   roam times. If the endpoint is missing, use only the derived roams.
5. Do not use syslog/SIEM in this phase. It needs a listener and network
   configuration. Keep it as a later option.
6. A connection test reports the detected Network version and which
   endpoints answered. TalkOps stores this capability set and shows it in
   the UI. The user documentation contains a version table that the
   prototype and later user reports fill.

## Proposed user behavior

### Start a survey

1. A user with `wifi_survey.run` opens **Extensions → extension → Devices**.
   A Wi-Fi device row shows **Start Wi-Fi survey**. A new page
   **Settings → Wi-Fi survey** lists all surveys (see "Place in the admin
   area").
2. The start dialog shows the device, the phone MAC and the UniFi state
   ("client found on AP *Hall*", "no UniFi connection" or "client not found").
   The survey also works without UniFi. Then the result has no radio data.
3. The dialog shows the handset preparation checklist (VAD off, charged
   battery, roaming on) and an optional label, for example "before AP move".
4. The user selects a start mode:
   - **TalkOps calls the handset** (default). TalkOps originates a call to
     the selected device only, not to the full extension.
   - **Self-dial**. The UI arms the survey for 10 minutes. The tester dials
     `*39` from that device. An unarmed `*39` call plays "no survey prepared"
     and ends. This prevents data collection without a started survey.
5. The handset rings with the caller name "TalkOps Wi-Fi test".
   After answer, the test runs until the tester hangs up, presses `#`, the
   user selects **Stop** in the UI, or the maximum time (default 60 min) ends.

### During the survey

- Test signal: one short tone (1 kHz, 100 ms) each second. Every 30 s, Piper
  says "Measurement running" in the tenant language. The worker renders this
  loop once as an audio file. FreeSWITCH plays it with `endless_playback`.
  The tester hears a missing tone as a drop.
- DTMF markers with time stamp:
  - `1` = disturbance heard
  - `2` = new location
  - `3` = note (free text can be added later in the UI)
  - `#` = end test
  A short confirmation tone answers each accepted digit.
- The UI shows the live dashboard (see "Live dashboard during the walk").
- Recording: TalkOps records the call in stereo with the existing
  `record_session` function (ADR 0012). The recording does not depend on
  the recording settings of the extension. The survey always records.
  The recording is not transcribed.

### Place in the admin area

The survey is a normal part of the admin area. It uses the existing
settings navigation (`SettingsNav.svelte`) and the shared components
(`PageHeader`, `Tabs`, `Modal`, `.scroll-x`) from `docs/ui-guidelines.md`.

| Place | Content | Permission |
| --- | --- | --- |
| **Settings → Wi-Fi survey** (new setup page `/wifi-surveys`, next to *Phones*) | Tab **Surveys**: list, start, live dashboard, results, comparison | `wifi_survey.run` |
| same page, tab **Settings** | Thresholds, retention, maximum duration, parallel limit | `wifi_survey.manage` |
| same page, tab **UniFi** | Connection, API key, TLS fingerprint, connection test, detected version and endpoints | `unifi.manage` |
| **Extensions → extension → Devices** | Button **Start Wi-Fi survey** on each Wi-Fi device row | `wifi_survey.run` |
| **Phones → phone** | Link to the surveys of this phone and its current UniFi state | `wifi_survey.run` |

A user without these permissions does not see the page or the buttons.

### Live dashboard during the walk

The administrator carries the handset and normally a smartphone or tablet
with the TalkOps web UI. The live dashboard shows all live values on one
screen, without scrolling on a phone in portrait orientation.

Layout (mobile first, one column on a phone, a grid on a tablet or desktop):

1. **Status bar**: state (calling / running), elapsed time, a large
   traffic-light color for the last 5 s (green / yellow / red), **Stop**.
2. **Value tiles** (large numbers, color by threshold):
   - Access point: name, band and channel (for example "Hall · 5 GHz · 36")
   - RSSI in dBm with a bar, and the trend of the last 10 s
   - Data rate TX/RX in Mbit/s
   - Uplink: packets in the last second (for example "50/50"), loss in % for
     the last 10 s, longest gap in the last 10 s in ms
   - Downlink: loss and jitter from the latest RTCP report, with its age
   - Estimated MOS (FreeSWITCH)
   - Roams: count, and the last roam with "from AP → to AP", gap and a
     preliminary rating
   - Markers: count and the latest marker
3. **Rolling chart** of the last 2 minutes: RSSI line, loss bars for each
   second, vertical lines for roams and markers. A switch shows the full
   test.
4. **Marker buttons** in the UI as an alternative to DTMF: *Disturbance*,
   *New location*, *Note* (with a short text field). A UI marker has the
   same effect as the DTMF digit.
5. **Event list** (newest first): roams, gaps above the audible threshold,
   markers, UniFi poll errors.

Behavior:

- The page polls the compact endpoint `GET /api/v1/wifi-surveys/{id}/live`
  once each second. The response contains only the current values and the
  new samples since the last request (less than 2 KB). This uses the same
  polling method as the live transcript and diagnostics capture views. No new WebSocket or SSE channel
  is necessary.
- The API calculates the preliminary roam rating with the same rule
  functions as the analysis job. The final rating after the call can differ
  a little, because it also uses the CDR error periods.
- The page asks for a screen wake lock (Screen Wake Lock API) so that the
  phone screen stays on. If the browser does not support it, the page shows
  a hint.
- When the connection to TalkOps is lost (for example in a Wi-Fi dead zone),
  the page shows "no connection" with the time of the last update. The
  measurement continues on the server. The page loads the missing values
  when the connection comes back.
- Colors always have a text label or an icon too, for accessibility.
- An optional **audio warning** in the browser: a short sound when the
  last-second rating is red. Off by default.

### Result page

- **Summary**: duration, number of roams, rating counts (clean / audible /
  bad), worst gap, total loss, estimated MOS, detected patterns, and
  recommendations.
- **Timeline** (SVG, one shared time axis):
  - RSSI line, colored background bands for each AP section
  - uplink loss for each second, downlink loss from RTCP, jitter where known
  - vertical lines for roams (color = rating) and markers (icon = digit)
  - click on a point → play the recording from 3 s before that point
- **Roam table**: time, from AP → to AP, band/channel before and after,
  RSSI before and after, gap in ms, loss and jitter ±3 s, nearby markers,
  rating, flags. Each row has **Play** (starts 3 s before the roam).
- **Comparison**: the user selects two surveys of the same device or site.
  The page shows both summaries side by side and the difference for each
  metric and each AP pair. Roams are matched by AP pair, not by time.
- **Export**: JSON and CSV of the timeline and the roam table.

### Rating of each roam

Default thresholds (editable in **Settings → Wi-Fi survey → Settings**):

| Rating | Rule |
| --- | --- |
| Clean | Largest uplink gap < 50 ms and no loss block |
| Audible | Largest gap 50–150 ms |
| Bad | Largest gap > 150 ms, or a loss block: more than 5 % loss in any 1 s inside the ±3 s window |

The gap is the longest run of missing uplink packets (multiplied by the
packet time) inside the roam window. The roam window starts at the last
sample on the old AP and ends at the first sample on the new AP, plus 3 s
on each side. If a `stat/event` roam time is available, TalkOps uses it
instead.

### Pattern detection

| Pattern | Proposed rule (thresholds editable) |
| --- | --- |
| Sticky client | RSSI below -75 dBm for at least 10 s on one AP, and later a roam or a loss rise |
| Ping-pong roaming | At least 3 roams between the same two APs within 60 s |
| Drop without roam | Gap above the "audible" threshold with no AP change within ±3 s |
| Roam without drop | Roam with a "clean" rating (positive result) |
| Weak coverage | RSSI below -80 dBm for at least 5 s, no matter which AP |

Limit: UniFi does not report the RSSI of other APs for a client. "Sticky
client" therefore shows that a better AP was probably available. It cannot
prove it. The UI states this limit.

### Recommendations (read only in this phase)

The analysis creates rule-based recommendations as text. Each text names the
rule that produced it and the measured values. Examples:

- Many audible or bad roams → enable 802.11r (Fast Roaming), 802.11k and
  802.11v (BSS Transition) on the voice SSID. Check that the handset supports
  802.11r.
- Sticky client → set a minimum RSSI on the APs (for example -75 dBm) or
  lower the handset roaming threshold.
- Ping-pong → lower the transmit power of the two APs or move an AP.
- Weak coverage → add an AP or change the AP position near the markers.
- Downlink loss without uplink loss → check the AP transmit power and
  interference on the AP channel.
- Uplink loss on 2.4 GHz → move the voice SSID to 5 GHz or use band steering.
- General voice SSID checks: DTIM 1 or 2, minimum data rate at least
  12 Mbit/s, WMM on, DSCP EF (46) for RTP on the handset.

TalkOps does not read the UniFi WLAN configuration in this phase. The
recommendations therefore state "check this setting". They do not state
"this setting is wrong".

### Handset preparation

- The survey dialog and the user guide state: switch off VAD / silence
  suppression on the handset for the test.
- Proposal for stage 1: add Yealink catalog settings `vad` and `rtp_dscp`
  for the `wifi` family, if the AX8x parameters are confirmed by the
  official Yealink documentation (expected: `voice.vad` and
  `network.qos.audiotos`). A phone setting override applies them through
  the existing provisioning. Without confirmation, the guide gives only
  manual steps.
- The analysis detects VAD use: long periods with no packets but no
  sequence gap mean silence suppression. The result page then shows a
  warning that loss values are not reliable.

## Technical design

### Components

| Component | Change |
| --- | --- |
| New crate `talkops-unifi` | HTTP client for the UniFi Network API (read only). Typed errors (`thiserror`). Capability detection. Never logs the API key or full URLs with query values. |
| `talkops-core` | Module `wifi_survey` (domain types, queries, rating rules, pattern rules as pure functions) and module `unifi` (connection settings with `SecretBox`). New permissions and job kind. |
| `talkops-api` | Routes, survey controller task (originate, outbound socket session, 1 s media poll, UniFi poll, DTMF markers), CDR extension for the error log of survey calls, retention, `Range` support for recording audio. |
| `talkops-media-worker` | Job `wifi_survey.analyse` (analysis and recommendations) and job `wifi_survey.signal` (render the test signal with Piper once for each language). |
| Dialplan (`fsxml`) | `*39` self-dial: transfer to the outbound socket only for an armed survey of the calling device. Add `*39` to `FIXED_FEATURE_CODES`. |
| Web | Admin page (surveys, settings, UniFi tabs), start dialog, live dashboard, result page, comparison, settings. SVG charts without a new dependency. Texts in `en.ts` and `de.ts`. |

The analysis runs as a job in the media worker. The live collection must run
in the API process, because it needs the ESL connection and the call
lifetime. The API writes raw samples. The worker reads them after the call
and writes the analysis. A re-analysis with new thresholds is a new job.

### Call flow

1. API creates the survey row (`state = calling`) and an audit entry.
2. API originates through ESL:
   `{origination_caller_id_name='TalkOps Wi-Fi test',talkops_survey_id=<id>,
   rtcp_audio_interval_msec=1000,RECORD_STEREO=true}user/<sip_username>@<domain>
   &socket(<api>:<port> async full)`.
   All values that TalkOps does not fully control go through
   `fsxml::sanitize_value`.
3. The outbound socket session answers, starts `record_session`, starts the
   test signal and subscribes to `DTMF` and `RECV_RTCP_MESSAGE` for the call
   UUID.
4. A task polls `mediaStats` once each second and UniFi `stat/sta/{mac}`
   once each second. Each poll writes one sample row with the TalkOps time.
5. At hangup, the CDR arrives. The CDR handler stores the error periods for
   survey calls and links the recording. It then enqueues
   `wifi_survey.analyse`.
6. The worker writes roams, patterns, summary and recommendations and sets
   `state = analysed`.

Only one active survey for each device is permitted. A tenant can run at most
3 surveys at the same time (setting). Poll errors do not stop the call. The
sample row records the error class.

### Clock alignment

All samples use the TalkOps receive time. UniFi values have the
precision of the poll interval (about 1 s plus API latency). FreeSWITCH error
periods use the FreeSWITCH clock. FreeSWITCH and TalkOps run on the same
host (host networking, ADR 0007), so the clocks agree. The analysis uses the
±3 s window to absorb the UniFi uncertainty.

### Data model (draft)

All tables have `tenant_id UUID NOT NULL REFERENCES tenants`.

```text
unifi_connections
  tenant_id PK, base_url, site_id, site_name, api_key_enc,
  tls_mode (system|fingerprint), tls_fingerprint_sha256,
  enabled, detected_version, capabilities JSONB, last_check_at, last_error,
  updated_at, updated_by

wifi_survey_settings
  tenant_id PK, thresholds JSONB (clean_ms, bad_ms, loss_block_pct,
  sticky_rssi, sticky_secs, pingpong_count, pingpong_secs, weak_rssi, weak_secs),
  retention_days (default 30, 1..365), max_minutes (default 60),
  max_parallel (default 3)

wifi_surveys
  id, tenant_id, device_id, extension_id, phone_id, mac (snapshot),
  label, mode (originate|self_dial), state (armed|calling|running|
  collecting|analysing|analysed|failed|cancelled), call_uuid,
  recording_id NULL REFERENCES recordings ON DELETE SET NULL,
  started_by, armed_until, started_at, answered_at, ended_at,
  thresholds JSONB (snapshot), unifi_capabilities JSONB (snapshot),
  summary JSONB, error, expires_at, created_at

wifi_survey_media_samples
  survey_id, tenant_id, at, offset_ms, in_packets, in_expected,
  in_flaws, in_mos_est, in_jitter_max, rtcp_fraction_lost,
  rtcp_cum_lost, rtcp_jitter_ms, poll_error
  PK (survey_id, offset_ms)

wifi_survey_radio_samples
  survey_id, tenant_id, at, offset_ms, ap_mac, ap_name, bssid, radio,
  channel, rssi, signal, tx_rate_kbps, rx_rate_kbps, poll_error
  PK (survey_id, offset_ms)

wifi_survey_events
  id, survey_id, tenant_id, at, offset_ms,
  kind (marker|error_period|unifi_event|state), digit, note,
  data JSONB

wifi_survey_roams       (written by the analysis)
  id, survey_id, tenant_id, at, offset_ms, from_ap_mac, from_ap_name,
  to_ap_mac, to_ap_name, from_radio, to_radio, from_channel, to_channel,
  rssi_before, rssi_after, gap_ms, loss_pct_window, jitter_ms_window,
  rating (clean|audible|bad), flags TEXT[], marker_ids UUID[]
```

Volume: 1 sample each second gives 3,600 rows for each table and hour.
Survey data is deleted after `retention_days`.

Proposed migration IDs (recheck at integration):
`20261107000001_unifi_connections.sql`, `20261107000002_wifi_surveys.sql`.

### API (draft)

All routes have `#[utoipa::path]`, a role check, and an audit entry for each
mutation. Audit data never contains the API key.

| Method and path | Permission | Purpose |
| --- | --- | --- |
| `GET /api/v1/unifi/connection` | `unifi.manage` | Settings without the key (`has_api_key: true`) |
| `PUT /api/v1/unifi/connection` | `unifi.manage` | Save; an empty key keeps the stored key |
| `DELETE /api/v1/unifi/connection` | `unifi.manage` | Remove the connection and the key |
| `POST /api/v1/unifi/connection/test` | `unifi.manage` | Version, sites, capabilities, TLS fingerprint |
| `GET /api/v1/unifi/clients/{mac}` | `wifi_survey.run` | Current state of one handset MAC |
| `GET/PUT /api/v1/wifi-surveys/settings` | `wifi_survey.manage` | Thresholds, retention, limits |
| `POST /api/v1/wifi-surveys` | `wifi_survey.run` | Start (`device_id`, `mode`, `label`) |
| `GET /api/v1/wifi-surveys` | `wifi_survey.run` | List (own; all with `wifi_survey.manage`) |
| `GET /api/v1/wifi-surveys/{id}` | `wifi_survey.run` | State, summary, roams, recommendations |
| `GET /api/v1/wifi-surveys/{id}/timeline` | `wifi_survey.run` | Samples and events |
| `GET /api/v1/wifi-surveys/{id}/live?since=<offset_ms>` | `wifi_survey.run` | Compact live values for the dashboard (polled each second) |
| `POST /api/v1/wifi-surveys/{id}/markers` | `wifi_survey.run` | Add a marker from the UI |
| `POST /api/v1/wifi-surveys/{id}/stop` | `wifi_survey.run` | End the call |
| `POST /api/v1/wifi-surveys/{id}/markers/{marker_id}` | `wifi_survey.run` | Add a note to a marker |
| `POST /api/v1/wifi-surveys/{id}/analyse` | `wifi_survey.manage` | Run the analysis again with current thresholds |
| `DELETE /api/v1/wifi-surveys/{id}` | `wifi_survey.manage` | Delete survey, samples and recording |
| `GET /api/v1/wifi-surveys/compare?a=&b=` | `wifi_survey.run` | Comparison of two surveys |
| `GET /api/v1/wifi-surveys/{id}/export.{json,csv}` | `wifi_survey.run` | Export |
| `GET /api/v1/recordings/{id}/audio` | existing | Add `Range` / `206` support |

New permissions: `wifi_survey.run` (area `diagnostics`),
`wifi_survey.manage` (area `diagnostics`), `unifi.manage` (area `system`,
sensitive). The admin role has all of them. The operator role gets none by
default.

The recording of a survey is visible to the users who can see the survey.
It is not visible in the normal call recording list of the extension.

### Security and privacy

- The UniFi API key is encrypted with `SecretBox` (ADR 0008). The API never
  returns it. Logs and audit entries never contain it.
- TLS: UniFi consoles often use a self-signed certificate. TalkOps accepts
  either a certificate that the system trust store accepts, or a pinned
  SHA-256 certificate fingerprint. The connection test shows the fingerprint.
  The administrator confirms it. There is no "ignore TLS errors" option.
- Read only: the `talkops-unifi` crate in this phase has only GET requests.
  A unit test checks that the client cannot send another HTTP method.
- Data collection only in survey mode: TalkOps reads UniFi only during an
  active survey and only for the MAC of the survey device. TalkOps does not
  store other clients and does not track devices in the background.
- Location data is only the DTMF "new location" marker and an optional
  note. The UI says that a note must not contain personal data.
- Retention: the hourly retention loop deletes surveys, samples, events,
  roams and the survey recording after `retention_days` (default 30).
- The survey call always records. The start dialog and the test signal
  inform the tester. The tester is normally the administrator.

### Tests

- Unit tests for rating and pattern rules with synthetic timelines
  (clean, audible, bad, ping-pong, sticky, drop without roam, VAD detection).
- `#[sqlx::test]` tests for each query (connections, surveys, samples,
  retention, permission checks).
- `talkops-unifi` tests against an axum mock server: classic and Integration
  responses of two versions, missing endpoints, 401, TLS fingerprint pin,
  and the GET-only rule.
- API tests for start/stop, role checks, audit entries without secrets,
  `Range` responses.
- E2E (`tests/e2e`): SIPp answers the survey call, sends RTP with a planned
  gap and DTMF `1`, `2`, `#`. The test checks samples, markers, the error
  period and the analysis. A UniFi mock gives an AP change at the gap.
- Web: `npm run check`, lint, build; a browser check of the result page.

### Documentation

- `docs/en/wifi-survey.md` and `docs/de/wlan-begehung.md`: preparation,
  UniFi API key, run, result page, limits, version table.
- ADR 0041 (Accepted after review), `docs/adr/README.md`, roadmap entry and
  a short section in `docs/architecture.md`.

## Stages and delivery

| Stage | Content | Can run alone | Size estimate |
| --- | --- | --- | --- |
| 0 | Hardware prototype with the user: connection test against the real UniFi console, `stat/sta` and Integration responses, one test call to the AX8x handset with RTCP check. Result goes into this proposal and the version table. | — | Small (part of stage 1/2) |
| 1 | Survey call, test signal, DTMF and UI markers, recording, 1 s media samples, CDR error periods, live dashboard (media tiles), simple result page without UniFi, admin page and permissions, retention, `Range` support | Yes: finds drops without UniFi | Large (~2,500 lines incl. tests) |
| 2 | `talkops-unifi` crate, encrypted connection, UniFi tab, connection test and capabilities, radio samples, live AP/RSSI/rate tiles and live roam display, AP sections in the timeline | Yes | Medium (~1,500 lines) |
| 3 | Analysis job, roam rating, patterns, recommendations, roam table with playback, comparison, export, settings | Yes | Large (~2,000 lines) |
| 4 | Follow-up phase (not in this delivery): handset provisioning (roaming threshold), UniFi write access | — | Separate proposal |

Recommendation: deliver stages 1 to 3 as three PRs in this order. Each PR
is runnable and testable on its own. This keeps reviews small and lets the
release chat ship stage 1 early. One large PR is also possible if the user
prefers it.

### Stage 4: assessment of write access

Recommendation: stage 4 is a follow-up phase with its own ADR, not part of
this delivery. Reasons:

- The current Integration API reference documents writes only for WLAN
  broadcasts (create, update, delete) with few documented fields. Fast
  roaming, minimum RSSI, DTIM and transmit power are not documented there.
  A safe write path through the official API therefore does not exist yet
  for most recommendations.
- Radio settings through the private API change between Network versions.
  A wrong write can disconnect all Wi-Fi clients, also the administrator.
- A snapshot and rollback design needs its own tests on real hardware.

Preparation in this delivery: recommendations have a stable rule ID and
target setting. Stage 4 can attach a "propose change" action to these IDs
later. Stage 4 rules, already agreed in the task: explicit confirmation,
snapshot before write, rollback, official Integration API only, private API
only as an explicitly enabled "experimental" module.

## Open points for the user

1. **Delivery**: three PRs (recommended) or one PR?
2. **Self-dial code**: is `*39` acceptable? Alternative: only originate mode.
3. **Hardware for stage 0**: which UniFi console and Network version and
   which handset model are available? Can the user run the connection test
   and one test call and share the (anonymized) result?
4. **Classic API**: radio values (RSSI, channel, rate) need the classic
   read endpoint `stat/sta`. Is read access to the classic API acceptable
   while writes stay excluded? Without it, the survey has only the AP
   section, not RSSI.
5. **Test signal**: one tone each second plus an announcement every 30 s,
   or a continuous tone?
6. **Recording**: the survey always records, independent of the extension
   settings. Is that acceptable?
7. **Retention default**: 30 days?
8. **Permissions**: should the built-in operator role get `wifi_survey.run`?
9. **Handset provisioning in stage 1**: add `vad` and `rtp_dscp` catalog
   settings once the Yealink parameter names are confirmed?

## Release and upgrade notes (for later PRs)

- New migrations add tables only. No change to existing data.
- No new container capability in stages 1 to 3. The API needs HTTPS access
  to the UniFi console.
- The feature is inactive until a user starts a survey. UniFi is optional.
- Proposed release note (stage 1): "Add a Wi-Fi voice survey for Wi-Fi
  handsets. TalkOps calls the handset, plays a test signal, records the
  call and measures packet loss and drops for each second."

## References

- [ADR 0005](adr/0005-job-queue-postgres.md), [ADR 0006](adr/0006-steuerung-xml-curl-und-esl.md),
  [ADR 0008](adr/0008-secrets-at-rest.md), [ADR 0012](adr/0012-aufzeichnung-und-transkription.md),
  [ADR 0029](adr/0029-roles-and-permissions.md), [ADR 0032](adr/0032-automatic-deployment-capture.md)
- FreeSWITCH v1.10.12 sources: `src/mod/applications/mod_commands/mod_commands.c`
  (`uuid_set_media_stats`, `mediaStats`), `src/switch_core_media.c`
  (`RECV_RTCP_MESSAGE`), `src/switch_rtp.c` (`do_mos`, error periods),
  `src/switch_ivr.c` (CDR `error-log`)
- UniFi Network API reference 10.3.58 (developer.ui.com, community copy:
  <https://github.com/gigabrit/unifi-network-api-reference>)
- UniFi event names: <https://github.com/oznu/unifi-events>
