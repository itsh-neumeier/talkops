# Wi-Fi survey stage 4b: handset roaming threshold and band protection: proposal for user review

Status: Accepted by the user on 2026-10-23 ("ein PR auf Basis 62"), with
the recommended answers to Q1 to Q4 and Q5 = one PR on top of PR #62.
Implemented in the same PR.
Date: 2026-10-23 (Europe/Berlin).
Base: branch of stage 4a (PR #62, commit `c053920`). Stage 4b needs the
diff dialog and the change history of stage 4a.
ADR draft: [ADR 0043](adr/0043-wifi-handset-band-and-roaming.md).
Scope set by the user: only the band and the roaming setting of the
handsets. No Wi-Fi credentials (SSID, password).

## Implementation notes

- Migration `20261108000002_wifi_handset_changes.sql`.
- Apply: `POST /api/v1/wifi-handset-changes`. Rollback of a handset row
  uses the existing route `POST /api/v1/unifi/wifi-changes/{id}/rollback`.
- The handset change does not need the UniFi write switch (it changes only
  TalkOps data), but it needs `unifi.write` and `phones.manage`.
- The survey gate is tenant-wide (any armed, calling or running survey),
  like the SSID gate.
- The phone settings UI shows the Wi-Fi group under **Phones
  (provisioning) → Comfort settings** and on the phone page.

## Goal

The survey result shows slow roams and sticky clients. Stage 4a changes the
access point side (UniFi voice SSID). Stage 4b adds the handset side:

1. The diff dialog also proposes the roaming threshold of the survey
   handset. TalkOps provisions it after the same confirmation.
2. TalkOps prevents a band change on the voice SSID that would make a Wi-Fi
   handset unreachable.

## Facts from the Yealink documentation

| Item | Source | Result |
| --- | --- | --- |
| Roaming threshold | AX83H Wi-Fi Roaming Guide; AX83H Administrator's Manual (Wi-Fi section) | `static.network.wifi.roaming_threshold`, integer -100 to -30, default -70. Web UI: **Network > Wi-Fi > Roaming Signal Threshold**. |
| 5 GHz preference | AX83H Wi-Fi Roaming Guide | "Prioritize 5GHz connection when operating in dual-band mode." |
| Band parameter | AX83H Administrator's Manual | Not documented. The handset menu has "Wi-Fi Band (2.4G & 5G)". |
| `roam_lossrate_threshold`, `roam_aggressive.enable`, `wireless.channel_enable` | Yealink Wi-Fi Roaming Guide for MP66W | Documented only for the MP66W. Not used for AX handsets. |
| Recommended signal at the handset | Yealink AX83H access point guidance | -67 dBm or better from the access point. |

Project rule: provisioning parameters come from official documentation.
The band parameter therefore stays out of the catalog until stage 0
confirms it (see open points).

## Part 1: roaming threshold

### Catalog

New setting in `presets/phones/yealink.yaml`:

```yaml
- { key: wifi_roaming_threshold, group: wifi, param: static.network.wifi.roaming_threshold,
    kind: int, min: -100, max: -30, default: "-70", families: [wifi] }
```

- New `SettingKind::Int` with `min` and `max` in `talkops-provisioning`.
  The phone settings UI shows a number field.
- The value works like other comfort settings: tenant-wide default and an
  override for each phone. TalkOps writes the parameter only when it is set.
- New UI group `wifi` ("Wi-Fi") in the phone settings.

### Proposal from the survey

The diff dialog of stage 4a gets a second section "Handset". It appears when
the survey has a provisioned phone (`phone_id`) of the `wifi` family.

| Recommendation | Proposed handset change |
| --- | --- |
| `min_rssi` (sticky client) | Roaming threshold = survey threshold `sticky_rssi` + 5 dB, limited to -75…-60 dBm (default survey: -75 → -70) |
| `fast_roaming` with a sticky pattern | same as above |

Reason for "+5 dB": the handset starts to scan before the access point
(UniFi handoff suggestion at `sticky_rssi`, stage 4a) pushes it away. The
handset decides first; the access point is the fallback.

If the computed value equals the current value (for example -70 on a new
handset), the dialog shows no row.

### Apply

1. The administrator confirms the dialog (same check box as stage 4a).
2. TalkOps stores the value as the setting of this phone (not tenant-wide).
3. TalkOps sends `check-sync` (`reboot=false`) to the device.
4. The history row shows "waiting for handset" until `phones.last_seen_at`
   is newer than the change. Then it shows "applied".

Gates:

- Permission `phones.manage` and `unifi.write`? See Q2.
- No change while a survey of this handset is armed, calling or running.
- The SSID part and the handset part are separate requests. A failure of
  one part does not undo the other part. The history shows both rows.

### History and rollback

Stage 4a stores changes in `unifi_wifi_changes`. Stage 4b adds a new
migration (stage 4a is not released, but a new file keeps the IDs stable):

- `target TEXT NOT NULL DEFAULT 'ssid' CHECK (target IN ('ssid', 'handset'))`
- `phone_id UUID REFERENCES phones ON DELETE SET NULL`
- `wifi_id` and `site_id` become nullable for `handset` rows.
- State `waiting` for a handset change that the handset has not fetched yet.

A rollback of a handset row restores the earlier value (or removes the
setting if it was not set) and sends `check-sync` again. A foreign change
(someone changed the phone setting in TalkOps since) asks again, like the
SSID rollback.

## Part 2: band protection

Stage 4a can remove 2.4 GHz from the voice SSID (`band_5ghz`, not selected
by default). Stage 4b adds a check before this change.

1. TalkOps lists the Wi-Fi handsets of the tenant (`DeviceKind::Wifi` with a
   provisioned phone and a MAC address).
2. TalkOps reads each MAC with `stat/sta` (classic API, GET only, already
   used by the survey).
3. Result for each handset:

| UniFi reports | Result |
| --- | --- |
| Connected on 5 GHz (`na`) | OK |
| Connected on 2.4 GHz (`ng`) to the voice SSID | Blocked: the handset would lose the SSID |
| Not connected / unknown | Warning: "not reachable, band unknown" |

4. The diff dialog lists blocked and unknown handsets in the band row.
   A blocked handset disables the row. An unknown handset needs a second
   check box that names the handset ("I checked that AX83H 300 supports
   5 GHz").
5. The apply request repeats the check on the server.

The same check applies to a rollback that removes a band.

Band provisioning later: when stage 0 confirms the band parameter, a small
follow-up adds the catalog setting `wifi_band` with the rule:

| Voice SSID | Allowed handset band | Default |
| --- | --- | --- |
| 2.4 + 5 GHz | both, 5 GHz only, 2.4 GHz only | both |
| 2.4 GHz only | both, 2.4 GHz only | both |
| 5 GHz only | both, 5 GHz only | both |

TalkOps then refuses a handset band that the voice SSID does not broadcast,
and refuses an SSID band change that leaves a handset without a common band.

## Data, API and UI

- Migration (ID chosen at implementation, after `20261108000001`):
  history columns of part 1.
- `talkops-provisioning`: `SettingKind::Int`, catalog entry, renderer test
  for `static.network.wifi.roaming_threshold`.
- API:
  - `GET /api/v1/wifi-surveys/{id}/unifi-proposal` gets a `handset` section
    (current value, proposed value, phone, write state) and, for the band
    row, a `handsets` list with the check result.
  - `POST /api/v1/wifi-handset-changes` (apply) and
    `POST /api/v1/unifi/wifi-changes/{id}/rollback` (also for handset rows).
  - Audit actions `wifi.handset.apply`, `wifi.handset.rollback`.
- UI: section "Handset" in the diff dialog; band check result in the band
  row; history shows the target (SSID or handset name); phone settings
  show the new Wi-Fi group. Texts in `en.ts` and `de.ts`.

## Tests

- Provisioning: catalog loading with `int`, limits, rendered config line for
  AX83H and AX86R, no line when unset.
- Core: `#[sqlx::test]` for the new history columns and queries.
- API: proposal with and without phone, apply with `check-sync` (mock),
  `waiting` → `applied` after a config fetch, rollback, foreign change,
  active survey gate, roles, audit; band check with mock `stat/sta`
  answers (5 GHz, 2.4 GHz, unknown), server-side repeat of the check.
- Web: lint, check, unit tests for the new helpers, build, browser check at
  390 px and desktop.
- Docs: user guides (EN/DE) section "Handset roaming threshold" and "Band
  protection", ADR 0043, task summary.

## Stage 0 on real hardware (open points)

1. In the web UI of an AX83H, set **Wi-Fi Band** to each value, export the
   configuration (**Settings > Configuration > Export**), and send the lines
   that change (without passwords). This confirms the band parameter.
2. Check if `static.network.wifi.roaming_threshold` takes effect after
   `check-sync` without a reboot.
3. Note the firmware version of the handset.

## Open questions for the user

- **Q1 Proposed threshold**: `sticky_rssi` + 5 dB, limited to -75…-60 dBm
  (recommended). Alternative: always -67 dBm (Yealink guidance).
- **Q2 Permission**: the handset change needs `unifi.write` and
  `phones.manage` (recommended: both, because it is part of the same
  flow). Alternative: `phones.manage` only.
- **Q3 Scope of the threshold**: only the survey handset (recommended).
  Alternative: tenant-wide for all Wi-Fi handsets, with a second check box.
- **Q4 Offline handsets in the band check**: allow with a named second
  confirmation (recommended), or always block.
- **Q5 Delivery**: one PR on top of PR #62 (recommended, because it extends
  its dialog and history), or wait until PR #62 is merged.

## References

- [ADR 0041](adr/0041-wifi-voice-survey-and-unifi-correlation.md),
  [ADR 0042](adr/0042-unifi-voice-ssid-write-access.md),
  [stage 4a proposal](unifi-wifi-write-proposal.md)
- Yealink AX83H Wi-Fi Roaming Guide (support.yealink.com, AX83H documents)
- Yealink AX83H Administrator's Manual, Wi-Fi section
- Yealink Wi-Fi Roaming Guide for MP66W (MP66W parameters, not used here)
