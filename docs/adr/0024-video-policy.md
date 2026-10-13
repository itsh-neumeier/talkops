# 0024 - Video only on local calls and trunks with video

- Status: Accepted
- Date: 2026-10-13

## Context

TalkOps supports video between extensions, from door stations and through
trunks. The flag `extensions.video_enabled` allows video for an extension.
The flag `trunks.video_enabled` allows video through a trunk. Most providers
have no video.

The internal SIP profile listed `H264,VP8` in its codec preferences. The
dialplan removed video only on some paths. These paths sent a video offer to
the phones:

- Queue agents (`mod_callcenter`) and other calls without a dialplan codec
  list. Thus a call from a trunk to a queue rang the video phones with video.
- Door stations rang every target with video. The flag of the target had no
  effect.
- Calls to SIP addresses offered video to every peer when the caller had
  video. The flag of the trunk had no effect.

A video phone (for example a Yealink T58W or VP59) then asks if the user
wants to answer with video. The browser softphone always showed the
*Answer with video* button for an extension with video.

A trunk with video could not send video to a phone. The external profile
negotiated the provider's offer before the dialplan, with audio codecs only.

## Options

1. Remove the video codecs on each path that does not allow video. Each new
   path must remember this. A missing removal sends video again.
2. Offer audio only by default. The dialplan allows video per call and per
   called leg when the policy allows it.

## Decision

TalkOps uses option 2.

- The internal profile lists audio codecs only (`OPUS,G722,PCMA,PCMU`).
  A call without a codec list from the dialplan is audio-only.
- The external profile uses `inbound-late-negotiation`. FreeSWITCH
  negotiates the provider's offer when the call is answered, with the codec
  list of the dialplan.
- A call can use video only if the caller's offer has an active video stream
  (`m=video` with a port other than 0 in `switch_r_sdp`).
- The dialplan calculates a video policy for the call (`CallVideo`):

  | Call | Video to |
  | --- | --- |
  | Extension with video to an extension with video | the devices of that extension |
  | Extension with video to a ring group | the members with video |
  | Door station to its target | all called extensions |
  | Trunk with video to an extension or ring group | the extensions with video |
  | Extension with video to a trunk with video | the provider |
  | Extension with video to a SIP address through a trunk with video | the peer |
  | All other calls (trunks without video, direct SIP addresses, queues, voicemail, IVR, conferences, transfers) | nobody |

- For a call with video, the caller's leg gets a codec list with video
  (`set absolute_codec_string=…,H264,VP8`). The called legs get the audio
  list (`export nolocal:absolute_codec_string=…`). Each device that gets
  video has its own codec list with video in the bridge string
  (`[^^;absolute_codec_string=…,H264,VP8]`). `local_var_clobber=true` lets
  this per-leg list win over the exported audio list (see ADR 0023).
- A device with a codec profile `hd` or `robust` stays audio-only as a
  caller and as a called device.
- The browser softphone shows *Answer with video* only if the offer of the
  call has an active video stream.

## Consequences

- Positive: a call from a provider never asks a phone about video, unless
  the trunk has video and the provider offers video.
- Positive: a new call path is audio-only until the dialplan allows video.
- Positive: a trunk with video can send video to an extension with video.
- Negative: a queue does not support video. A call transferred to another
  extension loses video.
- Negative: late negotiation changes the time of the codec negotiation for
  all trunk calls. The tests showed the same codecs for audio calls
  (bridge, voicemail, queue).
- Negative: a door station call to an extension without the video flag
  still sends video. This keeps the existing door station behavior: the
  call is local.
- Fix found during this work: `export nolocal:` gives a CDR element name
  with a colon (`<nolocal:absolute_codec_string>`). TalkOps rejected these
  records as malformed. Calls from trunks lost their call records since the
  dialplan used `nolocal:` for these calls. The CDR parser now replaces the
  colon in element names.
