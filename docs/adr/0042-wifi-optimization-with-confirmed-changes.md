# 0042 - Wi-Fi optimization with confirmed changes

- Status: Proposed (draft for user review, 2026-10-22).
- Extends: ADRs 0008, 0029, 0038, 0039 and 0041.
- Proposal: [Wi-Fi optimization proposal](../wlan-optimization-proposal.md).

## Context

The Wi-Fi survey (ADR 0041) gives recommendations as text. Administrators
want to apply some of them from TalkOps. ADR 0041 excludes writes to UniFi
and requires a separate decision with confirmation, snapshot, rollback and
the official API.

Yealink documents the handset roaming threshold
(`static.network.wifi.roaming_threshold`, -100 to -30 dBm). The pinned
FusionPBX sources contain `voice.vad` and `static.network.qos.audiotos`.
The UniFi Integration API has a full-replacement update for Wi-Fi
broadcasts (SSIDs) with some roaming fields. It has no endpoint for radio
settings. A broadcast object contains the Wi-Fi passphrase.

## Options

1. Handset values: (a) Yealink catalog settings with phone overrides;
   (b) free parameter text per phone.
2. UniFi writes: (a) official Integration API, SSID fields from an
   allowlist; (b) private classic API for SSID and radio settings;
   (c) no writes.
3. Confirmation: (a) explicit confirmation for each change set with a typed
   SSID name; (b) automatic application.

## Decision

- Use 1a. Add `vad`, `audio_dscp` and `wifi_roaming_threshold` to the
  Yealink catalog for the `wifi` family. Add an `int` setting kind with a
  range. Store applied values as phone overrides. Resync only on request.
- Use 2a. Write only through the official Integration API. Read the full
  broadcast object, change only allowlisted fields that the object of the
  detected version contains, and send the full object back. Never change
  the SSID name, security mode, passphrase, VLAN or network.
- Use 3a. Every change is a change set with a preview, a typed
  confirmation, a conflict check, an encrypted snapshot (ADR 0008), a
  verification after the write and a rollback with its own conflict check.
  No automatic changes. One open change set for each tenant.
- Write access is off by default (`allow_changes`). A new sensitive
  permission `unifi.write` controls UniFi change sets. Audit entries never
  contain snapshots, passphrases or keys.
- Keep writes in a separate module of `talkops-unifi`. The read module
  keeps its GET-only test.
- Do not use the private API (2b) in this decision. A later experimental
  module needs its own decision and switch.

## Consequences

- Positive: administrators apply measured recommendations and verify them
  with a second survey. Every change can be undone.
- Negative: a SSID change reconnects its clients for some seconds. The UI
  must warn, and the change must not run during a survey.
- Negative: field names change between Network versions. Unknown fields are
  shown as not available and are not sent.
- Negative: transmit power, channel and minimum data rate stay manual.
- Follow-up: hardware test of the survey with the real console before the
  UniFi part, because the real broadcast object decides the field mapping.
