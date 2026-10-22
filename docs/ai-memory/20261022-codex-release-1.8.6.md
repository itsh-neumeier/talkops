# TalkOps 1.8.6 release

## Task state

- Task ID: 20261022-codex-release-1.8.6
- Date: 2026-10-22, Europe/Berlin
- Tool: Codex
- Status: Completed
- Release branch: codex/release-1.8.6 (merged and removed from origin)
- Base: b1fa28350dc1830e222a27ee770641edb1935ccc
- Final candidate: e2ff772ce0fb9a988244fb7528f0760b7f382c96
- Main/release: c093fa8dddd40a14823d9e6ed41922e0e38efc74
- Integration: https://github.com/itsh-neumeier/talkops/pull/60
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.6

## Result and scope

Publish all open, ready contributions in the user-selected version 1.8.6.
Preserve both source histories through merge commits.

| Source | Head | Result |
| --- | --- | --- |
| Codex PR58 | 60ff55f0326adc501d5b3c028c7748dfe8c09ce8 | Merged: bilingual handbook and Wiki generator |
| Claude Code PR59 | 26cd5d0fc9c121512b2c59e8bb69d7f4be3c87ca | Merged: Wi-Fi voice survey stages 1-3 |

No agreed contribution or dependency is missing. Stage 4 UniFi writes and
handset roaming provisioning are a separate source follow-up. PR59 suggested
a minor release. The user's explicit current target selected 1.8.6.
PR60 merged at 2026-10-22T13:10:49Z. Both source PRs became merged three seconds
later. The final candidate and main have identical trees. Both source heads
are ancestors of main. The previous release audit is included.

The new draft PR61 appeared at 2026-10-22T17:38:03Z, after publication.
It contains an optimization proposal at 16c6c45a61655a9ff215c350ac47d499d15dae9f.
It is not in this release. Preserve its reused Claude branch.

## Integration changes

Keep the new README layout and add survey links in both languages. Add the
survey guide pair, navigation, three UI routes and coverage test counts to
the handbook manifest and indexes. The source now has 35 topic pairs and
75 generated pages. Update active installation, Portainer, update and reference
examples in both languages to matching 1.8.6 config tags and image versions.
Keep historical fax notes unchanged. No feature code was changed.

Set Cargo.toml, seven workspace lock entries, npm root versions and changelog
to 1.8.6. No dependency version upgrade was introduced. The new talkops-unifi
workspace crate uses already locked dependencies.

## Checks

- Local cargo fmt passed.
- Local web lint/check passed with zero errors/warnings. All 40 tests and build passed.
  Reused installed locked dependencies. CI used a clean install.
- All 13 docs tests and Wiki --check passed: 75 pages and 654 local links.
  The first combined coverage test failed on its old hardcoded topic count.
  Updated the count and new route coverage, then reran all 13 tests successfully.
- Four Compose variants, provisioning reference hashes and both generator
  --check commands passed. Version-only lock changes were verified.
- All 51 migration IDs are unique and ordered. The previous 49 SQL files are unchanged.
  All 2,470 EN/DE keys and placeholders match. ADR links are valid.
- Manual integration review covered navigation, roles, tenant access, encrypted
  API keys, verified TLS/pins, bounded GET-only requests, range audio and contracts.
- Final candidate: PR CI 39781490043/E2E 39781490052 and push
  CI 39781481681/E2E 39781481705 all passed. All 16 final checks were green.
- Earlier candidate 14513c8 CI passed. Its pending E2E runs were cancelled by the
  final installation-example correction. They are superseded, not release gates.
- Exact-main CI 39784592529 and E2E 39784592569 passed before the tag push.
  Rust: 442 passed and one existing model-dependent test ignored.
  Web: 40 tests and 16 existing browser suites. Docs: 13 tests.
  DB/LDAP, backup/recovery, storage/version smoke, image builds, fax fixtures,
  G.711/T.38 received PDFs, SIPp and browser softphone passed.
  Server and worker smoke output displayed version 1.8.6.
- Full Linux/container/SIPp/browser checks were not run locally. The local Linux
  Docker daemon was unavailable and C: had less than 1 GiB free. CI supplied evidence.
- CodeRabbit CLI was unavailable. No CodeRabbit review ran.
- Repository clarity rules were applied. The full STE standard was unavailable;
  the full language review remains pending.

## Publication and artifacts

