# TalkOps 1.7.11 release

## Task state

- Task ID: 20261014-codex-release-1.7.11
- Date: 2026-10-14, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.7.11 (removed after verification)
- Base: 3cad0180d34fedaa554339afaa67a9e56fed0ab4 (main, 1.7.10)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/22
- Candidate: 32b9bd96fd0557379ae66b8f8213d459b1bc63d4
- Release commit: 7b6e51f612475fc4473880f8a2f818c49147e9e2 (main)
- Annotated tag: v1.7.11; object c5e6eb01d2b3cdc55bd8b09a8d8fd40140d1ae6d
- Published: 2026-10-14T09:20:40Z
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.11

## Goal and result

Publish all open PRs as 1.7.11. The only source was ready Claude Code PR #21:
station video codecs and ASCII names on door calls. Exact head:
f533c65b73994a4de7ac858a824d7ee3e3240487, branch claude/zen-wright-tuunvi.
No Codex feature PR was open. No planned contribution was omitted or blocked.
No external dependency or migration. Conflict-free merge commits retain source
history and the completed 1.7.10 audit. The merged tree equals the candidate.

Codex supplied integration, necessary corrections/regressions, release metadata
and verified publication. Existing published tags stayed fixed.

## Changes and decisions

- Door-originated calls use the station's H264/VP8 codecs. Direct internal calls
  to door station extensions and calls from stations use ASCII SIP names.
  Full caller names remain in CDR; ordinary calls keep their names.
- Ignore port-zero media and payload mappings not listed in active media.
  Add unit regressions for both. Retain the source's H264 fallback if mappings
  are missing. No general video-policy or FreeSWITCH source change.
- Normalize effective names after door-target routing so a ring-group prefix
  cannot reintroduce umlauts. Extend the DB/API regression; verify full CDR name.
- Mark the suspected UTF-8 ACK cause as unconfirmed in code comments, EN/DE
  troubleshooting and English release notes. No real-device success is claimed.
- Set Cargo workspace and web root versions to 1.7.11 with package tools.
  External dependency locks unchanged. SQL unchanged, 36 unique migration IDs.
  DE/EN retain 1,338 matching keys; no API/UI schema change.

## Checks

All final candidate and resulting main checks passed before the single tag
push. A fresh gate checked scope, remote main, source ancestry, code versions
and unused tag/release. The existing Release workflow published the artifacts.

| Stage | CI | E2E | Result |
| --- | --- | --- | --- |
| #21 source PR | 38529444449 | 38529444294 | Passed |
| #21 source push | 38529416822 | 38529416677 | Passed |
| Final candidate PR | 38536710003 | 38536710012 | Passed |
| Final candidate push | 38536705015 | 38536704964 | Passed |
| Resulting main | 38538862130 | 38538862192 | Passed before tagging |
| Tag | 38540847692 | 38540847773 | Passed |

- Release 38540847643: all six native image builds, three manifests and GitHub
  release passed. Tag FreeSWITCH image 38540847745 passed health/modules/iLBC/UID.
- CI covers Rust fmt/clippy/locked PostgreSQL/LDAP tests, web/browser checks,
  Compose/host recovery and image smoke tests. E2E covers the complete stack,
  SIPp audio/media/video and browser softphone.
- ASCII, station codec, ring-group and door_station tests passed on candidate
  and main. Both server and worker smoke tests report 1.7.11.
- Source reported one earlier local door_station event-wait failure. It did not
  recur in three single/two complete source runs or final candidate/main CI.
  No failed required check was ignored.
- Local Rust fmt/whitespace, npm ci/lint/check (zero errors/warnings), six Node
  tests/build, migration/translation/ancestry and lock checks passed.
- Full Linux services used CI because the local Docker daemon was unavailable.
  Deployed UI and real Dahua hardware were unavailable. No physical door,
  device password or production deployment was changed.
- Manual integration review; CodeRabbit CLI unavailable.
- Full official STE standard unavailable; full language review remains pending.

## Published artifacts

GitHub release is published, not a draft or prerelease. Its annotated tag
resolves to the checked release commit. Registry inspection confirms linux/amd64
and linux/arm64 in all three versioned manifests. All six configs have version
v1.7.11 and revision 7b6e51f612475fc4473880f8a2f818c49147e9e2. Moving tags
1.7 and latest have the same platform digests as 1.7.11.

| Image | amd64 platform digest | arm64 platform digest |
| --- | --- | --- |
| talkops-server | sha256:06081f68c4bf61eb68b69dd3d7c323ea72212fa970f488d9bcfd951e2d9cd191 | sha256:67cba32b26c8037c5f9287d90854ad5cd57b265202cadd2082a338463158e93e |
| talkops-media-worker | sha256:5fee95a66aee4187655a0f7e458db55886d05f24f1a012755831082d23370039 | sha256:1d5bc7946993e7be1ac949f8015c893b64b8453fa38061a2868bd002b5065009 |
| talkops-freeswitch | sha256:b296eff5690279f51e103fb09b35841aa9cbbf76e03a2a8e43007abe8ad6553b | sha256:968168943a9fe49118fd433b78f59ac4c3023e0bce1937d90e30f7f930659d2e |

## Upgrade and remaining work

No migration, password change or FreeSWITCH source patch. Update the server to
use the revised dialplan on new calls. Hardware must verify VTO-to-VTH picture
and VTH-to-VTO calls longer than 30 seconds on VTO4202F-P-S2 V4.600.0.3 /
VTH5422H V4.800.0.2. ASCII addresses a suspected cause; tests do not confirm it.
Video direction and rtcp-fb behavior are unchanged; no sendonly override.
The raw percent-encoded CDR spool import defect remains separate work.
No assigned PR fixes it. Preserve cdr-spool/rejected; do not claim recovery.

## Cleanup and durable state

Validated unchanged source/release heads and ancestry, then atomically removed
claude/zen-wright-tuunvi and codex/release-1.7.11 remotely. Remote has only main,
with no open PR. Preserved foreign local branches/worktrees, primary unpublished
docs commit 397fc83ec5a0c1032e3c8b944eab337b3ef495f8 and three untracked Dahua
summaries. They have no source PR and were excluded; no foreign work was imported.
Saved this final summary in a docs-only audit commit with annotated
audit/release-1.7.11. Detached the owned checkout and removed only its local release
branch. Audit-tag checks are separate from released code. No automation or
development-chat message was requested or created.

## Next action

None for this release. Hardware verification and CDR spool recovery are separate
development work. A later release needs a fresh scope review and a new version.

## References

- [Integration PR](https://github.com/itsh-neumeier/talkops/pull/22)
- [Source PR](https://github.com/itsh-neumeier/talkops/pull/21)
- [Release](https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.11)
- [Release workflow](https://github.com/itsh-neumeier/talkops/actions/runs/38540847643)
- [Source summary](20261014-claude-dahua-door-video-names.md)
- [Previous release](20261014-codex-release-1.7.10.md)
- [Video policy](../adr/0025-video-policy.md)
- [Door vendors](../adr/0026-door-station-vendors.md)
