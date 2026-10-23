# Wi-Fi survey

[Handbook](index.md) · [Deutsch](../de/wlan-begehung.md)

A Wi-Fi survey checks voice quality and roaming of a Wi-Fi handset on the
whole site. An administrator walks through the site with the handset while
TalkOps keeps a test call open. TalkOps measures the voice stream each
second, records the call and reads the Wi-Fi state of the handset from
UniFi Network. After the call, TalkOps rates each roam and gives
recommendations.

The decision records are [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md)
(survey, read access), [ADR 0042](../adr/0042-unifi-voice-ssid-write-access.md)
(changes to the voice SSID) and [ADR 0043](../adr/0043-wifi-handset-band-and-roaming.md)
(handset roaming threshold, band protection).

## Requirements

- An extension with a device of kind **Wi-Fi handset** (for example Yealink
  AX83H or AX86R).
- For access point data: the device is an account on a provisioned phone
  (TalkOps uses the phone MAC address), and a UniFi connection (see below).
  Without UniFi, the survey measures the voice stream only.
- Permissions under **Settings → Users → Roles**:

| Permission | Function |
| --- | --- |
| `wifi_survey.run` | Start surveys and read own surveys. |
| `wifi_survey.manage` | Read all surveys, change thresholds and retention, delete surveys. |
| `unifi.manage` | Store and test the UniFi connection, select the voice SSID, read proposals and the change history (sensitive). |
| `unifi.write` | Switch on write access, apply changes to the voice SSID and roll them back (sensitive). With `phones.manage` also the roaming threshold of the survey handset. |

The admin role has all permissions. Other roles get none by default.

## Prepare the handset

1. Switch off VAD / silence suppression on the handset. With VAD, silence
   looks like packet loss. The result page shows a warning if TalkOps
   detects VAD.
2. Charge the handset.
3. Make sure that roaming is on and the handset uses the voice SSID.
4. Recommended: set DSCP EF (46) for RTP on the handset.

## Connect UniFi

1. In UniFi Network, open **Settings → Control Plane → Integrations** and
   create an API key.
2. In TalkOps, open **Settings → Wi-Fi survey → UniFi**.
3. Enter the console address, for example `https://192.168.1.1`, and the
   site name (usually `default`).
4. Enter the API key.
5. If the console has a self-signed certificate, select **Read
   certificate**. Compare the fingerprint with the certificate of the
   console. Then save. TalkOps then accepts only this certificate.
6. Select **Test connection**.

