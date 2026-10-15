# Door call video: media settings per device

## Task state

- Task ID: 20261015-claude-door-video-device-media
- Date: 2026-10-15
- Tool: Claude Code
- Status: Completed (PR submitted for review)
- Branch: `claude/jolly-curie-t4126j`
- Base commit: `6276e7d` (origin/main, TalkOps 1.7.12)
- Related commits or PR: Pending

## Goal

Hardware: Dahua VTO4202F-P-S2 (V4.600.0.3), two VTH5422H (V4.800.0.2),
browser softphone (Chrome, SIP.js).

1. VTO → VTH: audio, but no picture on the VTH.
2. Browser softphone (extension 8000, the door station number) → VTH 8002:
   video worked "yesterday", now no picture. Browser 999 → VTH: no picture.
   VTH → browser and VTH → VTO: picture works.

The user asked for analysis and a proposal first. After the analysis and a
hardware test (main VTO entered in the VTH: still no picture), the user
asked: "make all variants configurable: video on or off, audio codec and
video codec".

## Result

Analysis of the SIP traces:

- Browser → VTH: FreeSWITCH answered the browser with VP8 (Chrome lists VP8
  first; the internal profile uses the remote order). The VTH answers
  `H264 recvonly`. FreeSWITCH relays H264 (`mod_h26x`) and cannot encode
  H264, so no picture.
- VTO → VTH: H264 on both legs, no codec problem. The VTH answers a call
  from its VTO number at once with `180 Ringing` with SDP and
  `DependentInfo: <VTO IP>`. Dahua documentation (Quick Start Guide 4.8.1,
  section 3.2.4) and the Yeastar Dahua guide: the VTH gets the live picture
  directly from its main VTO. The user entered the main VTO: still no
  picture. Cause still open.
- The early media of the VTH (180 with SDP) made FreeSWITCH cancel all other
  parallel targets (`LOSE_RACE`): the browser of extension 8002 never rang.

Implemented (ADR 0027):

- `devices.video` (`auto`, `off`, `h264`, `vp8`; NULL = automatic: `auto`
  for profile `standard`, else `off`). Applied to called legs
  (`device_endpoint`) and to the caller's leg (`with_caller_codecs`, via
  directory variable `talkops_video`).
- Codec profile `g711` (PCMA, PCMU, no jitter buffer). A profile with a video
  setting other than `off` gets profile audio plus the video codecs.
- `inherit_codec=true` on the caller's leg of calls with video
  (`video_media`); removed for callers with a codec profile.
- Door calls: `ignore_early_media=true` and `ring_ready` before the bridge.
- Diagnostics capture: one line per leg at `CHANNEL_HANGUP_COMPLETE` with
  audio and video codec and RTP packet counters.
- Web: device dialog field "Video", profile "G.711 only", badge; in-app
  Dahua guide section "Video on the VTH".

## Changes

- `migrations/20261031000001_device_video_and_g711.sql`: enum value `g711`,
  type `device_video`, column `devices.video`.
- `crates/talkops-core/src/extensions.rs`: `DeviceVideo`, `CodecProfile::G711`,
  `video` in `Device`, `DeviceInput`, `DeviceAuth`, `RingTarget`, queries.
- `crates/talkops-api/src/fsxml/codecs.rs`: optional jitter buffer,
  `vars_with_codecs`, profile `g711`.
- `crates/talkops-api/src/fsxml/dialplan.rs`: `with_caller_codecs`,
  `with_video`, `caller_device_video`, `device_endpoint`, `video_media`
  (`inherit_codec`), door path (`ignore_early_media`, `ring_ready`); tests.
- `crates/talkops-api/src/fsxml/directory.rs`: variable `talkops_video`.
- `crates/talkops-api/src/diagnostics.rs`: `media_summary`, subscription.
- `crates/talkops-api/src/routes/extensions.rs`: audit detail `video`.
- `crates/talkops-api/tests/freeswitch.rs`: door call video settings test.
- Struct literals in tests and `active_calls.rs`: `video: None`.
- `web/src/lib/api.ts`, `web/src/lib/components/DeviceList.svelte`,
  `web/src/lib/i18n/{en,de}.ts`, `web/src/lib/doors/guides/dahua.ts`.
- Docs: `docs/adr/0027-device-media-settings.md`, notes in ADR 0023, 0025,
  `docs/adr/README.md`, `docs/en/codec-profiles.md`,
  `docs/de/codec-profile.md`, `docs/en/door-station-dahua.md`,
  `docs/de/tuersprechstelle-dahua.md`.

## Decisions

- Settings per device instead of Dahua detection: the correct Dahua settings
  are not known, and the user wants to test variants without releases.
- Default of `devices.video` follows the codec profile, so existing devices
  keep their behaviour (ADR 0025 rule "profile = no video").
- `ignore_early_media` only on door calls. Trunk calls need early media
  (provider announcements). A SIP early-media preview with parallel ringing
  is not possible in FreeSWITCH 1.10.12 (`bridge_early_media` relays audio
  of one leg only; checked in `switch_ivr_originate.c`).
- `inherit_codec` (FreeSWITCH 1.10.12 `inherit_codec()`): copies the
  answering leg's audio and video codec to the caller before its SDP
  answer. Only on calls with video. Not for callers with a codec profile.
- New ADR 0027, because the change amends ADR 0023 and ADR 0025.

## Checks

- Passed: `cargo fmt --all -- --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with local PostgreSQL 16
  (all test binaries). After `cargo fmt`, the `talkops-api` library tests
  and the `freeswitch` and `doors` tests ran again and passed.
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build` in
  `web/`.
- Not run: `tests/e2e/run.sh` (Docker/SIPp stack). CI runs it.
- Not run: hardware test with the Dahua VTO and VTH and the browser.
- STE review: manual review against `docs/documentation-style.md`; the
  full standard was not available. Review pending.

## Dependencies and open work

- Migration ID: `20261031000001`.
- Open: VTO → VTH picture. Next test after the release: capture with the
  new media lines; compare VTH video "H264 only" and "Off" with the main
  VTO entered in the VTH.

## Next action

The release manager reviews and integrates the PR. Then the user tests on
hardware and sends a capture with the media lines.

## References

- `docs/ai-memory/20261014-claude-dahua-door-video-names.md`
- `docs/ai-memory/20261013-claude-video-policy.md`
- `docs/adr/0023-device-codec-profiles.md`, `docs/adr/0025-video-policy.md`,
  `docs/adr/0027-device-media-settings.md`
