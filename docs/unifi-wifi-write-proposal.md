# Wi-Fi survey stage 4a: changes to the UniFi voice SSID: proposal for user review

Status: Draft for user review. No implementation yet.
Date: 2026-10-22 (Europe/Berlin).
Base: `origin/main`, `c093fa8dddd40a14823d9e6ed41922e0e38efc74` (TalkOps 1.8.6).
ADR draft: [ADR 0042](adr/0042-unifi-voice-ssid-write-access.md).
Previous stage: [Wi-Fi voice survey proposal](wlan-voice-survey-proposal.md), ADR 0041.

## Goal

The result page of a Wi-Fi survey shows recommendations. Stage 4a lets an
administrator apply some of them to the UniFi voice SSID:

1. TalkOps makes a proposal from the recommendations ("Propose change").
2. A diff dialog shows the current value and the new value of each field.
3. The administrator confirms. TalkOps makes a snapshot and writes the SSID.
4. The change history shows each change with a rollback.
5. The administrator starts a new survey to compare the result.

TalkOps reads and writes the SSID only through the official UniFi Network
Integration API (`/proxy/network/integration/v1/...`, header `X-API-KEY`).
TalkOps sends no write request to the classic API.

## API facts

Source: the OpenAPI files `openapi-10.6.106.json` and `openapi-11.0.89.json`
in <https://github.com/Art-of-WiFi/unifi-network-application-api-client>
(copies of the developer.ui.com specification). I compared the Wi-Fi
broadcast schemas of both files. The field sets are the same.

| Request | Body / answer |
| --- | --- |
| `GET /v1/info` | `applicationVersion` (already used) |
| `GET /v1/sites/{siteId}/wifi/broadcasts` | Page of overview objects: `id`, `name`, `type`, `enabled`, `metadata`, `network`, `securityConfiguration.type`, `broadcastingFrequenciesGHz` |
| `GET /v1/sites/{siteId}/wifi/broadcasts/{id}` | Detail object, all fields |
| `PUT /v1/sites/{siteId}/wifi/broadcasts/{id}` | Body "Wifi broadcast create or update"; answer: detail object |

Findings that affect the design:

- The PUT body is the full object. Required fields for `STANDARD`:
  `type`, `name`, `enabled`, `securityConfiguration`, `hideName`,
  `clientIsolationEnabled`, `multicastToUnicastConversionEnabled`,
  `uapsdEnabled`, `advertiseDeviceName`, `arpProxyEnabled`,
  `bssTransitionEnabled`, `broadcastingFrequenciesGHz`,
  `channel2gLockedTo6`, `dtimPeriod2gLockedTo3`.
- The detail object has `id` and `metadata`. The update schema does not
  have them. TalkOps removes these two fields from the PUT body.
- `securityConfiguration` contains `passphrase` (required for WPA2/WPA3
  personal), `presharedKeys` (WPA2 personal) and `radiusConfiguration`.
  The detail schema lists the passphrase, so a GET probably returns it.
  Stage 0 must confirm this (see open points).
- The DTIM override is `dtimPeriodByFrequencyGHzOverride` with the required
  keys `"2.4"`, `"5"` and `"6"` (each 1 to 255). If `dtimPeriod2gLockedTo3`
  is `true`, the value for `"2.4"` must be 3.
- `handoffSuggestionsConfiguration` has `band5GHzRssiThreshold`
  (-80 to -60 dBm) and `band6GHzRssiThreshold` (-90 to -70 dBm). `null`
  disables a band. `null` for the object disables the function.
- `pmfMode` is `REQUIRED`, `OPTIONAL` or `null` (off). It exists for
  `WPA2_PERSONAL` and `WPA2_WPA3_PERSONAL`, not for `WPA3_PERSONAL`.
- `wpa3FastRoamingEnabled` exists for `WPA2_WPA3_PERSONAL` and is required
  there. The specification says: "WPA3 fast roaming can be enabled only if
  the default fast roaming is enabled". Default fast roaming (802.11r) is
  `securityConfiguration.fastRoamingEnabled`. It is **not** in the task
  allowlist. See open question Q1.
