# TalkOps 1.8.7 release

## Task state

- Task ID: 20261023-codex-release-1.8.7
- Date: 2026-10-23, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.7 (merged and removed from origin)
- Base: c093fa8dddd40a14823d9e6ed41922e0e38efc74
- Final candidate: b310b63c1a934cd44a0371d0dc091f374094f5a5
- Main/release: a2478c81877eafce8bd1a40361c84ce097a72fa6
- Integration: https://github.com/itsh-neumeier/talkops/pull/64
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.7

## Goal and result

Publish all open ready contributions in the user-selected version 1.8.7.
Both ready contributions are merged with full source history:

| Contribution | Exact head | Result |
| --- | --- | --- |
| Claude Code PR62: UniFi voice SSID writes | c053920fef62cb5368acaee4e5ae9a3a2fac44db | Included and merged |
| Claude Code PR63: handset roaming and band protection | 19e146132bf07303909c56c36c5d92c6c1515550 | Included and merged; depends on PR62 |

No open Codex contribution or other agreed missing handoff. Draft PR61 remains
review-only and in progress at 16c6c45a61655a9ff215c350ac47d499d15dae9f. It is
excluded and preserved. Neither ready source was in the original main.
PR63 was retargeted from the PR62 branch to main after both were integrated.
Both sources closed as merged at 2026-10-23T10:33:13Z. Both source heads are
ancestors of main. The final candidate and main trees are identical.

## Changes and decisions

- Merge the previous 1.8.6 audit, PR62, then PR63. No merge conflicts.
- Set Cargo workspace, seven workspace lock entries and npm root versions to
  1.8.7. Cargo update --workspace --offline and npm version introduced no
  dependency version upgrades. Add combined English changelog notes.
- Update active EN/DE installation, update, Portainer and reference examples.
  Correct privacy text in both survey guides: band protection reads known
  handset bands outside an active survey. No feature code changed by the release manager.
- Use the explicit user-selected 1.8.7. Additive features preserve default
  behavior with UniFi writes off. The source's suggested minor version does
  not override this explicit current target.
- Do not publish the live Wiki or claim hardware acceptance.

## Checks

- Local cargo fmt passed. Web lint/check passed with zero errors/warnings;
  45 tests and build passed. Reused installed locked dependencies; CI used npm ci.
- All 13 docs tests and Wiki --check passed: 75 pages and 660 local links.
  Four Compose variants, provisioning blob hashes and both derivative checks passed.
- All 53 migration IDs are unique and ordered. The 51 released SQL files are
  unchanged. All 2,598 EN/DE keys and placeholders match. ADR IDs/links are valid.
- Manual integration review: permissions, confirmation, allowlist-only writes,
  secret handling, rollback conflicts, survey gates, handset bands and API/UI contracts.
- All 16 final candidate checks passed: PR CI39937648080/E2E39937648166 and
  push CI39937638952/E2E39937638882. Earlier fd71fc2 runs were superseded by
  the privacy correction. Cancelled runs were not used as final release gates.
- Exact-main CI39939922388 and E2E39939922335 passed before the tag push.
  Rust: 475 passed, one existing model-dependent test ignored. Web: 45 tests
  and 16 existing browser suites. Docs: 13 tests. DB/LDAP, recovery, image/storage
  smoke, fax G.711/T.38 received PDFs, SIPp and browser softphone passed.
  Server and worker smoke checks displayed 1.8.7.
- Source local full tests had the known root-only readonly fax failure, because
  root bypasses 0555. Final CI passed unprivileged; no release test was waived.
- Full Linux/Rust services, image and SIPp/browser tests were not run locally.
  Linux Docker was unavailable; CI supplied evidence. CodeRabbit CLI was unavailable.
- STE: repository clarity rules applied. Full standard unavailable; language review pending.

## Publication and artifacts

Fresh guards verified unchanged main, both source ancestors, identical candidate
and main trees, matching versions, only unchanged draft61 open and successful
latest exact-main gates. New annotated v1.8.7 was pushed once on the release commit.
Tag object: eb5e83e9c326876b8e02a15b1d806e5a06bd7f3a.
Release workflow39942397221 passed. Stable GitHub release published at
2026-10-23T11:03:04Z. Tag CI39942397201 and FreeSWITCH39942397198 passed.
Additional tag E2E39942397244 also passed. All final tag workflows are successful.

Docker buildx imagetools verified all versioned manifests and both platform
configs. All six OCI version labels equal v1.8.7; revision labels equal the
release commit. Tags 1.8 and latest have the same platform digests as 1.8.7.

| Image | Platform | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:13a4cf3e509e695b2450f0a299e710737d6fe47bf395fa5676decd3c51d89469 |
| talkops-server | arm64 | sha256:4f95d874c12d903b82f0849de276d7680be621a14ed6f57986ef165368105258 |
| talkops-media-worker | amd64 | sha256:9830dae5c549cfb95420401a211e888b90c29e230a33cbb9b00c4151497bc8f5 |
| talkops-media-worker | arm64 | sha256:de7bf50e18e2d61bac02ae987fb16b6c7054914e74c7cd3f3c9e497a2364317f |
| talkops-freeswitch | amd64 | sha256:1289df44ab13bbb87f86773992c59659fa515abf6a86423ec572eb64537c377e |
| talkops-freeswitch | arm64 | sha256:27963668b60e087fb0de9fbe48699bf581f101d4ece1c12b6e584c0564dd2db4 |
## Cleanup and open work

Removed unchanged merged remote branches ccr-74ace6e8-93dh09,
claude/wifi-handset-roaming and codex/release-1.8.7 with atomic expected-head
guards. Preserve draft61 branch claude/wonderful-fermat-6iar0b, closed unmerged
PR36 branch claude/vigilant-tesla-ef0027, and unmerged development-prompt branch
claude/charming-bell-fh6q3y. Do not delete foreign worktrees or primary files.
Archive the task-only record under audit/release-1.8.7. Do not push the merged
integration branch again. This audit does not change released application code.

Back up and apply 20261108000001_unifi_wifi_changes and
20261108000002_wifi_handset_changes in order. Keep server/worker together.
New sensitive permission: unifi.write. Handset changes also need phones.manage.
UniFi writes need the switch, voice SSID, valid version test and confirmation.
They can briefly disconnect SSID clients. Active surveys block changes.
No new dependency, container capability or Wi-Fi credential provisioning.

Hardware acceptance remains separate: real Network 11.x field/body behavior,
key rights and disconnect time; AX firmware and threshold effect after check-sync.
Handset band parameter mapping is unconfirmed and is not provisioned.
The live Wiki remains its separate source 9010545 snapshot at master
7fd1c4108386fd4cf3180756f78c86774e42dd2a; no new Wiki or live deployment is claimed.

## Next action

None for this release. The final record is archived under audit/release-1.8.7
and linked from PR64 and the release.
Hardware acceptance, draft61 and future Wiki publication are separate follow-ups.

## References

- https://github.com/itsh-neumeier/talkops/pull/62
- https://github.com/itsh-neumeier/talkops/pull/63
- https://github.com/itsh-neumeier/talkops/pull/64
- https://github.com/itsh-neumeier/talkops/pull/61
- docs/ai-memory/20261022-claude-unifi-voice-ssid-write.md
- docs/ai-memory/20261023-claude-wifi-handset-roaming.md
- docs/ai-memory/20261022-codex-release-1.8.6.md
- docs/release-workflow.md
