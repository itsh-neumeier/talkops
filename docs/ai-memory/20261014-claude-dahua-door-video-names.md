# Dahua door calls: video codec and ASCII caller names

## Task state

- Task ID: 20261014-claude-dahua-door-video-names
- Date: 2026-10-14
- Tool: Claude Code
- Status: Completed (PR submitted for review)
- Branch: `claude/zen-wright-tuunvi`
- Base commit: `3cad018` (origin/main, TalkOps 1.7.10)
- Related commits or PR: Pending

## Goal

Hardware: Dahua VTO4202F-P-S2 (V4.600.0.3), two VTH5422H (V4.800.0.2).

1. A call from the VTO to a VTH connects with audio. The VTH shows no
   picture. A call from the VTH to the VTO shows the picture.
2. A call from the VTH to the VTO ends after 30 s (the user confirmed).

The user asked for an analysis and a proposal first. Then the user approved:
"the door station can only send video, use the correct format", and
asked to fix the umlaut problem.

## Result

Analysis of the SIP trace from the user:

- VTH to VTO (picture shown): the VTH offers H264 `recvonly`, FreeSWITCH
  answers H264 `sendonly`. The video path VTO → FreeSWITCH → VTH works.
- VTO to VTH (no picture): FreeSWITCH offers H264 and VP8 with `rtcp-fb`
  and no direction attribute. The VTH answers H264 `recvonly`.
- 30 s drop: the VTO retransmits its `200 OK` every 4 s although
  FreeSWITCH sends an ACK each time, then sends BYE after 30 s. The VTO
  does not accept the ACK. The same ACK form works with the VTH (newer
  firmware). The main difference is the UTF-8 `From` display name
  ("Türsprechstelle_…").

Changes:

- Door station calls offer only the video codec that the station offers
  (H264 for Dahua; VP8 or both if the station offers them). FreeSWITCH
  relays video without transcoding (`mod_h26x`), so VP8 could never show
  the picture.
- On calls from a door station and on direct calls to a door station
  extension, the dialplan sets `effective_caller_id_name` to an ASCII form
  of the name (`ü` → `ue`, `ß` → `ss`, other diacritics to the base
  letter, other characters left out). `talkops_caller_name` (call records)
  keeps the full name. Other calls are not changed.

## Changes

- `crates/talkops-api/src/fsxml/mod.rs`: `ascii_name` with unit test.
- `crates/talkops-api/src/fsxml/dialplan.rs`: `door_codecs`,
  `AUDIO_H264_CODECS`, `AUDIO_VP8_CODECS`; door station path uses them and
  sets the ASCII caller name; direct calls to a door station set the ASCII
  caller name. Unit test `door_calls_offer_the_video_codec_of_the_station`.
- `crates/talkops-api/tests/freeswitch.rs`: door part of
  `video_from_trunks_and_door_stations` expects H264 only.
- `crates/talkops-api/tests/doors.rs`: new test
  `door_calls_use_ascii_caller_names`.
- `docs/en/door-station-dahua.md`, `docs/de/tuersprechstelle-dahua.md`:
  two troubleshooting rows.

## Decisions

- `a=sendonly` in the video offer to the VTH is not possible with
  configuration: FreeSWITCH 1.10.12 has `origination_audio_mode`, but no
  video counterpart (checked in `switch_core_media.c` of v1.10.12 and
  master). The B-leg offer takes the direction from the A-leg's remote
  flow. A FreeSWITCH patch was not made: no upstream source, and the VTH
  already answers `recvonly`. The `rtcp-fb` lines in offers are also
  fixed in FreeSWITCH.
- ASCII names only on door calls. Yealink phones and browsers show UTF-8
  names correctly; a global change would lose umlauts everywhere.
- The fix for the 30 s drop is based on a hypothesis (the UTF-8 `From`
  name). A hardware test must confirm it.
- No ADR: the change refines ADR 0025 (video policy) and ADR 0026 (door
  vendors) without changing a decision.

## Checks

- Passed: `cargo fmt --all -- --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with local PostgreSQL 16,
  two complete runs.
- Failed once, not reproduced: `door_station` in one complete run (assert
  on a JSON value while it waits for events). It passed 3 times alone, in
  2 more complete runs, and on the base commit. Probably timing under load;
  it is not related to this change.
- Not run: `tests/e2e/run.sh` (Docker/SIPp stack). No web changes.
- Not run: hardware test with the Dahua VTO and VTH.
- STE review: manual review against `docs/documentation-style.md`; the
  full standard was not available. Review pending.

## Dependencies and open work

- Migration IDs: none.
- Open: If the VTH still shows no picture, record a packet capture on the
  TalkOps host (`tcpdump -ni any -s0 -w door.pcap 'host 192.168.90.11 or
  host 192.168.90.12'`) and check if FreeSWITCH sends video RTP to the VTH.
  Next candidates: the VTO's SIP user (`8000VTO`) as the `From` user, and
  the `TransMode: SuggestRTSP` header on the leg to the VTH.
- Open: If the 30 s drop stays, check the ACK in the capture (CSeq size,
  session timer).

## Next action

The user tests VTO → VTH (picture) and VTH → VTO (longer than 30 s) after
the release. Then the release manager merges the PR.

## References

- `docs/ai-memory/20261013-claude-video-policy.md`
- `docs/ai-memory/20261013-claude-dahua-vth-setup.md`
- `docs/adr/0025-video-policy.md`, `docs/adr/0026-door-station-vendors.md`