- `broadcastingFrequenciesGHz` is an array of numbers (`2.4`, `5`, `6`).
- The API also has `basicDataRateKbpsByFrequencyGHz` (minimum data rate)
  and `bandSteeringEnabled`. They are not in the allowlist. TalkOps keeps
  them unchanged.
- `metadata.origin` is `USER_DEFINED`, `DERIVED` or `ORCHESTRATED`.
  TalkOps writes only `USER_DEFINED` SSIDs. An orchestrated SSID belongs
  to a central management (Site Manager).
- The specification has no ETag and no version token for an SSID.

## Safety gates

TalkOps writes only if all conditions are true. The API checks each
condition again on apply and on rollback.

| # | Condition | Error if false |
| --- | --- | --- |
| 1 | "Allow write access" is on for the UniFi connection | 403 `write_disabled` |
| 2 | The user has `unifi.write` | 403 |
| 3 | Last version check: Network 10.6.106 or later | 409 `version_unsupported` |
| 4 | The SSID is the selected voice SSID, `type` `STANDARD`, origin `USER_DEFINED` | 409 `ssid_not_writable` |
| 5 | No survey of the tenant in `armed`, `calling` or `running` | 409 `survey_active` |
| 6 | No other change in progress (Postgres advisory lock per tenant) | 409 `change_in_progress` |
| 7 | The request has `confirm: true` and the fields come from the diff dialog | 400 |
| 8 | The allowlist values of the SSID did not change since the proposal (`base_hash`) | 409 `ssid_changed` |
| 9 | The GET answer has the credentials that its security type needs | 409 `secret_not_readable` |

Without condition 1, TalkOps works exactly as today: no new UniFi request,
no new button that writes. The SSID list and the proposal (read only) are
available with `unifi.manage`.

## Field allowlist

| JSON pointer | Type and limits | Notes |
| --- | --- | --- |
| `/bssTransitionEnabled` | bool | 802.11v BSS transition |
| `/securityConfiguration/wpa3FastRoamingEnabled` | bool | Only if the field exists and `fastRoamingEnabled` is `true` |
| `/securityConfiguration/pmfMode` | `REQUIRED`, `OPTIONAL`, `null` | Only if the field exists |
| `/handoffSuggestionsConfiguration` | object or `null` | Limits from the specification |
| `/broadcastingFrequenciesGHz` | non-empty subset of `2.4`, `5`, `6` | |
| `/uapsdEnabled` | bool | |
| `/multicastToUnicastConversionEnabled` | bool | |
| `/arpProxyEnabled` | bool | |
| `/dtimPeriodByFrequencyGHzOverride` | object with `"2.4"`, `"5"`, `"6"` (1 to 255), or absent | `"2.4"` = 3 if `dtimPeriod2gLockedTo3` |

A field that the GET answer of a version does not contain is "not
available". TalkOps does not propose it. TalkOps never adds a field that
is not in this list.

## Mapping: recommendation to proposal

The core module makes the proposal from the survey result and the current
SSID. It is a pure function with unit tests. The administrator can clear
each row in the diff dialog.

| Recommendation | Proposed change | Condition |
| --- | --- | --- |
| `fast_roaming` | `bssTransitionEnabled` → `true` | value is not `true` |
| `fast_roaming` | `securityConfiguration.wpa3FastRoamingEnabled` → `true` | field exists and `fastRoamingEnabled` is `true` |
| `min_rssi` | `handoffSuggestionsConfiguration.band5GHzRssiThreshold` → survey threshold `sticky_rssi`, limited to -80…-60 | 5 GHz in the frequencies; the 6 GHz value stays |
| `band_5ghz` | `broadcastingFrequenciesGHz` → current list without `2.4` | list contains `5` |
| `voice_ssid` | `uapsdEnabled` → `true` | |
| `voice_ssid` | `multicastToUnicastConversionEnabled` → `true` | |
| `voice_ssid` | `arpProxyEnabled` → `true` | |
| `voice_ssid` | `dtimPeriodByFrequencyGHzOverride` → `{"2.4": 1, "5": 1, "6": 1}` (`"2.4"`: 3 if locked) | see Q3 |
| `voice_ssid` | `securityConfiguration.pmfMode` → `OPTIONAL` | only if now `null`; TalkOps never lowers `REQUIRED` |

