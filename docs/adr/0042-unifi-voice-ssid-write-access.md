# 0042 - Changes to the UniFi voice SSID through the official API

- Status: Proposed (draft for user review, 2026-10-22).
- Extends: ADR 0041. Also uses ADRs 0008 and 0029.
- Proposal: [UniFi voice SSID write proposal](../unifi-wifi-write-proposal.md).

## Context

The Wi-Fi voice survey (ADR 0041) gives recommendations with stable IDs,
for example `fast_roaming`, `min_rssi`, `band_5ghz` and `voice_ssid`. The
administrator must apply these recommendations by hand in UniFi Network.
ADR 0041 does not let TalkOps change UniFi. It requires a later ADR for
writes, with explicit confirmation, a snapshot, a rollback and the official
Integration API.

The official UniFi Network Integration API has these endpoints for Wi-Fi
broadcasts (SSIDs) in the published specifications 10.6.106 and 11.0.89:

- `GET /v1/sites/{siteId}/wifi/broadcasts` (list, overview fields only)
- `GET /v1/sites/{siteId}/wifi/broadcasts/{id}` (all fields)
- `PUT /v1/sites/{siteId}/wifi/broadcasts/{id}` (full replacement)
- `POST` and `DELETE` (not used by TalkOps)

The base path on a console is `/proxy/network/integration`. The API key goes
in the `X-API-KEY` header.

The update body has the same schema as the create body. The PUT replaces
the full object. Fields that the body does not contain can change to a
default value. From 10.6.106, `channel2gLockedTo6` and
`dtimPeriod2gLockedTo3` are required in the body. The detail schema
contains the Wi-Fi passphrase and other credentials. The PUT body must
contain them again.

A PUT to an SSID makes the access points apply the SSID again. Clients of
this SSID disconnect for a short time. This includes Wi-Fi handsets during
a call.

## Options

1. Write path: (a) official Integration API `PUT wifi/broadcasts/{id}`;
   (b) classic API `rest/wlanconf`; (c) no writes, instructions only.
2. Body: (a) read-modify-write of the full object from a new `GET`;
   (b) a body built from TalkOps data only.
3. Scope: (a) one selected voice SSID and a fixed field allowlist;
   (b) any SSID and any field; (c) also access point radio settings.
4. Secrets of the SSID: (a) keep in memory for one request only;
   (b) store encrypted for rollback.

## Decision

- Use options 1a, 2a, 3a and 4a.
- TalkOps writes to UniFi only with
  `PUT /proxy/network/integration/v1/sites/{siteId}/wifi/broadcasts/{id}`.
  TalkOps sends no other write request to UniFi. TalkOps sends no write
  request to the classic API. The read path of ADR 0041 stays GET only.
- The write code is a separate module of `talkops-unifi`. A test makes sure
  that this module sends only `PUT` to `wifi/broadcasts/{id}` and changes
  only allowlist fields. The GET-only test of the read path stays.
- Writes are off by default. A write needs all of these conditions:
  1. The administrator enabled "Allow write access" on the UniFi connection.
  2. The user has the new sensitive permission `unifi.write`. Only the
     built-in admin role has it by default.
  3. The last version check shows UniFi Network 10.6.106 or later. The
     target version is 11.0.x (reference 11.0.89) on UniFi OS 6.0 or later.
     With an older version, TalkOps stays read only.
  4. The SSID is the voice SSID selected on the connection. Its type is
     `STANDARD` and its `metadata.origin` is `USER_DEFINED`.
  5. No survey of the tenant is in the state `armed`, `calling` or
     `running`. No other change is in progress.
  6. The user confirmed the exact field values in the diff dialog. The SSID
     did not change since TalkOps made the proposal.
- Field allowlist (JSON pointers in the SSID object):
  - `/bssTransitionEnabled`
  - `/securityConfiguration/wpa3FastRoamingEnabled`
  - `/securityConfiguration/pmfMode`
  - `/handoffSuggestionsConfiguration`
  - `/broadcastingFrequenciesGHz`
  - `/uapsdEnabled`
  - `/multicastToUnicastConversionEnabled`
  - `/arpProxyEnabled`
  - `/dtimPeriodByFrequencyGHzOverride`
- Read-modify-write: TalkOps reads the SSID with a new `GET` immediately
  before the `PUT`. TalkOps removes only the read-only fields `id` and
  `metadata`. TalkOps changes only the confirmed allowlist fields. TalkOps
  sends all other fields back without a change. This includes
  `channel2gLockedTo6`, `dtimPeriod2gLockedTo3`, the passphrase and fields
  that TalkOps does not know. After the `PUT`, TalkOps reads the SSID again
  and compares the allowlist fields with the target values.
- TalkOps does not write when the `GET` answer has no readable
  passphrase or pre-shared keys for a security type that needs them. A
  `PUT` without them can fail or remove the credential.
- Snapshot: before each write, TalkOps stores the values of the allowlist
  fields before and after the change. A missing field is stored as
  "absent". TalkOps stores no other field of the SSID.
- Secrets: the passphrase, pre-shared keys and RADIUS data are in memory
  only for one read-modify-write. TalkOps never stores, logs, audits or
  returns them to the UI.
- Rollback: TalkOps restores the stored "before" values of the changed
  fields only, with the same read-modify-write. If a changed field has a
  value different from the stored "after" value, another person changed
  the SSID. TalkOps then shows these fields and asks for a second
  confirmation.
- Each apply and rollback writes an audit entry with the SSID ID, the SSID
  name, the field names and the snapshot values. The entry contains no
  secret.
- The connection test reports the Network version, whether
  `wifi/broadcasts` is readable, which allowlist fields the version gives
  and whether the version permits writes. The connection test never writes.
- Access point radio settings (for example `txPowerMode`), 802.11k,
  minimum data rates and 802.11r on WPA2-only SSIDs stay instructions in
  this stage. TalkOps does not write them.

## Consequences

- Positive: the administrator can apply survey recommendations without a
  manual change in UniFi. Each change has a snapshot, an audit entry and a
  rollback.
- Positive: TalkOps without the write switch works exactly as before.
- Negative: each write disconnects the clients of the voice SSID for a
  short time. TalkOps blocks writes during a survey. The UI shows a warning.
- Negative: the Integration API has no version token or ETag for an SSID.
  A change by another person between the second `GET` and the `PUT` is
  possible. The time window is short. The read-back after the `PUT` finds
  a mismatch of the allowlist fields.
- Negative: field names and the behavior of the `PUT` come from the
  published specification and a community client. A test on a real
  11.0 console must confirm them before the feature is marked as tested.
- Follow-up: hardware test (stage 0 of the proposal), and a later decision
  about 802.11r on WPA2-only SSIDs, minimum data rates and radio settings.
