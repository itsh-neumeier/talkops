# 0043 - Wi-Fi handset roaming threshold and band protection

- Status: Accepted by the user on 2026-10-23 ("ein PR auf Basis 62"; open
  questions answered with the recommended options).
- Extends: ADR 0041 and ADR 0042. Also uses ADR 0029.
- Proposal: [Wi-Fi handset band and roaming proposal](../wifi-handset-roaming-proposal.md).

## Context

The Wi-Fi survey (ADR 0041) finds slow roams and sticky clients. ADR 0042
lets TalkOps change the UniFi voice SSID, for example the minimum RSSI and
the bands. The handset also decides when it roams. A UniFi change alone
does not change this decision.

TalkOps provisions Yealink AX83H and AX86R handsets. The user decided that
this stage provisions only the band and the roaming setting of the
handsets. Wi-Fi credentials (SSID, password) stay out of scope.

Official Yealink documentation has these facts:

- `static.network.wifi.roaming_threshold`: integer from -100 to -30,
  default -70. When the signal of the current access point drops below
  this value, the handset scans for a better access point and roams
  (AX83H Wi-Fi Roaming Guide, AX83H Administrator's Manual, web UI
  **Network > Wi-Fi > Roaming Signal Threshold**).
- The handset prefers 5 GHz in dual-band mode (AX83H Wi-Fi Roaming Guide).
- The Administrator's Manual of the AX83H lists no provisioning parameter
  for the Wi-Fi band. The handset menu has a "Wi-Fi Band" item. Other
  roaming parameters (`roam_lossrate_threshold`, `roam_aggressive.enable`)
  are documented only for the MP66W, not for the AX series.

The handset receives its provisioning through the Wi-Fi network. If the
voice SSID and the handset have no common band, the handset goes offline.
TalkOps cannot then correct the handset. A person must reset it on site.

## Options

1. Roaming threshold: (a) a provisioning setting of the AX series, with a
   proposal from the survey result; (b) instructions only.
2. Band of the handset: (a) provision a band parameter now; (b) provision a
   band parameter only after a hardware test confirms its name; (c) no band
   parameter.
3. Protection when UniFi removes a band: (a) no check; (b) check the band of
   each Wi-Fi handset that UniFi reports, before the SSID change.
4. Wi-Fi credentials: (a) provision them; (b) out of scope.

## Decision

- Use options 1a, 2b, 3b and 4b.
- Add the phone setting `wifi_roaming_threshold` for the `wifi` family of
  the Yealink catalog: parameter `static.network.wifi.roaming_threshold`,
  integer from -100 to -30, factory value -70. A new setting kind `int`
  with lower and upper limits holds it.
- The survey proposal (ADR 0042 diff dialog) gets a section for the handset
  of the survey. TalkOps proposes a roaming threshold from the survey
  result. The administrator confirms it in the same dialog. TalkOps then
  stores the setting and sends `check-sync` without a reboot.
- TalkOps does not provision a band parameter in this stage. A hardware
  test must first confirm the parameter name and its values. A later change
  adds it to the catalog with the rule: default "both bands"; "5 GHz only"
  only if the voice SSID broadcasts 5 GHz; never a band that the voice SSID
  does not broadcast.
- Band protection: before TalkOps removes a band from the voice SSID
  (ADR 0042), TalkOps reads the radio of each known Wi-Fi handset from UniFi
  (`stat/sta`, read only, ADR 0041). TalkOps refuses the change if a handset
  is connected on the band that the change removes, or if UniFi does not
  report a handset. The administrator can override an offline handset only
  with a second confirmation that names the handset.
- No handset change while a survey of this handset is armed, calling or
  running.
- Each handset change goes into the change history of ADR 0042 with the
  value before and after, an audit entry and a rollback. A rollback restores
  the earlier value (or removes the setting) and sends `check-sync` again.
- Wi-Fi credentials are not provisioned.

## Consequences

- Positive: the survey result can change both sides of a roam, the access
  point (ADR 0042) and the handset, with one confirmation flow.
- Positive: a band change on the SSID cannot silently make a connected
  handset unreachable.
- Negative: the band of the handset is not provisioned until a hardware test
  confirms the parameter. Until then the handset keeps its own band setting.
- Negative: the band check sees only handsets that UniFi reports. An offline
  handset needs a manual decision.
- Negative: `check-sync` reaches only registered handsets. A handset that
  misses it receives the value at its next provisioning interval.
- Implementation notes: the handset change is blocked while any survey of
  the tenant is armed, calling or running (the same gate as the SSID). The
  handset part needs `unifi.write` and `phones.manage`; it does not need the
  UniFi write switch, because it changes only TalkOps data. A handset row
  stays `waiting` until the phone fetches its configuration.
- Follow-up: hardware test (stage 0), the band parameter, and VAD/DSCP when
  their AX parameter names are confirmed.
