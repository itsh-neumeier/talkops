# TalkOps 1.7.11 release

## Task state

- Task ID: 20261014-codex-release-1.7.11
- Date: 2026-10-14, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.7.11
- Base: 3cad0180d34fedaa554339afaa67a9e56fed0ab4 (main, 1.7.10)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/22

## Goal and scope

Publish all open PRs as 1.7.11. The only source is ready Claude Code PR #21:
station video codecs and ASCII names on door calls. Exact source head:
f533c65b73994a4de7ac858a824d7ee3e3240487, branch claude/zen-wright-tuunvi.
No Codex feature PR or planned contribution is missing. No external dependency
or migration. Existing source histories and the completed 1.7.10 audit are
preserved through conflict-free merge commits.

## Changes and decisions

- Integrate station-specific H264/VP8 selection and ASCII names for calls from
  stations and direct calls to station extensions. Keep full caller names in CDR.
- Codex integration corrections: ignore port-zero media and unadvertised payload
  mappings; prevent ring-group prefixes from reintroducing non-ASCII names on
  door calls. Extend unit and DB/API regressions for these cases.
- Mark the suspected UTF-8 ACK cause as unconfirmed in EN/DE troubleshooting,
  code comments and release notes. No physical hardware result is claimed.
- Set Cargo workspace and web root versions to 1.7.11 with package tools.
  External dependencies remain unchanged. No FreeSWITCH source patch.
- 36 unique migration IDs; no new or changed SQL. DE/EN retain 1,338 matching keys.

## Checks

- Passed: source PR CI 38529444449 and E2E 38529444294 on the exact head.
  Push CI 38529416822 and E2E 38529416677 also passed.
- Source reports a single earlier door_station event-wait failure. It did not
  recur in three single and two complete runs. Do not ignore a final failure.
- Passed: local Rust fmt/whitespace, migration/translation and lock checks.
- Passed: local npm ci/lint/check (zero errors/warnings), six Node tests/build.
- Pending: final candidate CI/E2E, resulting main CI/E2E,
  Release workflow, manifests, labels and moving 1.7/latest tags.
- Linux PostgreSQL/LDAP/Compose/images/SIPp/browser checks use CI because the
  local Docker daemon is unavailable. Real Dahua VTO/VTH and deployed UI are
  unavailable. Manual integration review; CodeRabbit CLI unavailable.
- Full official STE standard unavailable; full language review pending.

## Dependencies and open work

No migration, password change or production deployment. Update the server for
new calls. Hardware must confirm VTO-to-VTH video and VTH-to-VTO calls longer
than 30 seconds on VTO4202F-P-S2 V4.600.0.3 / VTH5422H V4.800.0.2.
Video direction and rtcp-fb behavior stay unchanged. The raw percent-encoded
CDR spool import defect remains separate work; preserve cdr-spool/rejected.

Preserve the primary unpublished docs commit 397fc83 and three untracked Dahua
summaries; no source PR hands them off. Other worktrees remain unchanged.
No development-chat message or automation is requested or created.

## Next action

Save the candidate and PR ledger. Wait for final checks, merge, then verify
resulting main CI/E2E before pushing unused v1.7.11 once. Verify Release and all
three amd64/arm64 manifests, labels and moving tags. Remove only integrated,
unchanged remote branches. Save final audit and detach the owned checkout.

## References

- [Source PR](https://github.com/itsh-neumeier/talkops/pull/21)
- [Source summary](20261014-claude-dahua-door-video-names.md)
- [Previous release](20261014-codex-release-1.7.10.md)
- [Video policy](../adr/0025-video-policy.md)
- [Door vendors](../adr/0026-door-station-vendors.md)
