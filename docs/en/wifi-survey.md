# Wi-Fi survey

[Handbook](index.md) · [Deutsch](../de/wlan-begehung.md)

A Wi-Fi survey checks voice quality and roaming of a Wi-Fi handset on the
whole site. An administrator walks through the site with the handset while
TalkOps keeps a test call open. TalkOps measures the voice stream each
second, records the call and reads the Wi-Fi state of the handset from
UniFi Network. After the call, TalkOps rates each roam and gives
recommendations.

The decision record is [ADR 0041](../adr/0041-wifi-voice-survey-and-unifi-correlation.md).

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
| `unifi.manage` | Store and test the UniFi connection (sensitive). |

The admin role has all permissions. Other roles get none by default.

## Prepare the handset

1. Switch off VAD / silence suppression on the handset. With VAD, silence
   looks like packet loss. The result page shows a warning if TalkOps
   detects VAD.
2. Charge the handset.
3. Make sure that roaming is on and the handset uses the voice SSID.
4. Recommended: set DSCP EF (46) for RTP on the handset.

## Connect UniFi (read only)

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
never writes it to a log or the audit log. TalkOps sends only read
requests to UniFi. TalkOps never changes the UniFi configuration.

The connection test shows which endpoints answered:

| Endpoint | Source | Use |
| --- | --- | --- |
| `/proxy/network/integration/v1/info` | Integration API | Network version |
| `/proxy/network/integration/v1/sites` | Integration API | Site ID |
| `/proxy/network/integration/v1/sites/{id}/devices` | Integration API | Access point names (fallback) |
| `/proxy/network/api/s/{site}/stat/sta/{mac}` | Classic API | Access point, band, channel, signal, data rate |
| `/proxy/network/api/s/{site}/stat/device-basic` | Classic API | Access point names |

The Integration API does not publish signal, channel or data rate of a
client. TalkOps therefore reads them from the classic endpoint `stat/sta`.
If this endpoint does not answer, the survey has no signal values.

### UniFi Network versions

| Network version | Integration API | `stat/sta` with API key | State |
| --- | --- | --- | --- |
| 10.3.58 (reference) | Published | Not published | From documentation, not tested with hardware |
| Other versions | — | — | Not tested |

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

## Limits of the measurement

- TalkOps uses the statistics of FreeSWITCH. The uplink values have a
  resolution of one second. FreeSWITCH gives no jitter value for each
  second; the jitter values come from the RTCP reports of the handset.
- The MOS is the estimate of FreeSWITCH from lost and late packets. It is
  not an ITU-T measurement.
- Downlink values depend on how often the handset sends RTCP reports.
- UniFi values have the precision of the poll interval (about one second).

## Privacy and retention

- TalkOps reads Wi-Fi data only during an active survey and only for the
  MAC address of the survey device.
- Location data is only the "new location" marker and the optional notes.
  Do not write personal data into notes.
- The survey always records the call. The tester is normally the
  administrator.
- TalkOps deletes the survey, its values and its recording after the
  retention period (default 30 days, **Settings → Wi-Fi survey →
  Settings**).
- Each start, stop, marker, export, setting change and UniFi change writes
  an audit entry. Audit entries contain no API key and no note text.
