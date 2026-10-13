# TalkOps 1.7.9 release

## Task state

- Task ID: 20261013-codex-release-1.7.9
- Date: 2026-10-13, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.7.9 (removed after verification)
- Base: 8fe0192a8067d0dcecb005075b876ea0f24bd7b3 (remote main, 1.7.8)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/18
- Final candidate: a999b24d7aed42679daf33008230b66d304952b4
- Release commit: bfc46429f0889ceec79e72f45b49c6ec79dc9f26 (main)
- Annotated tag: v1.7.9; object 8e88c3f7fa237dbbe523119ccf3aab04ee494843
- Published: 2026-10-13T17:33:51Z
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.9

## Goal and result

Publish all open contributions. The user first named 1.7.8, which was already
published. The user then selected 1.7.9. The 1.7.8 tag stayed fixed.
Claude Code supplied both feature PRs. Codex supplied integration, combined
regressions, release metadata, publication and verification. No planned PR
was omitted or blocked. No Codex feature PR was open.

| PR | Contribution | Exact source head |
| --- | --- | --- |
| #16 | Video policy and prefixed CDR names | d49322b46483e26c9ab687241d81d1af6444913a |
| #17 | SIP names and DTMF unlock codes | f7f345d3976dabdab45b95e73832618853d18c94 |

Both sources were ready with successful exact-head CI/E2E. #16's handoff named
an older head; its last commit only linked the summary. Merge commits retain
both histories and the completed 1.7.8 audit. The primary local docs commit
397fc83ec5a0c1032e3c8b944eab337b3ef495f8 had no PR handoff. It was preserved
and excluded. No uncommitted changes were copied from another worktree.

## Integration decisions

- Resolve two dialplan conflicts by retaining video policy and DTMF leg markers.
- Test active video codecs and the answering-leg marker on the same ring call.
  Test CDR parsing with both exported codec and door variable names.
- Review API/UI controls, encryption, audit output and external-number lists.
- Verify 34 unique migration IDs and 1,181 matching unique DE/EN keys.
  New 20261029000001_door_unlock_codes follows 20261028000001.
  Released SQL was not edited.
- Set Cargo workspace and web root versions to 1.7.9 with package tools.
  External dependency locks stayed unchanged. Add combined English release notes.
- Review the pinned FreeSWITCH v1.10.12 event command. It adds event types;
  the DTMF watcher preserves the SIP guard's custom-event subscription.

## Checks

All final candidate and main checks passed before the tag push. The merged
main tree equals the checked candidate tree. Both source heads are ancestors.
The final gate checked main, open PRs, code versions and unused tag/release.
Pushed the tag once on the checked main commit through the existing Release workflow.

| Revision / workflow | Run IDs | Result |
| --- | --- | --- |
| #16 source PR / push CI | 38399483901 / 38399477806 | Passed |
| #16 source PR / push E2E | 38399483961 / 38399477833 | Passed |
| #17 source PR / push CI | 38427191836 / 38427181749 | Passed |
| #17 source PR / push E2E | 38427191793 / 38427181746 | Passed |
| Final candidate PR / push CI | 38434186111 / 38434177379 | Passed |
| Final candidate PR / push E2E | 38434186011 / 38434177610 | Passed |
| Resulting main CI | 38436597176 | Passed |
| Resulting main E2E | 38436597299 | Passed |
| Tag CI | 38438762591 | Passed |
| Tag FreeSWITCH image | 38438762657 | Passed |
| Tag E2E | 38438762680 | Passed |
| Release | 38438762596 | Passed |

CI covers Rust fmt/clippy/locked DB/LDAP tests, web lint/check/build and browser
checks, Compose, host recovery and server/worker images. E2E builds the stack,
runs SIPp audio/media and video-policy scenarios, and calls through the browser
softphone. Main logs show all E2E checks passed. Combined door video/DTMF and
prefixed CDR regressions passed. Server and worker smoke tests report 1.7.9.
FreeSWITCH sources/workflow were unchanged; no separate branch-path job applied.

Local npm ci/lint/check, three Node tests/build, Rust format, whitespace,
source ancestry and dependency checks passed. Normalized CRLF only in the owned
checkout. Full Linux DB/LDAP/stack tests used CI because no local daemon was
available. Deployed UI and real Dahua DTMF hardware were unavailable. No actual
door was operated. Manual integration review; CodeRabbit CLI unavailable.
The full official STE standard was unavailable; full language review is pending.

## Published artifacts

GitHub release is public, not a draft or prerelease. Its tag resolves to the
release commit. The Release workflow passed all six native image builds and
three manifest jobs. Registry inspection confirmed linux/amd64 and linux/arm64
for all three images. Each image config has version v1.7.9 and the exact release
revision. Tags 1.7 and latest have the same platform digests as 1.7.9.

| Image | amd64 platform digest | arm64 platform digest |
| --- | --- | --- |
| talkops-server | sha256:edb6cd231e558a16d04326d70fa28583c4cf4d58e8af9064db89e663e8361d8c | sha256:22956a74ef2799e2ac4d8a9040b1fffa58145c7d12f38469a055335cac0e4b5f |
| talkops-media-worker | sha256:9dfff163c504d7212b7690047d0a517fe3c33e95be6dbebdb914590527713176 | sha256:c95e0555b6d469da13fcfa8a9c6e85b3787702a6542fd4029998ad08d4886afd |
| talkops-freeswitch | sha256:999ba3ce8a52c19a539863752aaf398f9c0655163640bbd9553d9aab3eaa8005 | sha256:f8a538cfdacf1aa2bf4cf57d6fc7691af1d1d2b2bcf0e5d5ceea5fc2f55a3775 |

## Upgrade and remaining work

Restart FreeSWITCH to load the internal audio-only and external late-negotiation
profiles; active calls end. Enable video only on the trunks that need it.
Existing SIP names stay until renamed. Update device credentials and register
again after a rename. Dahua SIP Server Username/Password must match the device
Username/Password. No production stack was restarted or changed in this task.

External callers require the documented number allow list. Caller ID can be
falsified. Queue/menu loopback legs may not inherit the unlock marker. Real
Dahua/DTMF hardware remains unverified.

#16 explicitly excludes raw percent-encoded CDR spool import. There is no
assigned source PR for that defect; it remains separate development work.
The HTTP CDR parser fix does not recover rejected spool files. Preserve
cdr-spool/rejected. This limit is recorded in the changelog and release notes.

## Cleanup and durable state

Deleted only the two source branches and the integration branch after validating
unchanged heads and ancestry. Atomic remote deletion succeeded. Remote has only
main; no open PR remains. Preserve foreign local branches/worktrees and the
primary unpublished docs commit. Saved this final summary in a docs-only audit
commit with annotated audit/release-1.7.9. Detached the owned checkout and removed
only its local release branch. Audit tag checks are separate from released code.

No release work remains. A later release must use a new version and a fresh
remote-main scope review. No automation or development-chat message was created.

## References

- [Integration PR](https://github.com/itsh-neumeier/talkops/pull/18)
- [Release](https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.9)
- [Release workflow](https://github.com/itsh-neumeier/talkops/actions/runs/38438762596)
- [Previous release](20261013-codex-release-1.7.8.md)
- [Video policy](../adr/0025-video-policy.md)
- [Door station architecture](../adr/0013-tuersprechstellen-dahua.md)
- [FreeSWITCH event source](https://github.com/signalwire/freeswitch/blob/v1.10.12/src/mod/event_handlers/mod_event_socket/mod_event_socket.c)