TalkOps stores the API key encrypted. TalkOps never shows the key again and
never writes it to a log or the audit log. Without **Allow write access**
(see [Apply recommendations to UniFi](#apply-recommendations-to-unifi)),
TalkOps sends only read requests to UniFi and never changes the UniFi
configuration.

The connection test shows which endpoints answered:

| Endpoint | Source | Use |
| --- | --- | --- |
| `/proxy/network/integration/v1/info` | Integration API | Network version |
| `/proxy/network/integration/v1/sites` | Integration API | Site ID |
| `/proxy/network/integration/v1/sites/{id}/devices` | Integration API | Access point names (fallback) |
| `/proxy/network/api/s/{site}/stat/sta/{mac}` | Classic API | Access point, band, channel, signal, data rate |
| `/proxy/network/api/s/{site}/stat/device-basic` | Classic API | Access point names |
| `/proxy/network/integration/v1/sites/{id}/wifi/broadcasts` | Integration API | SSID list, voice SSID |

The Integration API does not publish signal, channel or data rate of a
client. TalkOps therefore reads them from the classic endpoint `stat/sta`.
If this endpoint does not answer, the survey has no signal values.

### UniFi Network versions

| Network version | Integration API | `stat/sta` with API key | Write to voice SSID | State |
| --- | --- | --- | --- | --- |
| 10.3.58 (reference) | Published | Not published | No (read only) | From documentation, not tested with hardware |
| Below 10.6.106 | — | — | No (read only) | — |
| 10.6.106 | Published | — | Yes | From the published API specification, not tested with hardware |
| 11.0.89 (target) | Published | — | Yes | From the published API specification, not tested with hardware |
| Other versions | — | — | — | Not tested |

UniFi Network 11.0.x needs UniFi OS 6.0 or later.

Add a row when you test a version. The connection test shows the version
and the available endpoints.

## Start a survey

1. Open **Settings → Wi-Fi survey**, or select **Wi-Fi survey** on the
   device row of an extension.
2. Select **Start survey**.
3. Select the Wi-Fi device. The dialog shows the MAC address and the
   current UniFi state of the handset.
4. Select the start mode:
   - **TalkOps calls the handset now**: only this device rings, not the
     other devices of the extension.
   - **I dial \*39 from the handset**: dial `*39` within 10 minutes. A
     `*39` call without a prepared survey plays a short notice and ends.
5. Optional: enter a label, for example "before AP move".
6. Confirm the checklist and select **Start**.

## During the survey

The handset plays one short tone each second. Every 30 seconds a voice says
"Measurement running". A missing tone is a drop that you can hear.

Keys on the handset:

| Key | Function |
| --- | --- |
| `1` | Marker "disturbance": you heard a drop. |
| `2` | Marker "new location". |
| `3` | Marker "note" (add the text later). |
| `#` | End the test. |

The live dashboard shows all values on one phone screen:

- traffic light of the last 5 seconds, elapsed time, **Stop**
- access point, band and channel, signal (RSSI), data rate
- packets of the last second, uplink loss and longest gap of the last
  10 seconds
- downlink loss and jitter from the latest RTCP report of the handset
- estimated MOS, number of roams and the last roam with its rating
- buttons for markers and notes
- a chart of the last 2 minutes and a list of events

The page keeps the screen on if the browser supports it. If the page loses
the connection (dead zone), the measurement continues on the server. The
page loads the missing values when the connection comes back.

The test ends when you hang up, press `#`, select **Stop**, or the maximum
duration (default 60 minutes) ends.

## Result

The media worker analyses the survey after the call. The result page shows:

- a summary: duration, roams by rating, worst gap, loss, estimated MOS,
  signal, markers, codec
- recommendations, each with the measured values
- a timeline: access point sections, signal, loss for each second, loss
  periods, roams and markers. Click the timeline to play the recording from
  3 seconds before that point.
- the roam table with **Play** for each roam
- detected patterns, time on each access point and the markers with
  editable notes
- CSV and JSON export

The recording is stereo. The left channel is the handset microphone. The
right channel is the test signal from TalkOps.

### Rating of a roam

TalkOps detects a roam when the access point of the handset changes between
two UniFi samples. A change of band or channel at the same access point is
a band change. The window of a roam starts 3 seconds before the last sample
on the old access point and ends 3 seconds after the first sample on the new
one.

| Rating | Rule (defaults) |
| --- | --- |
| Clean | Longest gap below 50 ms |
| Audible | Longest gap from 50 to 150 ms |
| Bad | Longest gap above 150 ms, or more than 5 % loss in the window (loss block) |

The gap is the number of missing uplink packets in consecutive seconds,
multiplied by the packet time (usually 20 ms).

### Patterns

| Pattern | Rule (defaults) |
| --- | --- |
| Sticky client | Signal below -75 dBm for 10 s on one access point, then a roam or loss |
| Ping-pong | 3 roams between the same two access points within 60 s |
| Drop without roam | A gap of 50 ms or more with no roam within ±3 s |
| Roam without drop | A roam with a clean rating |
| Weak coverage | Signal below -80 dBm for 5 s |

UniFi does not report the signal of other access points for a client.
"Sticky client" therefore shows that a better access point was probably
available. It cannot prove it.

Change the thresholds under **Settings → Wi-Fi survey → Settings**. New
thresholds apply to new surveys. **Analyse again** on a survey applies them
to an old survey.

### Compare two surveys

Select two analysed surveys in the list and select **Compare**. The page
shows the summaries side by side and the roams for each pair of access
points.

## Apply recommendations to UniFi

TalkOps can apply some recommendations to the voice SSID in UniFi Network.
This function is off by default. Without it, TalkOps works as a read-only
tool.

### Rules

- TalkOps changes only the SSID that you select as the voice SSID. The
  SSID must be a standard SSID that you created in this console. TalkOps
  does not change IoT SSIDs or SSIDs that a central management (Site
  Manager) controls.
- TalkOps changes only these settings: BSS transition (802.11v), WPA3 fast
  roaming, protected management frames (PMF), handoff suggestions (minimum
  RSSI), bands, U-APSD, multicast to unicast, ARP proxy and the DTIM period
  for each band.
- TalkOps writes only through the official UniFi Network API with
  `PUT /proxy/network/integration/v1/sites/{siteId}/wifi/broadcasts/{id}`.
  TalkOps sends no write request to the classic API. TalkOps does not
  change access point radio settings.
- TalkOps writes only after you confirm the values in the diff dialog.
- TalkOps does not write while a survey is armed, calling or running.
- UniFi Network 10.6.106 or later is necessary. With an older version,
  TalkOps stays read only.
- Before each change, TalkOps stores the values of the changed settings
  (snapshot). A rollback restores exactly these settings.
- TalkOps never stores, logs or shows the Wi-Fi password of the SSID.

### Risks

- **Clients disconnect for a short time.** UniFi applies the SSID again on
  all access points. All clients of the SSID disconnect and connect again.
  A call on this SSID can drop. Apply changes outside working hours.
- **Fast roaming compatibility.** Some older clients cannot connect when
  802.11r or WPA3 fast roaming is on. Check the handset documentation.
- **Only 5 GHz.** If you remove 2.4 GHz, clients that support only 2.4 GHz
  lose the SSID. The dialog does not select this row by default.
- **Changes by other persons.** The UniFi API has no version token. TalkOps
  reads the SSID again directly before each change and refuses the change
  if the settings changed since the proposal.
- **Full update.** The API replaces the whole SSID object. TalkOps reads
  the SSID immediately before the change and sends all other settings back
  unchanged, including the password. If UniFi does not return the
  password, TalkOps does not write.

### API key rights

The connection test does not write. The first change shows if the API key
may write. A key that may only read causes the error "The UniFi API key may
read but not write". Then create the key with an administrator account that
may change the Wi-Fi settings of the site.

### Switch on write access

1. Open **Settings → Wi-Fi survey → UniFi**.
2. Select **Test connection**. The state shows **Writes by version:
   possible** for Network 10.6.106 or later.
3. Under **Voice SSID and write access**, select **Read SSIDs**.
4. Select the SSID of the Wi-Fi handsets.
5. Select **Allow write access**. You need the permission `unifi.write`.
6. Select **Save**.

A change of the console address or the site name clears the voice SSID and
switches write access off.

### Propose and apply a change

1. Open the result of an analysed survey.
2. In **Recommendations**, select **Propose change**. TalkOps reads the
   voice SSID. Nothing changes yet.
3. The dialog shows each setting with the current value and the new value.
   Clear the rows that you do not want.
4. Read the warnings. The dialog also lists recommendations that TalkOps
   does not write, for example 802.11r, 802.11k, the minimum data rate and
   the transmit power.
5. Select **I understand that clients of this SSID disconnect for a short
   time**.
6. Select **Apply**.
7. Select **Start a new survey for comparison** and compare the two
   surveys.

| Recommendation | Proposed change |
| --- | --- |
| Slow roams (`fast_roaming`) | BSS transition on; WPA3 fast roaming on if 802.11r is on |
| Sticky client (`min_rssi`) | Handoff suggestion for 5 GHz at the survey threshold (default -75 dBm, limits -80 to -60 dBm) |
| 2.4 GHz (`band_5ghz`) | Remove 2.4 GHz (not selected by default) |
| Voice SSID (`voice_ssid`) | U-APSD on, multicast to unicast on, ARP proxy on, DTIM 1 (2.4 GHz: 3 if UniFi locks it), PMF optional if it is off |

### Change history and rollback

**Settings → Wi-Fi survey → Wi-Fi changes** lists each change with the time,
the user, the settings and the state. Select the time to see the values
before and after the change.

1. Select **Roll back** on a change.
2. Check the values that TalkOps restores.
3. Select **Roll back now**.

If another person changed one of these settings in UniFi after the TalkOps
change, the dialog shows the values after TalkOps, the current values and
the values that the rollback writes. Select **Roll back anyway** only if
the rollback must overwrite these changes.

The audit log records each switch of write access, each change and each
rollback with the settings and values, never with the password.

## Handset roaming threshold and band protection

The handset also decides when it roams. TalkOps provisions the roaming
threshold of Yealink AX83H and AX86R handsets and protects the handsets
against a band change on the voice SSID. TalkOps does not provision the
Wi-Fi name or password of the handsets.

### Roaming threshold

When the signal of the current access point drops below the roaming
threshold, the handset searches for a better access point. Parameter:
`static.network.wifi.roaming_threshold`, -100 to -30 dBm, factory value
-70 dBm (Yealink AX83H Wi-Fi Roaming Guide and Administrator's Manual).

- Set it for all Wi-Fi handsets under **Settings → Phones (provisioning) →
  Comfort settings → Wi-Fi**, or for one handset on its phone page.
- From a survey: if the survey found a sticky client (`min_rssi`), the diff
  dialog proposes the survey threshold plus 5 dB, limited to -75 to -60 dBm
  (default survey: -80 → -75). The handset then searches before the access
  point pushes it away.

To apply the proposal:

1. Open the result of the survey and select **Propose change**.
2. In the section **Handset**, check the current value and the new value.
3. Confirm and select **Apply**. You need `unifi.write` and `phones.manage`.

TalkOps stores the value for this handset only and asks the handset to fetch
its configuration (`check-sync`). The change history shows **waiting for
handset** until the handset fetches it. A handset that is not reachable gets
the value at its next provisioning interval. The rollback restores the
earlier value. If someone changed the value since, the rollback asks again.

### Band protection

Before TalkOps removes a band from the voice SSID (for example 2.4 GHz),
TalkOps reads the band of each provisioned Wi-Fi handset from UniFi:

| UniFi reports | Result |
| --- | --- |
| Connected on a band that stays | OK |
| Connected to the voice SSID on the band that the change removes | Blocked: the handset would lose the SSID |
| Not connected | You must confirm the handset by name |

A rollback that removes a band uses the same check.

TalkOps does not provision the band setting of the handset yet. Yealink
does not document its parameter. A configuration export shows the name
`static.network.wifi.band.mode`, but the meaning of its values is not
confirmed. Keep the handset at **Wi-Fi Band:
2.4G & 5G** (the handset then prefers 5 GHz). A handset that uses only one
band goes offline if the voice SSID stops this band, and TalkOps cannot
reach it to correct it.

## Limits of the measurement

- TalkOps uses the statistics of FreeSWITCH. The uplink values have a
  resolution of one second. FreeSWITCH gives no jitter value for each
  second; the jitter values come from the RTCP reports of the handset.
- The MOS is the estimate of FreeSWITCH from lost and late packets. It is
  not an ITU-T measurement.
- Downlink values depend on how often the handset sends RTCP reports.
- UniFi values have the precision of the poll interval (about one second).

## Privacy and retention

- During an active survey, TalkOps reads Wi-Fi data for the MAC address of
  the survey device. When a proposed SSID change removes a band, TalkOps also
  reads the band of each provisioned Wi-Fi handset for the protection check.
  This check also runs before apply and rollback, outside an active survey.
- Location data is only the "new location" marker and the optional notes.
  Do not write personal data into notes.
- The survey always records the call. The tester is normally the
  administrator.
- TalkOps deletes the survey, its values and its recording after the
  retention period (default 30 days, **Settings → Wi-Fi survey →
  Settings**).
- Each start, stop, marker, export, setting change and UniFi change writes
  an audit entry. Audit entries contain no API key and no note text.
