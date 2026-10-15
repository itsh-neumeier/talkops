# 0023 - Codec profiles per device and transcoding in FreeSWITCH

- Status: Accepted (amended by [ADR 0027](0027-device-media-settings.md):
  profile `g711`, video setting per device)
- Date: 2026-10-12

## Context

Wi-Fi handsets (Yealink AX83H, AX86R) operate in networks with packet loss,
jitter and roaming gaps. The system codec lists are fixed in the dialplan
(`OPUS,G722,PCMA,PCMU` for internal calls, the trunk codecs for provider
calls). The AX86R does not support Opus. Without provisioned codec settings,
the AX86R enables only G.711 and its calls use narrowband audio.

FreeSWITCH always handles the media of a call (no `bypass_media`). Thus
FreeSWITCH can transcode between two legs with different codecs.

## Options

1. Use one global codec list for all devices. This option does not help a
   device in a bad network.
2. Set a codec list only in the directory entry of the device
   (`dial-var-*` or the `{…}` variables of the dial-string). A test with
   FreeSWITCH 1.10.12 showed that this does not work for dialplan bridges:
   `switch_ivr_originate` sets the variables the caller exported on the new
   channel after the channel is created, so the exported
   `absolute_codec_string` of the dialplan replaces the value.
3. Set a codec profile per device. The dialplan sets the codecs of the leg
   to the device per leg in the bridge string. The directory covers calls
   without a dialplan bridge. The dialplan sets the codecs of the leg from
   the device.
4. Change the codec in a call (`uuid_media_reneg`) when the quality drops.
   The re-INVITE causes a short audio gap and often fails in a bad network.

## Decision

TalkOps uses option 3.

- The column `devices.codec_profile` (`standard`, `hd`, `robust`) stores the
  profile. `NULL` means automatic: Wi-Fi handsets use `hd`, all other
  devices use `standard`.
- `standard` changes nothing. The dialplan codec lists apply.
- `hd` uses `G722,PCMA,PCMU`. `robust` uses `iLBC@20i,iLBC@30i,PCMA,PCMU`.
  Both profiles add a jitter buffer (`jitterbuffer_msec=60:200`) on the leg
  of the device. `rtp_jitter_buffer_during_bridge=true` keeps the jitter
  buffer active while the call is bridged. Without it, FreeSWITCH pauses
  the jitter buffer in a bridge.
- Calls to the device through the dialplan (extension, ring group): each
  endpoint of the device has the per-leg variables
  `[^^;absolute_codec_string=…;jitterbuffer_msec=…;rtp_jitter_buffer_during_bridge=true]`.
  The bridge string starts with `{local_var_clobber=true}`, so the per-leg
  variables win over the exported codec list. `^^;` keeps the commas of the
  codec list in the variable.
- Calls to the device without a dialplan bridge (queue agents of
  `mod_callcenter`, calls started over the Event Socket): the `{…}`
  variables of the directory dial-string set the same values. These calls
  have no exported codec list.
- Calls from the device: the directory variable `talkops_codec_profile`
  identifies the profile. The dialplan sets the same variables on the
  caller's leg first. The plan changes its own
  codec lists to `export nolocal:absolute_codec_string=…`, so these lists
  apply only to the called legs.
- FreeSWITCH transcodes between the leg of the device and the other leg.
- The FreeSWITCH image contains `mod_ilbc`. The library is
  `freeswitch/libilbc` tag `v0.0.1` (commit `7cee47f`), built from source,
  because Debian 13 has no package.
- Provisioning: for a device with `hd` or `robust`, the Yealink account gets
  G.722, iLBC and G.711 enabled and all other codecs disabled
  (`account.X.codec.<payload>.enable/.priority`). TalkOps selects the codec
  of each call. A change between `hd` and `robust` thus needs no resync.
  `standard` writes no codec settings.
- TalkOps does not set a DSCP value. Yealink phones mark RTP with DSCP 46
  (EF) by default, and a change of this value restarts the phone.

## Consequences

- Positive: an administrator can set the codec of one device. The device
  gets a robust codec, and the other party keeps its codec.
- Positive: Wi-Fi handsets get G.722 wideband audio by default.
- Negative: transcoding uses CPU time. For up to approximately 50 extensions
  the load is small.
- Negative: a device with `hd` or `robust` gets no video. The profiles
  contain audio codecs only.
- Negative: iLBC has narrowband audio. Use `robust` only for devices in a
  bad network.
- Negative: the jitter buffer adds 60 to 200 ms of delay on the leg of the
  device.
- Follow-up work: record the call quality per device (packet loss, jitter,
  MOS) and select `hd` or `robust` automatically from configurable rules.
