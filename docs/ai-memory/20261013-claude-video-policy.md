# Video only on local calls and trunks with video

## Task state

- Task ID: 20261013-claude-video-policy
- Date: 2026-10-13
- Tool: Claude Code
- Status: Completed (PR submitted for review)
- Branch: `claude/dreamy-dirac-1djvf7`
- Base commit: `21f5a51` (release 1.7.7 on `main`)
- Related commits or PR: Pending

## Goal

The user wants video on local calls: extension to extension, door station to
indoor stations and video desk phones. Calls from trunks must not offer
video. A video phone asked "answer with video?" on calls from outside. Only
a trunk with the video flag can carry video. Most providers have no video.

The user approved the proposal with these defaults:

1. A SIP address without a trunk (direct peer) is audio-only.
2. Ring groups give video per member with video.
3. Queues are always audio-only.
4. A forward from an extension with video to an external number is
   audio-only, unless the trunk has video.

## Result

Implemented, tested locally and on a local FreeSWITCH 1.7.7 stack.
ADR 0024 records the policy.

Root causes found:

- The internal SIP profile listed `H264,VP8` in its codec preferences. Each
  call without a dialplan codec list offered video: queue agents
  (`mod_callcenter`), calls over ESL. A trunk call to a queue thus asked the
  video phones about video.
- Door stations sent video to every target, also without the flag.
- SIP addresses offered video to every peer when the caller had video.
- The browser softphone always showed *Answer with video* for an extension
  with video.
- A trunk with video could not send video to a phone: the external profile
  negotiated the provider's offer before the dialplan.

Bug found and fixed: `export nolocal:` gives a CDR element name with a
colon (`<nolocal:absolute_codec_string>`). The CDR parser (roxmltree)
rejected the record as malformed. All calls from trunks lost their call
records since `3e8974b` (2026-10-10, `TRUNK_TO_PHONE_CODECS`). The local
stack showed `malformed CDR … unknown namespace prefix 'nolocal'` for trunk
calls before the fix, and `call record stored` after the fix.

## Changes

- `crates/talkops-api/src/fsxml/sofia.rs`: internal profile codecs audio-only;
  external profile `inbound-late-negotiation=true`.
- `crates/talkops-api/src/fsxml/dialplan.rs`: `CallVideo` policy in
  `Routing`; `device_endpoint` adds a per-leg codec list with video;
  `offers_video` (`switch_r_sdp`), `video_destination`, `video_media`,
  `has_codec_profile`; policy in the internal, door station, SIP address,
  outbound and public (trunk) paths.
- `crates/talkops-api/src/routes/fs.rs`: `Routing.video` starts as `Off`.
- `crates/talkops-api/src/fsxml/cdr.rs`: `without_name_prefixes` before the
  XML parse.
- `crates/talkops-core/src/extensions.rs`: doc comment of `video_enabled`.
- `web/src/lib/softphone.ts`, `web/src/lib/components/Softphone.svelte`:
  `offersVideo`; *Answer with video* only for an offer with video.
- `web/src/lib/i18n/en.ts`, `de.ts`: hints of the extension and trunk flags.
- Tests: `crates/talkops-api/tests/freeswitch.rs` (`video_only_where_enabled`
  rewritten, new `video_from_trunks_and_door_stations`, video checks in
  `sip_addresses`), unit tests in `dialplan.rs` and `cdr.rs`,
  `web/tests/softphone.test.ts`, SIPp `tests/sipp/uas_audio_only.xml`,
  `tests/sipp/trunk_call_video.xml`, new section in `tests/e2e/run.sh`.
- Docs: `docs/adr/0024-video-policy.md`, `docs/adr/README.md`,
  `docs/en/softphone.md`, `docs/de/softphone.md`, `docs/en/door-station.md`,
  `docs/de/tuersprechstelle.md`.

## Decisions

- Audio by default, video only by an explicit per-call and per-leg codec
  list. A new call path is thus audio-only. Reason: robustness; the old
  approach removed video per path and missed paths.
- Video only if the caller's offer has an active `m=video`. Reason: a trunk
  with video sends mostly audio-only calls; without this check, a video
  phone would ask about video on each call (seen on the local stack).
- Door stations keep video to all targets (old documented behavior,
  "Door stations always show their video"). The call is local.
- Late negotiation on the external profile, always on. A conditional value
  (only with a video trunk) needs a restart of the external profile on a
  flag change. That drops calls. Tests showed no change for audio calls.
- The trunk leg accepts video only if a called extension can get video
  (`video_destination`), so the provider does not get a video answer for
  an audio-only target.
- ADR number 0024; no migration (the flags exist since `20261021000003`).

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- Passed: `cargo test --workspace --locked --no-fail-fast` with PostgreSQL 16
  (all tests, including `backup_and_restore`).
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build` in
  `web/`.
- Passed (local stack: TalkOps from this branch, image
  `ghcr.io/itsh-neumeier/talkops-freeswitch:1.7.7`, SIPp 3.7.2):
  extension with video to extension with video gets video; audio offer gives
  audio; to an extension without video gives audio and the caller gets
  `m=video 0`; door station gives video; trunk without video gives audio;
  trunk with video and video offer gives video on both legs; trunk with
  video and audio offer gives audio; trunk to queue gives audio to the agent;
  trunk to voicemail answers with audio. No channels stayed open.
- Passed: the new `tests/e2e/run.sh` section against the local stack.
- Not run: the full `tests/e2e/run.sh` (needs all images, Piper). CI
  workflow `E2E` runs it.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- Migration IDs: none.
- Upgrade: existing door stations keep video. Phones with video get video
  from trunks only when the trunk has *Video calls via this trunk*. Call
  records of trunk calls are stored again. Records that FreeSWITCH spooled
  and TalkOps moved to `cdr-spool/rejected/` are not imported again.
- Found, not fixed here (out of scope): FreeSWITCH writes spool files
  percent-encoded without a `cdr=` key. `cdr_spool::spool_xml` does not
  decode them, so the importer rejects every real spool file
  (`unknown token at 1:1`, 7 of 7 files on the local stack). Suggested as a
  separate task.
- Open: a queue does not support video. A transferred call loses video.

## Next action

Wait for the review and the merge by the release manager.

## References

- `docs/adr/0024-video-policy.md`
- `docs/adr/0023-device-codec-profiles.md`
- `docs/ai-memory/20261012-claude-codec-profiles.md`
