# TalkOps 1.7.9 release

## Task state

- Task ID: 20261013-codex-release-1.7.9
- Date: 2026-10-13, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.7.9
- Base commit: 8fe0192a8067d0dcecb005075b876ea0f24bd7b3 (remote main, 1.7.8)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/18

## Goal and scope

Publish all open PRs. The user first named 1.7.8, which is already published.
The user then selected 1.7.9. Keep the 1.7.8 tag fixed. Work only in the owned
release checkout; preserve other worktrees and unpublished work.

| PR | Contributor / change | Exact source head |
| --- | --- | --- |
| #16 | Claude Code: video policy and prefixed CDR names | `d49322b46483e26c9ab687241d81d1af6444913a` |
| #17 | Claude Code: SIP names and DTMF unlock codes | `f7f345d3976dabdab45b95e73832618853d18c94` |

Both are ready, not drafts, with successful exact-head CI/E2E. No Codex feature
PR is open. No planned contribution is missing. #16's handoff names an older
head; the later commit only links the summary. Use the actual checked head.

## Integration changes

- Preserve both source histories and the completed 1.7.8 audit.
- Resolve two dialplan conflicts by keeping video policy and DTMF markers.
- Extend the door API regression to check video codecs and the answering-leg
  marker on the same ring call. Extend CDR parsing to test both exported names.
- Existing encryption, permission and audit tests cover codes and allow lists.
- Set all Cargo and web root versions to 1.7.9 with package tools. No dependency
  upgrade. Write combined English release and upgrade notes.

## Checks

- Passed: source handoff and exact-head CI/E2E reviews. #16 also reports a real
  FreeSWITCH/SIPp video matrix; #17's DTMF path uses synthetic ESL events.
- Passed: 34 unique migration IDs and 1,181 matching unique DE/EN keys.
  New migration: 20261029000001_door_unlock_codes. Released SQL stays unchanged.
- Review: the FreeSWITCH v1.10.12 event command adds subscriptions. The DTMF
  watcher therefore preserves the SIP guard's custom-event subscription.
  Reviewed the pinned upstream mod_event_socket.c source.
- Passed: local npm ci/lint/check (zero errors/warnings), three Node tests/build.
- Passed: Rust format, whitespace, source ancestry and unchanged dependency locks.
- Passed: final candidate a999b24d7aed42679daf33008230b66d304952b4 PR/push CI (38434186111, 38434177379) and E2E (38434186011, 38434177610).
- Pending: target checks and release
  workflow and amd64/arm64 manifest, label and stable tag verification.
- Full Linux DB/LDAP/stack checks use CI. Real Dahua DTMF and deployed UI
  observation are unavailable. No actual door was operated.
- Manual integration review. CodeRabbit CLI unavailable.
- STE review: full official standard unavailable; language review pending.

## Dependencies and open work

Restart FreeSWITCH to load the new SIP profiles. Existing SIP names stay until
renamed; update device credentials and registration after a rename.
Caller ID can be falsified. External DTMF callers require the documented allow
list. Queue/menu loopback legs may not get the marker; real-device tests remain.

#16 explicitly excludes the raw percent-encoded CDR spool import defect.
It has no assigned PR in this batch and remains a separate development task.
Do not claim that rejected files are recovered. Preserve cdr-spool/rejected.
No automation or message to a development chat is authorized or needed.

## Next action

PR #18 merged as bfc46429f0889ceec79e72f45b49c6ec79dc9f26. Its tree equals the checked candidate; both source histories are ancestors. No open PR remains.
Target CI 38436597176 and E2E 38436597299 are queued. Verify the
resulting main CI/E2E before pushing unused v1.7.9. Verify publication and all
three manifests and moving tags, then delete only integrated unchanged branches.
Save the final result in the PR and task summary.

## References

- [Release workflow](../release-workflow.md)
- [Previous release summary](20261013-codex-release-1.7.8.md)
- [Video policy](../adr/0025-video-policy.md)
- [Door station architecture](../adr/0013-tuersprechstellen-dahua.md)
- [FreeSWITCH event socket source](https://github.com/signalwire/freeswitch/blob/v1.10.12/src/mod/event_handlers/mod_event_socket/mod_event_socket.c)
