# Codec profiles and video per device

A codec profile sets the audio codec that TalkOps uses with one device.
TalkOps (FreeSWITCH) converts the audio for the other party. The other party
keeps its codec. Use a profile for devices in a Wi-Fi network with packet
loss or jitter.

Decision records: [ADR 0023](../adr/0023-device-codec-profiles.md),
[ADR 0028](../adr/0028-device-media-settings.md).

## Profiles

| Profile | Codecs on the device leg | Use |
|---|---|---|
| Standard | System codecs: Opus, G.722, G.711 (video where allowed) | Desk phones, DECT, softphones |
| HD (G.722) | G.722, fallback G.711, jitter buffer | Wi-Fi handsets in a good network |
| Robust (iLBC) | iLBC (20 ms, then 30 ms), fallback G.711, jitter buffer | Devices in a network with packet loss |
| G.711 only | PCMA, PCMU, no jitter buffer | Devices that work well only with G.711 (for example door stations) |
| Automatic | Wi-Fi handsets: HD. All other devices: Standard | Default |

Effects in a call:

- **HD** gives wideband audio. With packet loss, you hear short gaps.
- **Robust** has narrowband audio (as a usual telephone). With packet loss
  of approximately 5 to 15 %, the speech stays clear. Each iLBC frame
  decodes on its own, so a lost packet does not damage the next packets.
- No codec helps when the network stops for a longer time (roaming gaps of
  more than approximately 200 ms, no Wi-Fi coverage).
- The jitter buffer on the TalkOps side smooths the audio from the device.
  It adds 60 to 200 ms of delay.
- A device with HD, Robust or G.711 only gets no video, unless you set
  **Video** for the device (see [Video per device](#video-per-device)).
- The codec does not change during a call. A profile change applies to the
  next call.

## Set the profile

1. Open **Extensions** and select the extension.
2. In **Devices**, click **Edit** for the device.
3. Select a **Codec profile**.
4. Click **Save**.

The device list shows "HD (G.722)" or "Robust (iLBC)" next to the device
kind when a device uses one of these profiles.

## Video per device

The setting **Video** selects the video codecs of a device on calls that
allow video. The [video policy](../adr/0025-video-policy.md) still decides if
a call can have video (for example: extension with video to extension with
video, door station to its target).

| Setting | Video codecs on the device leg |
|---|---|
| Automatic | Standard profile: H264 and VP8. All other profiles: off |
| H264 and VP8 | H264 and VP8 |
| H264 only | H264. Use it for devices without VP8 (for example Dahua VTH). |
| VP8 only | VP8 |
| Off | No video. The device leg gets audio codecs only. |

The setting applies to calls to the device and to calls from the device.
FreeSWITCH does not convert video. When the two devices of a call have no
common video codec, the call has no picture.

On a call with video, the caller's leg takes over the audio and video codec
of the device that answers (`inherit_codec`). Thus a browser that calls a
device with H264 only uses H264.

The device list shows "Video: …" when a device does not use H264 and VP8.

## Provisioned Yealink phones

For a device with HD, Robust or G.711 only, the phone configuration enables
these codecs on the account of the device:

| Codec | Yealink payload | Priority |
|---|---|---|
| G.722 | `g722` | 1 |
| iLBC | `ilbc` (AX83H), `ilbc_15_2kpbs` and `ilbc_13_33kpbs` (firmware with two iLBC rates) | 2, 3 |
| G.711 A-law | `pcma` | 4 |
| G.711 µ-law | `pcmu` | 5 |
| G.729, G.726, G.723.1 | `g729`, `g726_*`, `g723_*` | disabled |

The phone ignores payload names that its firmware does not know. TalkOps
selects the codec of each call, so a change between HD and Robust needs no
resync. After a change from or to Standard, click **Resync** on the phone
page. With Standard, TalkOps does not change the codec settings of the phone.

Sources: Yealink AX83H Administrator's Manual, section "Audio Codecs
Configuration" (`account.X.codec.<payload_type>.enable` and `.priority`), and
Yealink SIP-T2 Administrator's Manual V85, same section, for the two iLBC
rates.

## Wi-Fi network

These settings in the Wi-Fi network often help more than the codec:

- Enable WMM (Wi-Fi Multimedia) on the access points. Yealink phones mark
  the voice packets with DSCP 46 (EF) by default.
- Enable fast roaming (802.11r/k/v) when the handsets support it.
- Use the 5 GHz band for the handsets.

## Check the codec

**Settings → System → Active channels** shows the codec of each channel
(`read_codec` and `write_codec`). A call to a device with Robust shows
`iLBC` on the leg of the device and the codec of the other party on the
other leg.
