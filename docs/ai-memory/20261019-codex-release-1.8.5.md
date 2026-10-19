# TalkOps 1.8.5 release

## Task state

- Task ID: 20261019-codex-release-1.8.5
- Date: 2026-10-19, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: codex/release-1.8.5, removed from origin
- Base commit: 488e42a1a5e2853ef1ce5a5e0ada253d97a5c99d
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/57

## Goal

Integrate every open contribution and publish the requested version 1.8.5.

## Result

TalkOps 1.8.5 is the latest stable release. Both open ready PRs are included:
- Codex PR55: a535797c12a26c977111d9f3dc5a174e433d66d6.
- Codex PR56: 823a59faf346a3dbbe8a5b8f4a72216ccda30f07.

Both sources had 14 successful final-head checks. Neither was in main.
No contribution or promised handoff was omitted. No Claude Code contribution
was assigned or pending. PR57 and both sources are merged. Merge commits
preserve both source histories and the previous completed release audit.

## Changes

- PR55: probe fax storage writes, sync, cleanup and free space before upload;
  safe 503/507 errors; retained attachments; permission-protected administrator
  storage check; volume repair instructions and read-only image smoke tests.
- PR56: retain actual block reasons through CDR/spool ingestion; exact tenant
  whitelist before local rules/PhoneBlock; separate permission and atomic audit;
  call views, result filters, totals/statistics and softphone classification.
- Release metadata: Cargo.toml, six workspace Cargo.lock entries, npm root
  versions and CHANGELOG.md. Package tools changed versions only.
- This record: scope, decisions, checks, release commit and artifact evidence.

## Decisions

- Resolve the only merge conflict in DE/EN locale files by retaining both
  independent additions. Verify every changed source translation and both CI
  additions. No other integration correction was required.
- Migration 20261106000003 follows 48 unchanged released SQL files. All 49 IDs
  are unique and ordered. Existing CDR reasons remain null; no history is inferred.
- Keep non-admin grants explicit. Whitelisting does not change historical
  results, PhoneBlock accounts or other routing rules.
- Preserve foreign files/worktrees and closed, unmerged PR36. No force-push
  rewrote shared history. No published tag moved. No dependency version upgrade.
- Publish only after final candidate and exact-main CI/E2E passed. FreeSWITCH
  sources/workflow were unchanged; no separate candidate/main run applied.

## Checks

- Passed locally: Rust fmt; clean web install/lint/check with zero errors and
  warnings, 38 tests/build; four Compose variants; version and migration checks.
- Passed locally: 2,283 matching EN/DE keys and placeholders; every translation
  change from both source heads preserved; 16 browser suites; 39 unique ADRs
  and valid index links. Original provisioning hashes and both generators passed.
- Manual integration review: safe fax errors, probe/partial-upload cleanup,
  volume scope, tenant/role guards, whitelist precedence and atomic audit,
  historical CDR/spool markers, filters/statistics and additive API/UI fields.
- Source55 CI39233215945/E2E39233215937 passed. Source56 final documentation head
  also passed runs39249343901/39249343915/39249349840/39249349914. Its handoff
  referenced earlier runtime checks; final-head evidence was rechecked separately.
- Final candidate 9f1a29ed88e73b1e6dfe65fe7e308f4b337aa7b1 passed all 14 checks:
  PR CI39252972813/E2E39252972792 and push CI39252965249/E2E39252965273.
- Exact-main CI39254950679 and E2E39254950487 passed before tag. Rust: 413 tests
  passed and one existing model-dependent test ignored. Web: 38 tests and all
  16 browser suites passed. Ten fax MIME/IMAPS/conversion fixtures passed.
  Database/LDAP, backup/restore, Compose, image version/storage smoke checks,
  G.711/T.38 multi-page fax transfers and received PDFs passed. The new real
  blocked-call/historical-reason/whitelist-next-call scenario ran. All existing
  SIPp and browser softphone scenarios passed.
- Server and media-worker exact-main smoke tests report 1.8.5.
- Release39256855610, tag E2E39256855675 and FreeSWITCH39256855604 passed.
- Additional tag CI39256855605 passed on the exact release commit. No failed gate, rerun or waiver.
- CodeRabbit CLI unavailable. No CodeRabbit review ran.
- Not run locally: full Linux Rust/services, image/Python/fax/SIPp/browser stack.
  Local Linux Docker daemon unavailable. Complete checks used GitHub CI/E2E.