Fresh guards verified unchanged main, both source ancestors, matching versions,
identical candidate/main trees, empty open scope and successful exact-main gates.
The new annotated tag v1.8.6 was pushed once. It peels to the main/release commit.
Tag object: 7e5720a3495dce1c87ccac042700973313b5d204.
Release workflow 39822855165 passed. GitHub published the stable release at
2026-10-22T17:37:10Z. The latest stable release is TalkOps 1.8.6.
FreeSWITCH tag workflow 39822855715 passed. Additional tag CI 39822855438 and
E2E 39822855179 also passed. All final tag workflows are successful.

Docker buildx imagetools verified each versioned manifest and both platform
configs. All six version labels equal v1.8.6. All six revision labels equal
c093fa8dddd40a14823d9e6ed41922e0e38efc74. Tags 1.8 and latest contain exactly
the same amd64/arm64 platform digests as 1.8.6.

| Image | Platform | Digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:6dfa860149d33e8766daeec986032d0b11a4000f250a9e3a22b2a7b7620ac589 |
| talkops-server | arm64 | sha256:bed238d8895091d96c63577a4c171ce69ca840e34d18c8b529040166591dc506 |
| talkops-media-worker | amd64 | sha256:1ea4866cc16d5fa4bce81af45b4ea0c19264e90d5b58f4574a8e7c1665be1713 |
| talkops-media-worker | arm64 | sha256:4482860e604138e61f0dbf9bd459ba214c8f25f94b289f56774e103dbcf02fcb |
| talkops-freeswitch | amd64 | sha256:5ed159bd0cc80e6c8c10528d307518ba18970ae247787a8bcb6ebf2a200b1c1a |
| talkops-freeswitch | arm64 | sha256:e0fa6084ecd66f8a8c897bd856bfcf033a275bb1df1f668e2ea7201860427f04 |

## Cleanup

Removed the unchanged remote Codex source and integration branches through
an atomic, expected-head guarded deletion. The first three-branch deletion
was rejected because Claude pushed the new PR61 commit between read and push.
No branch was removed by that rejected atomic operation. Rechecked and preserved
the changed branch claude/wonderful-fermat-6iar0b. Also preserved the closed,
unmerged PR36 branch claude/vigilant-tesla-ef0027 at 37f8a18aa2971c1a570821d150dcf0941fae12eb.
Foreign worktrees and primary untracked files remain untouched.
Archive the final task-only record under audit/release-1.8.6. Do not push the
merged integration branch again. The audit does not change the release tree.

## Upgrade notes and separate follow-ups

Back up and apply 20261107000001_unifi_connections and 20261107000002_wifi_surveys
before the new binary. Keep server/worker versions together. Feature code *39
is reserved for armed surveys. New permissions are wifi_survey.run,
wifi_survey.manage and unifi.manage. No new volume, capability or environment
variable is required. Survey recordings and samples default to 30-day retention.
MOS values are estimates. Downlink results depend on handset RTCP.
Real UniFi/handset acceptance remains pending. Record Network version and
classic stat/sta API-key compatibility. Mock and CI checks do not certify hardware.

The live Wiki remains its separate snapshot from source 9010545, with 73 files.
Read-only remote check found master 7fd1c4108386fd4cf3180756f78c86774e42dd2a.
This release includes repository sources/generation for 75 pages. It does not
republish the Wiki. No live deployment or hardware acceptance is claimed.

## Next action

None for this release. Archive this record under audit/release-1.8.6 and link
it from PR60 and the release. Hardware acceptance, draft PR61 and Wiki publication
are separate follow-ups.

## References

- https://github.com/itsh-neumeier/talkops/pull/58
- https://github.com/itsh-neumeier/talkops/pull/59
- https://github.com/itsh-neumeier/talkops/pull/60
- https://github.com/itsh-neumeier/talkops/pull/61
- https://github.com/itsh-neumeier/talkops/actions/runs/39784592529
- https://github.com/itsh-neumeier/talkops/actions/runs/39784592569
- https://github.com/itsh-neumeier/talkops/actions/runs/39822855165
- docs/ai-memory/20261020-codex-readme-wiki-proposal.md
- docs/ai-memory/20261021-codex-wiki-publication.md
- docs/ai-memory/20261022-claude-wifi-voice-survey.md
- docs/ai-memory/20261019-codex-release-1.8.5.md
- docs/adr/0041-wifi-voice-survey-and-unifi-correlation.md
- docs/release-workflow.md