Instructions only (no write in this stage). The dialog shows them as a list
with the reason:

| Topic | Reason |
| --- | --- |
| 802.11r on WPA2 (`fastRoamingEnabled`) | Not in the allowlist; compatibility risk with older clients |
| 802.11k | No field in the Integration API |
| Minimum data rate | `basicDataRateKbpsByFrequencyGHz` affects all clients; not in the allowlist |
| AP transmit power (`txPowerMode`) | Access point radio setting; excluded |
| `pingpong`, `coverage`, `interference`, `downlink_loss` | Placement or radio problem; no SSID field |
| `disable_vad`, `connect_unifi` | Not a UniFi setting |

The dialog shows two warnings:

- "Clients of this SSID disconnect for a short time when TalkOps applies
  the change." (always)
- Fast roaming compatibility: "Some older clients cannot connect when
  802.11r or WPA3 fast roaming is on. Check the handset documentation."
  (when a fast-roaming row is selected)

## Data model

New migration `20261108000001_unifi_wifi_changes.sql` (ID to check again
before the PR; current highest: `20261107000002`):

- `unifi_connections`: add `voice_wifi_id TEXT` (UUID text from UniFi),
  `voice_wifi_name TEXT` (name at selection, for display) and
  `write_enabled BOOLEAN NOT NULL DEFAULT false`. A save of the connection
  without these fields keeps them. A change of URL or site clears
  `voice_wifi_id` and sets `write_enabled` to `false`.
- New table `unifi_wifi_changes`:
  - `id`, `tenant_id`, `site_id`, `wifi_id`, `wifi_name`
  - `kind` (`apply`, `rollback`), `rollback_of` (reference to the change)
  - `survey_id` (`ON DELETE SET NULL`; the history stays after the
    survey retention)
  - `state` (`applying`, `applied`, `failed`, `rolled_back`)
  - `fields TEXT[]`, `before JSONB`, `after JSONB` (allowlist values only;
    a missing field is `{"absent": true}`)
  - `network_version`, `error` (error class only), `created_by`,
    `created_at`, `finished_at`
- New permission `unifi.write` (area `wifi`, sensitive). The admin role
  has it automatically. The operator and user roles do not get it.

TalkOps never writes the full SSID object to the database, a log, the
audit log or an API answer. A test checks that a passphrase from the mock
console appears in no stored row, no audit entry and no API answer.

## Crate `talkops-unifi`

- `lib.rs` (read path, GET only, test stays):
  - `wifi_broadcasts(site_id)`: list for the SSID selection.
  - `wifi_broadcast(site_id, id)`: detail object in a type `Broadcast`.
    `Broadcast` has no `Serialize`, and its `Debug` shows only ID, name and
    type.
  - `probe()` also reads `wifi/broadcasts` and, if a voice SSID is
    selected, its detail object. It reports which allowlist fields exist.
- `allowlist.rs` (pure): field list, value checks, snapshot extraction,
  merge of the changes into a copy of the object, comparison.
- `write.rs` (write path): `WifiWriter::apply(site_id, id, changes, expected)`:
  1. GET the detail object.
  2. Compare its allowlist values with `expected`. Stop with `Conflict` if
     they differ.
  3. Check the credentials of the security type.
  4. Remove `id` and `metadata`. Merge the changes. Keep all other fields.
  5. `PUT` the object.
  6. GET again and compare the allowlist fields with the target values.
  7. Return the snapshots "before" and "after".
  The `PUT` is the only write request in the crate. A source test checks
  that `write.rs` has exactly one `.put(` with the path
  `/proxy/network/integration/v1/sites/{site}/wifi/broadcasts/{id}` and no
  other HTTP method. A mock test records all requests and checks that the
  PUT body differs from the GET body only in allowlist fields (plus the
  removed `id` and `metadata`).

## API

