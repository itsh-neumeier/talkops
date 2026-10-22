# 0041 - Wi-Fi voice survey and read-only UniFi correlation

- Status: Accepted by the user on 2026-10-22.
- Extends: ADRs 0005, 0006, 0008, 0012 and 0029.
- Proposal: [Wi-Fi voice survey proposal](../wlan-voice-survey-proposal.md).

## Context

Administrators must check that Wi-Fi handsets (`DeviceKind::Wifi`) keep good
voice quality everywhere on a site and during roaming between access points.
A user cannot hear the cause of a drop. The cause can be a slow roam, a weak
signal, interference or a network problem behind the access point.

FreeSWITCH 1.10.12 gives cumulative RTP counters during a call through the
`uuid_set_media_stats` API and the `mediaStats` JSON API. It sends RTCP
receiver reports from the far end as `RECV_RTCP_MESSAGE` events. The XML CDR
contains error periods with start and stop times. FreeSWITCH does not give
a jitter value for each second.

UniFi Network has an official Integration API with API keys. Its published
client schema does not contain radio values (RSSI, channel, data rate). The
classic API endpoint `stat/sta` has these values. Roam events are available
only through the classic event endpoint or syslog/SIEM. Field sets change
between Network versions.

## Options

1. Media data: (a) FreeSWITCH counters, RTCP events and CDR error periods;
   (b) a packet capture of the survey call; (c) analysis of the recording only.
2. Wi-Fi data: (a) Integration API only; (b) Integration API plus read-only
   classic `stat/sta`; (c) syslog/SIEM events.
3. Analysis place: (a) in the API process during the call; (b) a media
   worker job after the call.

## Decision

- Use option 1a. Poll `mediaStats` for the survey call once each second and
  store the differences. Set `rtcp_audio_interval_msec=1000` on the survey
  leg. Store the CDR error periods of survey calls. Add a packet capture
  (1b) only if a hardware test proves that per-second jitter is necessary.
  It needs a separate decision because it needs `CAP_NET_RAW`.
- Use option 2b. Add the crate `talkops-unifi`. It sends only GET requests
  in this phase. It reads sites, devices and client identity through the
  Integration API where available. It reads radio values for one MAC
  through classic `stat/sta`. TalkOps derives roams from AP/BSSID changes.
  The classic event endpoint is an optional source of exact roam times.
  Syslog/SIEM is not used in this phase.
- Store the UniFi API key with `SecretBox` (ADR 0008). Accept a server
  certificate from the system trust store or a pinned SHA-256 fingerprint.
  Do not offer an option that ignores TLS errors.
- TalkOps originates the survey call to one device through ESL and controls
  it with the outbound socket (ADR 0006). The socket session plays the test
  signal with `playback` and `playback_terminators=none`. `endless_playback`
  is not used: it does not read the DTMF queue, so FreeSWITCH fires no
  `DTMF` events. The survey records the call with `record_session` in
  stereo (ADR 0012). The survey owns its recording file; it is not a row of
  the `recordings` table, so it does not appear in the call recordings of
  the extension and is deleted with the survey. The self-dial feature code
  `*39` starts only a survey that a user armed in the web UI for the
  calling device.
- The API process collects raw samples during the call (3a for collection
  only). A media worker job (ADR 0005) does the analysis, the roam rating,
  pattern detection and recommendations (3b). Rules are pure functions in
  `talkops-core` with editable thresholds.
- The live dashboard polls a compact endpoint once a second, like other
  live views. It does not use a new WebSocket or SSE channel.
- Collect Wi-Fi and location data only for an active survey and only for
  the survey device. Delete survey data and the survey recording after a
  configurable retention period (default 30 days).
- New permissions `wifi_survey.run`, `wifi_survey.manage` and the sensitive
  `unifi.manage` (ADR 0029). Each mutation writes an audit entry without
  secrets.
- Writes to UniFi are not part of this decision. A later ADR must decide
  them. That ADR must require explicit confirmation, a snapshot, a rollback,
  and the official Integration API. Private API writes can only be an
  explicitly enabled experimental module.

## Consequences

- Positive: no new container permission and no new media component. The
  survey works without UniFi. Results use measured values from the call.
- Positive: the analysis can run again with new thresholds, because raw
  samples are stored.
- Negative: uplink jitter has no per-second value. MOS is the FreeSWITCH
  estimate. Downlink values depend on the RTCP interval of the handset.
- Negative: the classic UniFi API is not an official stable interface. The
  connection test must detect available endpoints for each version. The
  user documentation keeps a version table.
- Negative: radio values have a precision of about one poll interval.
  The analysis uses a ±3 s window around each roam.
- Follow-up: hardware test with a real UniFi console and a Wi-Fi handset,
  handset provisioning values (VAD, DSCP, roaming threshold), and the
  write-access ADR.