- STE review: repository clarity rules applied. Full standard unavailable;
  full language review pending, separate from functional release gates.

## Publication

- Final main/release commit: b1fa28350dc1830e222a27ee770641edb1935ccc.
- Main tree equals the tested candidate. Fresh guards checked latest successful
  exact-main gates, both source ancestors, matching versions, empty open scope
  and unused tag. Main was rechecked immediately before the single tag push.
- PR57 merged 2026-10-19T12:48:29Z. PR55/56 became merged through it two seconds later.
- New annotated v1.8.5 object: 0963dfb4500904f26c40be88814c9a7e0cfa0667.
- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.8.5
- Published 2026-10-19T13:10:39Z. Latest stable, not draft or prerelease.
- The existing Release workflow built six images and published three indexes.
  Every index has linux/amd64 and linux/arm64. All six OCI labels match v1.8.5
  and the release revision. Moving 1.8/latest platform digests match 1.8.5.

## Artifact evidence

| Image | Architecture | Published platform digest |
| --- | --- | --- |
| talkops-server | amd64 | sha256:74ac62380503a6ec6f6b6dd724ecdbeaf16cb709d2e57ec82300dae21d3e19a5 |
| talkops-server | arm64 | sha256:fb5930ea8eb92a619f6ec9e9c7467646580beb2bb44231684c8d6b38339ee072 |
| talkops-media-worker | amd64 | sha256:9d2a99b3e3a7664e3707c4cbed3c3a4318dd046247478b4cec1baaeb8cf8aabb |
| talkops-media-worker | arm64 | sha256:8f34ef700bf1c3b5da6269a99c3cfe9beb1cd6cd2925ada3d7af0cb7fb34e239 |
| talkops-freeswitch | amd64 | sha256:c0d90f6d7a25e3e6168531f967237d8062cb06c288c9fdbda36e7583137631f8 |
| talkops-freeswitch | arm64 | sha256:b4829bba8e74c51b40ff9ca119c94b69fd4342191c7b42dd55f541daeafa6ca1 |

## Cleanup

Unchanged integrated PR55/56 and release branches were removed with one atomic
push after publication, artifact and ancestry checks. Expected-head guards
prevented deletion of changed branches. Remote branches left: main and closed,
unmerged PR36's claude/vigilant-tesla-ef0027. Foreign local work is intact.
Archive the final task-only descendant through audit/release-1.8.5. Only this
record differs from released main. Never push the removed batch branch again.

## Dependencies and open work

No missing handoff or software release blocker remains. Back up and apply
20261106000003 before the new binary. Keep images together and reload the UI.
Explicitly grant blocking.allowlist to non-admin roles as needed. Existing
calls retain their original results; no old block reasons are reconstructed.

Deployment repair is still required where old fax mounts are wrong. Server,
worker and FreeSWITCH need the same writable fax volume/path, UID/GID 10001.
Update Compose/Portainer mounts while preserving existing documents and read-only
root protection. An image update alone does not correct old mounts. The admin
check verifies server access only. Operator acceptance must verify actual storage
recovery, outbound/inbound provider delivery and any intended whitelist exception.
No live deployment, production repair or real-provider acceptance was performed.
Prior mail-server and device acceptance limitations remain unchanged.

## Next action

None. Publication, artifact verification and branch cleanup are complete.

## References

- [Source PR55](https://github.com/itsh-neumeier/talkops/pull/55).
- [Source PR56](https://github.com/itsh-neumeier/talkops/pull/56).
- [Integration PR57](https://github.com/itsh-neumeier/talkops/pull/57).
- docs/ai-memory/20261019-codex-fax-send-receive-fix.md.
- docs/ai-memory/20261019-codex-spam-blocklist-visibility.md.
- docs/ai-memory/20261019-codex-release-1.8.4.md; docs/release-workflow.md.

## Final workflow evidence

- [Candidate CI](https://github.com/itsh-neumeier/talkops/actions/runs/39252972813)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39252972792).
- [Exact-main CI](https://github.com/itsh-neumeier/talkops/actions/runs/39254950679)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39254950487).
- [Release](https://github.com/itsh-neumeier/talkops/actions/runs/39256855610),
  [tag CI](https://github.com/itsh-neumeier/talkops/actions/runs/39256855605),
  [tag E2E](https://github.com/itsh-neumeier/talkops/actions/runs/39256855675),
  [FreeSWITCH](https://github.com/itsh-neumeier/talkops/actions/runs/39256855604).
