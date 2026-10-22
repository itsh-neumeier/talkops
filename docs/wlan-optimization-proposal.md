# Wi-Fi optimization after a survey (stage 4): proposal for user review

Status: Proposed. Waiting for user review. No code, migration or UI change yet.
Date: 2026-10-22 (Europe/Berlin).
Base: `origin/main`, `c093fa8` (TalkOps 1.8.6, includes the Wi-Fi survey).
ADR draft: [ADR 0042](adr/0042-wifi-optimization-with-confirmed-changes.md).
Previous stage: [Wi-Fi survey proposal](wlan-voice-survey-proposal.md), [ADR 0041](adr/0041-wifi-voice-survey-and-unifi-correlation.md).

## Goal and review boundary

The Wi-Fi survey (TalkOps 1.8.6) measures, rates roams and gives
recommendations as text. This stage turns selected recommendations into
changes that an administrator can apply after explicit confirmation:

- on the handset, through the existing Yealink provisioning;
- on the UniFi voice SSID, through the official UniFi Integration API, with
  a snapshot and a rollback.

The user asked for a proposal first. This revision contains only this
proposal, an ADR draft and a task summary.

## New facts since the first proposal

| Fact | Source | Effect |
| --- | --- | --- |
| The AX83H takes `static.network.wifi.roaming_threshold`, integer -100 to -30, default -70. Below this signal, the handset scans for a better AP. | Yealink "Wi-Fi Roaming Guide" for the AX83H (Auto Provisioning table) | The handset roaming threshold can be provisioned with a documented range. |
| The pinned FusionPBX AX83H/AX86R templates contain `voice.vad` and `static.network.qos.audiotos`. They write the roaming value as `network.wifi.roaming_threshold` (without `static.`). | `crates/talkops-provisioning/reference/fusionpbx/yealink/ax83h/y000000000180.cfg` (ADR 0039) | VAD and DSCP names are confirmed by two sources. For the roaming threshold, TalkOps uses the Yealink guide name. |
| The Integration API (reference 10.0.162 and later) has `PUT /v1/sites/{siteId}/wifi/broadcasts/{id}`. Documented fields include `bssTransitionEnabled` (802.11v), `bandSteeringEnabled` and `dtimPeriodByFrequencyGHzOverride`. | developer.ui.com, "Update Wifi Broadcast" | Some SSID recommendations can be applied through the official API. |
| PUT replaces the whole object: the body must contain all required fields, not only the changed ones. Network 10.6.106 and later add required fields (`channel2gLockedTo6`, `dtimPeriod2gLockedTo3`) and an optional `handoffSuggestionsConfiguration` with RSSI thresholds for 5 and 6 GHz. Fast roaming (802.11r) appears as `wpa3FastRoamingEnabled` inside `securityConfiguration`. | Community client for the official API (Art-of-WiFi), to verify against the schema of the console | TalkOps must read the full object, change only known fields and send the full object back. Field names depend on the version. |
| The official API has no endpoint for radio settings (transmit power, channel) or the minimum data rate. | Official reference and the community client | These recommendations stay text only. |
| The broadcast object contains the Wi-Fi passphrase. | Schema (`securityConfiguration`) | A snapshot contains a secret. It must be encrypted and never shown or logged. |

## Proposed scope

### A. Handset settings (low risk)

Add three Yealink catalog settings for the `wifi` family (AX83H, AX86R) in
`presets/phones/yealink.yaml`, with the sources in the file header:

| Key | Parameter | Kind | Values | Factory value |
| --- | --- | --- | --- | --- |
| `vad` | `voice.vad` | bool | 0, 1 | 0 |
| `audio_dscp` | `static.network.qos.audiotos` | integer | 0 to 63 | 46 |
| `wifi_roaming_threshold` | `static.network.wifi.roaming_threshold` | integer | -100 to -30 | -70 |

The catalog has only `bool` and `choice` today. Add an `int` kind with
`min` and `max`. Validation, the settings editor and the phone settings API
use the same rule.

The survey result page gets **Apply to handset** for the recommendations
`disable_vad`, `min_rssi` (handset part) and `voice_ssid` (DSCP part):