| Route | Permission | Purpose |
| --- | --- | --- |
| `GET /api/v1/unifi/wifi-broadcasts` | `unifi.manage` | SSID list for the selection |
| `PUT /api/v1/unifi/connection` | `unifi.manage`; `unifi.write` to switch on writes | Also `voice_wifi_id` and `write_enabled` |
| `POST /api/v1/unifi/connection/test` | `unifi.manage` | Extended result (see below) |
| `GET /api/v1/wifi-surveys/{id}/unifi-proposal` | `unifi.manage` | Proposal, hints, warnings, `base_hash`, write state; no write |
| `POST /api/v1/unifi/wifi-changes` | `unifi.write` | Apply: `{survey_id, wifi_id, changes, base_hash, confirm}` |
| `GET /api/v1/unifi/wifi-changes` | `unifi.manage` | Change history |
| `POST /api/v1/unifi/wifi-changes/{id}/rollback` | `unifi.write` | Rollback: `{confirm, accept_foreign_changes}` |

Rollback:

1. TalkOps reads the SSID.
2. For each changed field, TalkOps compares the current value with the
   stored "after" value.
3. If a field differs and `accept_foreign_changes` is not set, the answer is
   409 with the list of fields (stored "after", current, "before"). The UI
   asks again.
4. TalkOps writes the stored "before" values of the changed fields only.
   Other fields stay as they are now. A field that was "absent" before is
   removed.

Audit actions: `unifi.write_access` (switch on or off), `unifi.voice_ssid`
(selection), `unifi.wifi.apply`, `unifi.wifi.rollback`. Each entry has the
SSID ID and name, the field names and the snapshot values. No entry
contains a secret.

Connection test (`capabilities`, extended):

```json
{
  "version": "11.0.89",
  "wifi_broadcasts": true,
  "voice_wifi": { "found": true, "type": "STANDARD", "origin": "USER_DEFINED" },
  "allowlist_fields": ["bssTransitionEnabled", "uapsdEnabled", "..."],
  "write_min_version": "10.6.106",
  "write_supported": true
}
```

The connection test never writes. The UI states that only an apply shows
whether the API key has write rights.

## UI

Follow `docs/ui-guidelines.md` (`PageHeader`, `Tabs`, `Modal`, `.scroll-x`,
390 px without horizontal scroll). All texts in `en.ts` and `de.ts`.

- Settings → Wi-Fi survey → UniFi:
  - "Voice SSID" selection (list from UniFi; `IOT_OPTIMIZED` and
    orchestrated SSIDs disabled with a reason).
  - Switch "Allow write access" with the version state and a short risk
    text. Disabled below 10.6.106 or without `unifi.write`.
  - Connection test result with the new rows.
- New tab "Changes" (history): time, user, SSID, fields, state, survey
  link. Row action "Roll back" opens a confirmation modal. On 409 the
  modal shows the foreign changes and asks again.
