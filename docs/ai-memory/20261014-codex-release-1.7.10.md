# TalkOps 1.7.10 release

## Task state

- Task ID: 20261014-codex-release-1.7.10
- Date: 2026-10-14, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.7.10 (removed after verification)
- Base: bfc46429f0889ceec79e72f45b49c6ec79dc9f26 (main, 1.7.9)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/20
- Candidate: b1582421e5a451afce0602cd610f40c9beaad991
- Release commit: 3cad0180d34fedaa554339afaa67a9e56fed0ab4 (main)
- Annotated tag: v1.7.10; object 2202a9027519ab16df7777893e83f4e4452f4af0
- Published: 2026-10-14T06:45:02Z
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.10

## Goal and result

Publish all open PRs as the user-selected patch 1.7.10. The only source was
ready Claude Code PR #19, branch claude/busy-fermat-sy3r41, exact head
8ad591945ef17bea48f5edf3fe27a85fe86c0434. It adds Dahua VTO/VTH setup,
per-device SIP password formats and door station vendors. The handoff named
692f9b0; the actual final head only adds a task-summary link.

Codex supplied integration, a necessary validation/regression and release
metadata. No Codex feature PR was open. No planned source was omitted or
blocked. No external PR dependency. API additions keep existing defaults.
Merge commits preserve source history and the completed 1.7.9 audit. The source
merge was conflict free. The resulting main tree equals the checked candidate.

## Changes and decisions

- The source checked the old device kind on updates. Validate the resulting
  kind and password format as well. A change to Browser cannot retain a compact
  format. Test both an omitted and explicit compact format; both fail without
  changing the device or password. Retain the rule for existing browser devices.
- Set Cargo workspace and web root versions to 1.7.10 with Cargo/npm tools.
  External dependency locks stayed unchanged. Add English release/upgrade notes.
- Verify 36 unique migration IDs. New 20261030000001_device_sip_password_format
  and 20261030000002_door_station_vendor follow released 20261029000001.
  Released SQL stayed unchanged. DE/EN have 1,338 matching keys.
- Verify the cited primary Yeastar guide: it specifies 8-15 characters and
  the room field mapping. It does not identify the tested VTH model/firmware.
- Keep other worktrees unchanged. The primary local unpublished docs commit
  397fc83ec5a0c1032e3c8b944eab337b3ef495f8 has no source PR and was excluded.
  Preserve its three untracked Dahua summaries: development-prompt, sip-alias
  and vth-registration, dated 20261013. No foreign changes were imported.

## Checks

All final candidate and resulting main checks passed before the single tag
push. A fresh gate checked scope, main, versions, source ancestry and unused
tag/release. Existing published tags stayed fixed. All runs below use the
exact source, final candidate or release commit for their respective stage.

| Stage | CI | E2E | Result |
| --- | --- | --- | --- |
| #19 source PR | 38472348133 | 38472348145 | Passed |
| #19 source push | 38472343115 | 38472343116 | Passed |
| Final candidate PR | 38520138029 | 38520138011 | Passed |
| Final candidate push | 38520132966 | 38520132928 | Passed |
| Resulting main | 38521860605 | 38521860655 | Passed before tagging |
| Tag | 38523419533 | 38523419490 | Passed |

- Release workflow 38523419501: passed all six native image builds, three
  manifest jobs and GitHub release creation.
- Tag FreeSWITCH image 38523419487: passed health, modules, iLBC and UID checks.
  FreeSWITCH sources/workflow were unchanged; no separate branch-path job applied.
- CI: Rust fmt/clippy/locked tests with PostgreSQL/LDAP; web/browser checks;
  Compose/host recovery and server/worker image smoke tests. Both password-format
  test suites passed, including the new device-kind regression.
- E2E: complete stack, SIPp registration/audio/media/video and browser softphone.
- Server and worker smoke tests report 1.7.10.
- Local npm ci/lint/check (zero errors/warnings), six Node tests/build, Rust
  format, whitespace, migration/translation/ancestry and lock checks passed.
- Full Linux services used CI because the local Docker daemon was unavailable.
  Source reports desktop and 390 px Playwright checks. Real Dahua VTO/VTH
  hardware and deployed UI observation were unavailable. No actual door,
  device password or production deployment was changed.
- Manual integration review; CodeRabbit CLI unavailable.
- Full official STE standard unavailable; full language review remains pending.

## Published artifacts

The GitHub release is published, not a draft or prerelease. The annotated tag
resolves to the checked release commit. Registry inspection confirms linux/amd64
and linux/arm64 in all three versioned manifests. All six image configs have
version v1.7.10 and revision 3cad0180d34fedaa554339afaa67a9e56fed0ab4.
Moving tags 1.7 and latest have the same platform digests as 1.7.10.

| Image | amd64 platform digest | arm64 platform digest |
| --- | --- | --- |
| talkops-server | sha256:0d02967c08bcdc4270c934f64d1de8a737f638a874455560c943626f1fa982e4 | sha256:70ae31594dbb4be32c4366676c652cf97f67ec2f5fd676709510144575ab3caf |
| talkops-media-worker | sha256:e095e8f242d9ba5ee1fa76e5152efeb85b0ab31bd5d7e7eaa46533a800797a90 | sha256:9bc0d1d54c1474c49a9586e6c391e74084712a4c2681317c8b5c4c139854595b |
| talkops-freeswitch | sha256:f7b8d21fc4b67fdf73ab555050d8e437f645575cea90e1bfb6339b1807b9100f | sha256:a793081ef2a2b858e9d722a940024c296f13808e6600fb01939736dfd7572cdf |

## Upgrade and remaining work

Existing passwords stay unchanged. Editing a format applies it to the next
password reset only. New door devices preselect compact15 in the web UI;
clients without the field keep standard. Update device credentials after a
reset. Existing stations get Dahua. Follow ADR 0026 and the EN/DE setup guide.

The 15-character workaround and Room No. settings for VTH5422H firmware
V4.800.0.2 still need a real-device test. A softphone test does not verify them.
The known raw percent-encoded CDR spool import defect remains separate work.
No assigned PR fixes it. Preserve cdr-spool/rejected; do not claim recovery.

## Cleanup and durable state

Validated unchanged source/release heads and ancestry, then atomically deleted
claude/busy-fermat-sy3r41 and codex/release-1.7.10 on the remote. GitHub now has
only main; no open PR remains. Preserved foreign local branches/worktrees,
unpublished docs and untracked files. Saved this final summary in a docs-only
audit commit with annotated audit/release-1.7.10. Detached the owned checkout
and removed only its local release branch. Audit tag checks are separate from
the released code. No automation or development-chat message was created.

## Next action

None for this release. A later release needs a new version and a fresh scope
review against remote main. Hardware verification and CDR spool recovery are
separate development work.

## References

- [Integration PR](https://github.com/itsh-neumeier/talkops/pull/20)
- [Source PR](https://github.com/itsh-neumeier/talkops/pull/19)
- [Release](https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.10)
- [Release workflow](https://github.com/itsh-neumeier/talkops/actions/runs/38523419501)
- [Source summary](20261013-claude-dahua-vth-setup.md)
- [Previous release](20261013-codex-release-1.7.9.md)
- [Vendor decision](../adr/0026-door-station-vendors.md)
- [Dahua guide](../en/door-station-dahua.md)
- [Yeastar VTH guide](https://help.yeastar.com/en/p-series-appliance-edition/integrations/dahua/register-dahua-vth-indoor-monitor-to-yeastar-p-series-pbx-system.html)
