# 0027 - Media settings per device and door call ringing

- Status: Accepted
- Date: 2026-10-15
- Amends: [ADR 0023](0023-device-codec-profiles.md),
  [ADR 0025](0025-video-policy.md)

## Context

Tests with a Dahua VTO4202F-P-S2 (V4.600.0.3), two VTH5422H (V4.800.0.2)
and the browser softphone showed these problems:

1. A browser lists VP8 before H264 in its offer. FreeSWITCH answered the
   browser with VP8. A Dahua VTH accepts H264 only. FreeSWITCH relays H264
   (`mod_h26x`) but cannot encode H264. Thus the VTH showed no picture on
   calls from the browser.
2. A VTH answers a call from its own VTO number at once with
   `180 Ringing` and SDP (early media). FreeSWITCH then selects the VTH as
   the winner of the parallel ringing and cancels all other targets (for
   example the browser of the same extension).
3. A call from the VTO to the VTH has audio, but the VTH shows no picture.
   The SIP signalling and the codecs are correct (H264 on both legs). The
   Dahua documentation and the Yeastar integration guide describe that the
   VTH gets the VTO picture directly from the VTO (main VTO with IP
   address, user name and password in the VTH). The cause is not clear.
   The administrator must be able to test different settings per device.
4. ADR 0023 and ADR 0025 had a fixed rule: a device with the codec profile
   `hd` or `robust` gets no video. No setting could give a device video with
   other audio codecs, or video with one codec only.

## Options

1. Detect Dahua devices (user agent, device kind) and use fixed rules for
   them.
2. Settings per device: the audio codec profile and the video codecs. Fixed
   rules only where a rule is always correct.

## Decision

TalkOps uses option 2. Detection by user agent is not reliable, and the
correct settings for Dahua are not known yet.

- New column `devices.video` (`device_video`: `auto`, `off`, `h264`,
  `vp8`). `NULL` means automatic: `auto` with the codec profile
  `standard`, `off` with all other profiles. This keeps the behaviour of
  existing devices.
- The video policy of the call (ADR 0025) still decides if a call can have
  video. The video setting of a device decides which video codecs its leg
  gets on such a call. `off` gives an explicit audio-only codec list.
- A device with a codec profile and a video setting other than `off` gets
  the audio codecs of the profile and the selected video codecs.
- The directory gives the variable `talkops_video` for calls from a device.
  The dialplan applies the setting to the caller's leg. Without the
  variable, a device with a codec profile has no video (as before).
- New codec profile `g711`: PCMA and PCMU, no jitter buffer.
- On calls with video, the caller's leg has `inherit_codec=true`. When the
  called device answers or sends early media, the caller's leg uses the
  same audio and video codec. Thus a browser gets H264 when the called
  device has H264 only. A caller with a codec profile keeps its profile.
- Door station calls set `ignore_early_media=true` and run `ring_ready`
  before the bridge. All targets ring until one target answers. The
  station gets `180 Ringing`. A preview picture through SIP early media is
  not possible with parallel ringing in FreeSWITCH (`bridge_early_media`
  relays audio of one target only).
- The diagnostics capture adds one line per call leg at hangup with the
  audio and video RTP packet counters. This shows if video arrives and if
  TalkOps sends it on.

## Consequences

- Positive: the administrator can test audio and video settings per
  device without a new release.
- Positive: a browser and an H264-only device use H264 without
  transcoding.
- Positive: a door call rings all targets.
- Negative: with `inherit_codec`, a browser that calls a desk phone uses the
  audio codec of the desk phone (for example G.722) instead of Opus. This
  removes transcoding.
- Negative: no SIP early media on door calls. A device that shows a preview
  only from early media shows the picture when the call is answered.
- Follow-up: the cause of problem 3 needs a hardware test with the new
  settings and the RTP counters.
