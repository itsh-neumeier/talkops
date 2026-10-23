# TalkOps 1.8.7 release

## Task state

- Task ID: 20261023-codex-release-1.8.7
- Date: 2026-10-23, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.7
- Base commit: c093fa8dddd40a14823d9e6ed41922e0e38efc74
- Related PR: https://github.com/itsh-neumeier/talkops/pull/64

## Goal

Integrate all open ready contributions and publish the user-selected 1.8.7.

## Result

Both ready contributions are merged with source history:
- Claude Code PR62: c053920fef62cb5368acaee4e5ae9a3a2fac44db, UniFi SSID writes.
- Claude Code PR63: 19e146132bf07303909c56c36c5d92c6c1515550, handset roaming and band protection.
PR63 is stacked on PR62. Both final heads have 16 green checks. Neither was
in main. There is no open Codex contribution. Preserve the previous release audit.
Draft PR61 remains a review-only, in-progress proposal at 16c6c45a61655a9ff215c350ac47d499d15dae9f.
It is not a ready handoff and is excluded. No other agreed handoff is pending.

## Changes

- Merge PR62, then PR63. No conflicts or feature code changes by the release manager.
- Version Cargo workspace, seven workspace lock entries and npm roots to 1.8.7.
  Cargo update --workspace --offline and npm version changed no dependency versions.
- Add combined English changelog notes and update active EN/DE install examples.

## Decisions

- Use the explicit user-selected version 1.8.7. The additive changes preserve
  default behavior; write access is off by default. PR62's minor suggestion
  does not override this explicit release target.
- Preserve draft PR61 and closed, unmerged PR36. Do not change foreign worktrees.
- New migrations 20261108000001 and 20261108000002 must follow the 51 released files.
- UniFi/AX hardware acceptance remains separate, as stated by both ready handoffs.
  Do not claim hardware support as tested. No handset band or Wi-Fi credentials.
- Do not republish the live Wiki as part of this application release.

## Checks

- Source PR62 and PR63 final CI/E2E passed. Source local root-only readonly fax
  test failed because root bypasses 0555. CI runs unprivileged and is green.
- Local: cargo fmt, web lint/check (zero errors/warnings), 45 tests/build passed.
  Installed locked dependencies reused; CI performs clean npm ci.
- Local: 13 docs tests; 75 pages/660 links; four Compose variants; provisioning
  hash and derivative checks passed. Metadata changed only seven workspace and npm
  root versions. 53 migration IDs unique/ordered, 51 released SQL files unchanged.
  All 2,598 EN/DE keys/placeholders match. ADR IDs/index links are valid.
- Full Linux/container/SIPp/browser checks will use CI. Linux Docker is unavailable locally.
- CodeRabbit CLI unavailable. Manual integration review completed: permissions, confirmation, allowlist-only
  writes, secrets, rollback conflicts, survey gates, handset bands and API/UI contracts.
- STE: repository clarity rules applied. Full standard unavailable; full review pending.

## Dependencies and open work

PR63 depends on PR62 and its migration. New permission unifi.write; handset
changes also need phones.manage. Back up, migrate in order and keep server/worker
together. A UniFi write can briefly disconnect SSID clients. Confirm real Network
11.x PUT fields/body, key rights and disconnect time; AX firmware and threshold
activation after check-sync. Hardware tests and live deployment are not claimed.

## Next action

Check the combined candidate, merge the integration PR, verify exact-main CI/E2E,
push one new tag and verify the workflow, release and all platform/tag manifests.

## References

- https://github.com/itsh-neumeier/talkops/pull/62
- https://github.com/itsh-neumeier/talkops/pull/63
- https://github.com/itsh-neumeier/talkops/pull/61
- docs/ai-memory/20261022-claude-unifi-voice-ssid-write.md
- docs/ai-memory/20261023-claude-wifi-handset-roaming.md
- docs/ai-memory/20261022-codex-release-1.8.6.md
- docs/release-workflow.md

## Merged checkpoint

- Final candidate: b310b63c1a934cd44a0371d0dc091f374094f5a5.
- Corrected EN/DE privacy text: band protection reads known handset bands outside
  an active survey. Docs tests and Wiki check passed again. Earlier fd71fc2 runs
  were superseded by this correction; cancelled runs are not final gates.
- PR63 retargeted from the PR62 branch to main after integration. All source
  heads unchanged. PR64 and both sources are merged.
- All 16 final candidate checks passed. PR CI39937648080/E2E39937648166;
  push CI39937638952/E2E39937638882. Rust 475 passed, one existing ignored.
  Web 45 tests, 16 existing browser suites, docs 13 tests, image/version smoke,
  storage, DB/LDAP, recovery, fax G.711/T.38, SIPp/browser softphone passed.
  Server and worker smoke output showed 1.8.7.
- Main: a2478c81877eafce8bd1a40361c84ce097a72fa6. Its tree equals the final
  candidate. Both source heads are ancestors. Source PRs closed at 10:33:13Z.
- Exact-main CI39939922388 and E2E39939922335 are queued. No tag is pushed.
- Do not push the merged release branch again. Save later task-only records
  through audit/release-1.8.7 after publication.

## Publication started

Exact-main CI39939922388 and E2E39939922335 passed before publication.
Required fax G.711/T.38 and all end-to-end checks passed. Fresh pre-tag guards
verified unchanged main, source ancestry, identical candidate/main trees,
matching metadata, only the unchanged draft61 open, and latest successful gates.
New annotated v1.8.7 was pushed once on a2478c81877eafce8bd1a40361c84ce097a72fa6.
Tag object: eb5e83e9c326876b8e02a15b1d806e5a06bd7f3a.
Release39942397221, tag CI39942397201/E2E39942397244 and FreeSWITCH39942397198
are running. Artifact verification is pending.
