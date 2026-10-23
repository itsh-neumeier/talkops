# TalkOps 1.8.7 release

## Task state

- Task ID: 20261023-codex-release-1.8.7
- Date: 2026-10-23, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: codex/release-1.8.7
- Base commit: c093fa8dddd40a14823d9e6ed41922e0e38efc74
- Related PR: Pending

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
- Local and combined candidate checks: pending.
- Full Linux/container/SIPp/browser checks will use CI. Linux Docker is unavailable locally.
- CodeRabbit CLI unavailable. Manual integration review in progress.
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