1. TalkOps shows the phone, the current value (phone override, tenant
   value or factory value) and the proposed value.
   Example: roaming threshold -70 → -72 dBm. The proposal comes from the
   measured RSSI at the sticky-client periods, limited to -75 to -65 dBm.
2. The administrator confirms.
3. TalkOps stores the value as a phone override in `phones.settings`
   (existing mechanism) and writes an audit entry.
4. Optional: **Resync now** sends the existing check-sync NOTIFY.
5. The page suggests a new survey with the label "after" for the
   comparison.

Undo: the change list of the survey shows each handset change with
**Undo**. Undo restores the previous override (or removes it) and offers a
resync.

### B. UniFi SSID settings through the official API (medium risk)

A change of an SSID causes the access points to send the SSID again. All
clients of that SSID can lose the connection for some seconds, also
handsets in calls. The design therefore has several protections.

**Off by default.** A new switch **Allow changes** in the UniFi tab
(permission `unifi.manage`) enables write access. The API key must have
write rights in UniFi. The connection test reports whether the Integration
API offers the broadcast endpoints for the detected version.

**Change set.** For the recommendations `fast_roaming`, `min_rssi` (AP
part), `band_5ghz` and `voice_ssid` (DTIM part), TalkOps prepares a change
set for one SSID:

| Recommendation | Field (only if the object of this version contains it) | Change |
| --- | --- | --- |
| `fast_roaming` | `bssTransitionEnabled` | `true` (802.11v) |
| `fast_roaming` | `securityConfiguration.wpa3FastRoamingEnabled` | `true` (802.11r) |
| `min_rssi` | `handoffSuggestionsConfiguration.band5GHzRssiThreshold` (and 6 GHz) | measured value, for example -75 |
| `band_5ghz` | `bandSteeringEnabled` | `true` |
| `voice_ssid` | `dtimPeriodByFrequencyGHzOverride` | 2.4 GHz: 1, 5 GHz: 1 (or 2, setting) |

TalkOps changes only fields from this list. TalkOps changes a field only if
it is present in the object that it read from the console. A field that the
version does not have is shown as "not available in Network x.y" and is not
sent. TalkOps never changes the SSID name, the security mode, the
passphrase, VLAN or the network.

**Flow:**

1. **Prepare.** TalkOps reads the SSID (`GET`), shows a table with field,
   current value and new value, and the warning about reconnecting clients.
   Nothing is changed yet.
2. **Confirm.** The administrator types the SSID name to confirm. The
   change set expires after 10 minutes.
3. **Snapshot.** TalkOps reads the SSID again. If it differs from the
   prepared state, TalkOps stops ("changed in the meantime"). Otherwise
   TalkOps stores the full object encrypted (ADR 0008) as the snapshot.
4. **Apply.** TalkOps sends the full object with the changed fields
   (`PUT`), reads it again and checks that the fields have the new values.
5. **Record.** The change set gets the state `applied` or `failed`, with the
   field list (before, after) without secrets, and an audit entry.

**Rollback.** **Undo** on an applied change set reads the SSID. If the
fields still have the values that TalkOps wrote, TalkOps sends the snapshot
back (`PUT`) and verifies it. If someone changed the SSID after TalkOps,
TalkOps shows the differences and asks for a second confirmation before it
overwrites them. Snapshots are kept for the survey retention period, at
least 30 days, and are deleted with the change set.

**Limits:** one open change set for each tenant. No automatic changes. No
change during a running survey on the same SSID (the survey would measure
the reconnect).

### C. Private API for radio settings (not in this stage)

Transmit power, channel width, channel and minimum data rate need the
private classic API (`rest/device`, `rest/wlanconf`). Its format changes
between versions, and a wrong write can disconnect every client and the
administrator. Recommendation: do not implement it in this stage. If the
user wants it later, it gets its own proposal and an explicit
"experimental" switch, separate from **Allow changes**.

## Data model (draft)