- Result page of a survey, section "Recommendations": button "Propose
  change". The button is visible with `unifi.manage`. It writes nothing.
  Without write access the dialog shows the proposal, but "Apply" is
  disabled with the reason (for example "Write access is off" or "A survey
  is running").
- Diff dialog (`Modal`): table with checkbox, field, current value, new
  value and recommendation; list of instructions; the warnings; a confirm
  check box "I understand that clients of this SSID disconnect for a short
  time"; button "Apply". After success: the new values, a link to the
  history and the button "Start a new survey for comparison" (opens the
  start dialog with the same device).

## Tests

- `talkops-unifi`: axum mock console with detail objects in the format of
  10.6.106 and 11.0.89 (with `channel2gLockedTo6`, `dtimPeriod2gLockedTo3`,
  unknown extra fields). Cases: apply, 401 and 403 on PUT, 400 on PUT,
  missing allowlist fields, full replacement keeps all other fields, masked
  or missing passphrase stops the write, read-back mismatch, rollback,
  foreign change, request methods and paths. The existing GET-only test of
  `lib.rs` stays. A new source test covers `write.rs`.
- `talkops-core`: pure tests for the mapping and the value checks;
  `#[sqlx::test]` for each new query (connection fields, change rows,
  state changes, active-survey check).
- `talkops-api`: role tests (`unifi.write`, operator denied), audit tests
  (entries without passphrase), all gates of the table above, the
  unchanged behavior without the write switch.
- Web: `npm run lint`, `npm run check`, `npm test` (proposal and diff
  helpers), `npm run build`; browser check at 390 px and desktop.

## Documentation

- `docs/en/wifi-survey.md` and `docs/de/wlan-begehung.md`: new section
  "Apply recommendations to UniFi": API key rights, risks (client
  disconnect, foreign changes, no ETag), the switch, the diff dialog, the
  rollback, and the version table with a column "Write (stage 4a)".
- ADR 0042 accepted; ADR README row; ADR 0041 "Follow-up" links to 0042.
- Task summary in `docs/ai-memory/`.

## Delivery

One PR on the session branch, with one commit for each part (ADR and
proposal, crate, core and migration, API, web, docs). Proposed release
note: "Wi-Fi survey: optionally apply recommendations to the UniFi voice
SSID through the official UniFi Network API, with a diff dialog, a
snapshot, an audit entry and a rollback. Write access is off by default
and needs UniFi Network 10.6.106 or later."

## Stage 0 on the real 11.0 console (open points)

The user must check these points with hardware. TalkOps marks the feature
as "not tested with hardware" until then.

1. Run `GET /v1/sites/{siteId}/wifi/broadcasts/{id}` for the voice SSID.
   Share the answer anonymized: replace the passphrase, pre-shared keys
   and names. Confirm the field names of the allowlist. Confirm whether the
   passphrase comes back in clear text, masked or not at all.
2. Confirm whether the PUT accepts a body with `id` and `metadata`, or only
   without them.
3. Confirm which rights the API key needs for the PUT (role of the key
   owner, for example "Site Admin" or "Full Management"). Note the answer
   to a PUT with a read-only key (401 or 403).
4. Measure the client disconnect time when TalkOps applies a change: a
   handset in a call, time from the PUT until the voice returns.
5. Check that a PUT that changes only `uapsdEnabled` keeps the passphrase,
   PPSK entries, the network (VLAN) and the schedules.
6. Note the UniFi OS version and the Network version for the version table.

## Open questions for the user

- **Q1 802.11r (`fastRoamingEnabled`)**: the API has this field for
  WPA2, WPA2/WPA3 and WPA3 personal. `wpa3FastRoamingEnabled` needs it.
  Recommendation: keep it out of the allowlist as specified. TalkOps
  proposes `wpa3FastRoamingEnabled` only when 802.11r is already on, and
  shows 802.11r as an instruction. Alternative: add `fastRoamingEnabled`
  to the allowlist for WPA2/WPA3 and WPA3 SSIDs only.
- **Q2 SSID with 2.4 GHz only devices**: the proposal for `band_5ghz`
  removes 2.4 GHz from the voice SSID. Recommendation: propose it, but
  clear the check box by default and show a warning.
- **Q3 DTIM target values**: recommendation 1 for 5 and 6 GHz, 1 for
  2.4 GHz (3 if locked). Alternative: 2 or 3 for better battery life.
  The handset documentation should decide; stage 0 can confirm.
- **Q4 Handoff threshold**: recommendation: use the survey threshold
  `sticky_rssi` (default -75 dBm), limited to -80…-60 dBm.
- **Q5 Proposal without write access**: recommendation: show the proposal
  read only also when the write switch is off, so the administrator can
  apply it by hand.
- **Q6 History retention**: recommendation: keep change rows without a time
  limit (configuration history), independent of the survey retention.

## References

- [ADR 0041](adr/0041-wifi-voice-survey-and-unifi-correlation.md),
  [ADR 0008](adr/0008-secrets-at-rest.md), [ADR 0029](adr/0029-roles-and-permissions.md)
- UniFi Network API (developer.ui.com), versions 10.6.106 and 11.0.89
- Community client and OpenAPI copies:
  <https://github.com/Art-of-WiFi/unifi-network-application-api-client>
- Recommendation IDs: `crates/talkops-core/src/wifi_survey/analysis.rs`
