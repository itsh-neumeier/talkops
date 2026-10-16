# TalkOps 1.7.13 release

## Task state

- Task ID: 20261015-codex-release-1.7.13
- Date: 2026-10-15, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: `codex/release-1.7.13`
- Base commit: `6276e7d5e1b3bbe2a97e0fe237758b3f3a0e44b4` (remote main, 1.7.12)
- Integration PR: [#28](https://github.com/itsh-neumeier/talkops/pull/28)
- Final summary reference: `audit/release-1.7.13` (a non-release tag)

## Goal

Complete and integrate all open PRs for 1.7.13. Check the final candidate and
resulting main commit. Publish and verify the release. Keep only `main` on
GitHub after safe cleanup of unchanged, fully integrated source branches.

## Result

Both source PRs and integration PR #28 are merged. Main is
`702507b39fb1cf80db4ce5a0f9dd3d339b9da68c`; its tree equals tested candidate
`5f9116b6b391d0141439187048df8b7c2118c523`. Both source heads are ancestors.
Neither head was in the base. No contribution is omitted or pending.
No separate Codex feature PR is assigned. Annotated tag `v1.7.13` was pushed
once on the tested main commit. Its tag object is
`25ecd89dcc546293041ad79d08161c09ae9a0969`. Release workflow 38752127330
passed. [TalkOps 1.7.13](https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.13)
was published at 2026-10-15T14:37:06Z. It is neither a draft nor a prerelease
and is the latest GitHub release. Its remote tag resolves to the tested main.

All three versioned manifests and six OCI configs are verified. Each image
contains amd64 and arm64. Every config reports `v1.7.13` and the exact release
commit as its revision. Moving `1.7` and `latest` tags match these digests.

| Image | amd64 digest | arm64 digest |
| --- | --- | --- |
| `talkops-server` | `sha256:7a49d52343b11ceb9680ea222620f448120ed49f3cd121a5df50799bf3a79581` | `sha256:a9010283f7dbaa2926d5ab93882de698b2a8baab43c69295ca8d21cad6a21b58` |
| `talkops-media-worker` | `sha256:5715c4fc6594c02af9c4d43b0039b056fb86266b5d018410b9350d049be6e182` | `sha256:700bf4e30b1b5ab5b4a92b7e8379b1f018931853d64e8c59f6ff084aaac2a8f7` |
| `talkops-freeswitch` | `sha256:056388e8c4b3d2704d850bff4331e9baad86014da4207e81f12b3ef6186ad500` | `sha256:cdd3c195d873adda55bd5db3a6514384cc167f8c5cb8ec01753b6c183cb32d9b` |

The unchanged source and release remote branches were deleted in one atomic
push after ancestry and head checks. GitHub has only `main` and no open PR.
Foreign local worktrees, branches and unpublished primary-checkout work remain.
The final audit changes only this summary. Its tag does not run Release.
Audit CI/E2E runs check this documentation snapshot; they are not release
gates. The owned release worktree is detached at the audit commit, and its
local release branch is removed.

| Source | Contributor | Exact head | Dependencies | Migration IDs |
| --- | --- | --- | --- | --- |
| [#26](https://github.com/itsh-neumeier/talkops/pull/26), shared phonebook numbers | Claude Code | `83574493d75eb23945658b88218ca4367489f77a` | None | None |
| [#27](https://github.com/itsh-neumeier/talkops/pull/27), device media and door ringing | Claude Code | `652a9e7f3f7475e2c218302545f6722820e494a9` | None | `20261031000001` |

PR #27's body names the older head `088d744`. Its final commit only links
the PR in its summary. The selected actual head has all green checks.

## Changes

- PR #26: choose a contact name for a shared number, prefer the local
  extension's phonebooks, show all matches, and link to highlighted entries.
  Add API fields, EN/DE text, browser tests, user docs and ADR 0027.
- PR #27: device video modes, G.711 audio, caller codec inheritance,
  parallel door ringing, diagnostics RTP counters, migration, guides and tests.
- Integration: keep the phonebook ADR as 0027. Rename the concurrent media
  ADR to 0028. Update its code/doc links and the unreleased migration comment.
  Keep the original contributor summaries as source-history records.
- `crates/talkops-api/src/active_calls.rs`: add `video: None` to the new
  phonebook test device, to match the Device field added by PR #27.
- Release metadata: set Cargo workspace and web root versions to 1.7.13.
  Combine both contributions and upgrade notes in `CHANGELOG.md`.
- Merge `audit/release-1.7.12` to preserve the final previous release report.
  The owned checkout had a previous report copy identical to that remote
  archive. Save a temporary copy and clear only that published local change
  before switching branches. Import no foreign uncommitted work.

## Decisions

- Use the requested patch version 1.7.13. API fields are additive. Existing
  device defaults, SIP names and passwords stay. New settings are optional.
- The `ambiguous` API field stays. A selected phonebook name is not proof of
  identity. The matches card shows the other possible entries.
- Migration `20261031000001_device_video_and_g711` follows the existing
  migration IDs. No released migration changes. The PostgreSQL enum addition
  is not used as a new value in the same migration transaction.
- Door calls lose SIP early-media previews. On video calls, callers without
  a fixed audio profile inherit both codecs from the answering device.
- Do not claim a hardware fix for the VTO-to-VTH missing picture. The source
  handoff explicitly leaves its cause open and supplies settings and counters.
- Preserve the primary checkout's unpublished commit and three untracked
  Dahua summaries, all foreign local branches, and foreign worktrees.
- Target `main` is verified. Merge commits are permitted. No branch protection
  or ruleset is active. Do not force push or move existing release tags.

## Checks

- Passed: #26 PR CI 38735445206 and E2E 38735444110;
  push CI 38735404379 and E2E 38735404472, on the selected source head.
- Passed: #27 PR CI 38740919360 and E2E 38740918846;
  push CI 38740911428 and E2E 38740911576, on the selected source head.
- Passed locally: `cargo fmt --all --check`; `npm ci`, web lint, check
  (0 errors, 0 warnings), tests (6 passed), and production build.
- Passed: lock-file comparison finds no external dependency changes;
  DE/EN have the same 1,354 keys; all 37 migration IDs and 28 ADR numbers
  are unique; released migration contents are unchanged; ADR links resolve.
- Not run locally: Rust compilation and full DB/LDAP tests. The Windows GNU
  toolchain lacks `dlltool.exe`. Linux CI must run the complete workspace.
- Not run locally: Compose, container execution, SIPp and browser E2E. The
  Linux Docker daemon is unavailable. Use Linux CI and E2E as the evidence.
- FreeSWITCH source and workflow files are unchanged. E2E builds the image.
- Passed: final candidate PR CI 38745692299 and E2E 38745692279;
  push CI 38745684540 and E2E 38745684301. All 12 final-head checks passed.
  Logs confirm the phonebook priority, media settings and RTP summary tests.
  Image smoke output reports server and media worker version 1.7.13.
- Passed: main tree equals candidate; both source heads are ancestors.
  All three PRs are closed as merged. No open contribution remains.
- The first main CI 38748755531 and E2E 38748755542 were cancelled when
  new push-event runs started on the same commit. Main did not change.
  Replacement CI 38749466584 and E2E 38749466804 both passed on the exact
  main commit. Main container smoke output reports server and worker 1.7.13.
  The latest-main validation gate passed before the tag push.
- Passed before publication: latest-main-run, source ancestry, main SHA,
  open-PR scope, code-version and unused-tag gates.
- Passed: Release 38752127330, all six platform builds, three manifest jobs
  and GitHub release creation.
- Passed: additional tag CI 38752127329 and FreeSWITCH image 38752127153.
- Passed: three direct GHCR verification scripts, all architectures, all
  version/revision labels, and matching versioned/moving platform digests.
- Passed: release metadata, latest release, peeled tag, safe remote branch
  cleanup and final open-PR inventory.
- Passed: additional tag E2E 38752127238 on the exact released commit.
- STE review: manual review with `docs/documentation-style.md`. The official
  full standard is unavailable; full language review remains pending.
- CodeRabbit CLI is unavailable. Focused integration review used the actual
  diffs, API/UI contracts, migrations, ADRs, translations and source tests.

## Dependencies and open work

- Only the stated migration is new. No source dependency or package upgrade.
- The VTO-to-VTH picture cause remains open. Hardware tests must compare the
  main-VTO configuration, video modes, parallel ringing and RTP counters.
- A deployed UI version check is unavailable. Check source versions and CI
  server/worker version output before reporting release success.
- The percent-encoded FreeSWITCH CDR spool import defect remains separate
  work. Preserve `cdr-spool/rejected/` files for later recovery.

## Next action

None for this release. Future release tasks must read the final audit summary.
The stated hardware tests and CDR spool work remain separate development tasks.

## References

- `docs/ai-memory/20261015-claude-shared-phonebook-numbers.md`
- `docs/ai-memory/20261015-claude-door-video-device-media.md`
- `docs/ai-memory/20261014-codex-release-1.7.12.md`
- `docs/adr/0027-shared-phonebook-numbers-in-active-calls.md`
- `docs/adr/0028-device-media-settings.md`
- `docs/release-workflow.md`