```text
unifi_connections
  + allow_changes BOOLEAN NOT NULL DEFAULT false

wifi_changes
  id, tenant_id, survey_id NULL REFERENCES wifi_surveys ON DELETE SET NULL,
  target (handset|unifi_ssid), phone_id NULL, ssid_id NULL, ssid_name,
  recommendation, fields JSONB  -- [{path, before, after}], no secrets
  snapshot_enc TEXT NULL        -- full SSID object, encrypted (UniFi only)
  applied_hash TEXT NULL        -- hash of the written field values
  state (prepared|applied|failed|rolled_back|expired),
  created_by, created_at, expires_at, applied_at, rolled_back_at, error,
  delete_after
```

Proposed migration IDs (recheck at implementation and integration):
`20261108000001_wifi_changes.sql`.

## API (draft)

| Method and path | Permission | Purpose |
| --- | --- | --- |
| `POST /api/v1/wifi-surveys/{id}/changes` | `wifi_survey.manage` (handset) or `unifi.write` (UniFi) | Prepare a change set from a recommendation |
| `GET /api/v1/wifi-changes/{id}` | as above | Preview with fields and warnings |
| `POST /api/v1/wifi-changes/{id}/apply` | as above | Apply (body: typed confirmation) |
| `POST /api/v1/wifi-changes/{id}/rollback` | as above | Undo (body: confirmation if changed in the meantime) |
| `GET /api/v1/wifi-changes` | `wifi_survey.run` | Change history |
| `PUT /api/v1/unifi/connection` | `unifi.manage` | + `allow_changes` |

New permission `unifi.write` (sensitive). The admin role has it. Other roles
do not. Each step writes an audit entry without snapshot, passphrase or key.

## UI (draft)

- Result page, recommendations: buttons **Apply to handset** and
  **Prepare UniFi change** next to the matching recommendations.
- Dialog: field table (current → new), warnings, confirmation input for the
  SSID name, **Apply**.
- New tab **Changes** under Settings → Wi-Fi survey: list with state,
  survey, target, fields, user, time, **Undo**.
- UniFi tab: **Allow changes** switch with explanation, write capability
  from the connection test.

## Tests (plan)

- Catalog: `int` kind, ranges, rendering of the three parameters in the
  Yealink `wifi` family only.
- Change sets: field allowlist, fields missing in the version are not sent,
  full-object PUT, verification, conflict detection before apply and before
  rollback, expiry, one open set, snapshot encrypted and never in API or
  audit output.
- UniFi client: PUT only in a separate write module that requires
  `allow_changes`. The read-only test of `talkops-unifi` keeps its rule for
  the read module.
- Mock console tests for 10.0.x and 10.6.x payloads (required fields).
- API permission and audit tests; web check, lint, build.

## Effort

| Part | Size |
| --- | --- |
| A. Handset settings, `int` kind, apply/undo, resync | Small to medium |
| B. UniFi change sets, snapshot, apply, rollback, UI | Medium to large |
| Docs (en/de), ADR, tests | Medium |

Recommendation: deliver A first (one PR), then B (second PR) after a
hardware test of stage 1–3 with the real console. The hardware test shows
the real broadcast object of the console version, which B depends on.

## Open points for the user

1. **Order**: A and B in two PRs (recommended), or together?
2. **UniFi version**: which Network version runs on your console? B depends
   on the field names of that version.
3. **API key rights**: can the UniFi API key have write rights, or should
   TalkOps use a separate key for changes?
4. **Confirmation**: is typing the SSID name enough, or should a second
   user approve UniFi changes?
5. **Private API (C)**: keep it excluded, as recommended?
6. **Roaming threshold proposal**: limit the handset proposal to -75 to
   -65 dBm, or allow the full range -100 to -30 with a warning?

## References

- Yealink, "Wi-Fi Roaming Guide" (AX83H), parameter `static.network.wifi.roaming_threshold`
- Ubiquiti, UniFi Network API reference, "Update Wifi Broadcast" (developer.ui.com, v10.0.162)
- Art-of-WiFi, UniFi Network Application API client (community, field and version notes)
- [ADR 0008](adr/0008-secrets-at-rest.md), [ADR 0029](adr/0029-roles-and-permissions.md),
  [ADR 0038](adr/0038-vendor-provisioning-profiles.md), [ADR 0039](adr/0039-source-derived-multi-vendor-provisioning.md),
  [ADR 0041](adr/0041-wifi-voice-survey-and-unifi-correlation.md)
