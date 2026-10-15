# TalkOps 1.7.12 release

## Task state

- Task ID: 20261014-codex-release-1.7.12
- Date: 2026-10-14, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: `codex/release-1.7.12`
- Base commit: `7b6e51f612475fc4473880f8a2f818c49147e9e2` (`origin/main`, 1.7.11)
- Integration PR: [#25](https://github.com/itsh-neumeier/talkops/pull/25)
- Final summary reference: `audit/release-1.7.12` (a non-release tag)

## Goal

Include all open TalkOps PRs in 1.7.12. Check the combined code, merge it,
publish the release, and verify its images. Keep only `main` on GitHub after
publication if the source branches contain no new work.

## Result

Both source PRs and integration PR #25 are merged. Main is
`6276e7d5e1b3bbe2a97e0fe237758b3f3a0e44b4`. Its tree equals tested candidate
`45121e6c1e6c425f05dfd96b4618bd719be5a5d4`. Both source heads are ancestors.
No source history was rewritten. Annotated tag `v1.7.12` was pushed once on
the tested main commit. Its tag object is
`9cd0b56cc32a3adf98ef206b77270bf3decac4ae`. Release workflow 38621735634
passed. [TalkOps 1.7.12](https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.12)
was published at 2026-10-14T19:18:54Z. It is neither a draft nor a prerelease.
The remote tag still resolves to the exact tested main commit.

All three versioned manifests, their six OCI configs, and the moving `1.7`
and `latest` tags are verified. Each config reports version `v1.7.12` and
the release commit as its revision. Moving tags have the same platform digests.

| Image | amd64 digest | arm64 digest |
| --- | --- | --- |
| `talkops-server` | `sha256:15bc52affb2319e09bb7dc6467d24771693609bf10da9d6f955258aff4467974` | `sha256:fc21667b29217dfa48100ceb72eb3de8cf2a8c4b525883019893b52fab90cb74` |
| `talkops-media-worker` | `sha256:cdc9c659abb7654f5fd3dbc2617d0b70614b85502b773dfe80a4ed1ace1558b8` | `sha256:dd47575cafff146eda2689317acb24f2aac097fb4569e0fc4f1a1be05eae314c` |
| `talkops-freeswitch` | `sha256:9417cdf5734c5750b95104b7fd3bb16fda6a3b5ae19b640995918b7ae27a762e` | `sha256:2d1253ded74dbae74bb4608f213dd6514765408e8a9af73a94ba9a5c5a78c4cc` |

The unchanged source and release remote branches were deleted in one atomic
push after ancestry and head checks. GitHub has only `main` and no open PR.
Foreign local worktrees, local branches and primary-checkout work remain.
The final audit changes only this summary. Its tag does not run the Release
workflow. Audit CI/E2E checks are documentation checks, not release gates.
The owned release worktree is left detached at the audit commit. Only its
local release branch is removed.

| Source | Contributor | Head commit | State |
| --- | --- | --- | --- |
| [PR #23](https://github.com/itsh-neumeier/talkops/pull/23), free SIP names | Claude Code | `4e63b382b7457f51fbc529df60e28631f319c651` | Merged |
| [PR #24](https://github.com/itsh-neumeier/talkops/pull/24), diagnostics cleanup | Claude Code | `93b3366bd72d5f0ac6ce2c2abc713611e2d533ea` | Merged |

No open contribution is omitted. No separate Codex feature PR is assigned.
Codex owns integration checks, one error correction, and release metadata.
Neither source head was already in the base. Both PRs target `main` and are
not drafts. There are no source dependencies or new migration IDs.

## Changes

- PR #23: free SIP names, random defaults, active call number resolution,
  device editor, EN/DE text, guides, domain/API tests, and provisioning E2E.
- PR #24: remove the unreliable diagnostics test calls card and its strings.
  Keep SIP address dialing and `/phone?dial=...`.
- `crates/talkops-core/src/extensions.rs`: retry name conflicts only. Return
  database errors unchanged. A closed-pool regression test covers this case.
  Correct the generated-name example and the name-space comment.
- `Cargo.toml`, `Cargo.lock`, `web/package.json`, `web/package-lock.json`:
  set 1.7.12. Only the six workspace package versions and web root versions
  change. External dependency data is unchanged.
- `CHANGELOG.md`: combine both contributions and the integration correction
  in English release notes. Record upgrade steps and known limits.
- This summary: keep the scope, checks, publication state and next action.
- Merge `audit/release-1.7.11` to preserve the final previous release summary.

## Decisions

- Use the user-selected next patch, 1.7.12. The stable API fields stay. The
  change relaxes the name rules and changes the generated default. API clients
  must read `sip_username` from the device response instead of deriving it.
- Existing names and passwords stay. Names accept 3 to 64 ASCII characters
  (`A-Z`, `a-z`, `0-9`, `.`, `_`, `-`). The global unique index ignores case.
  `web-` remains reserved. Browser identities cannot be renamed.
- A device name can equal another extension number, as approved in PR #23.
  The active call overview resolves extension numbers first.
- PR #23 first used head `922d36e6dad339eeb2409080d9f72e53f8f43cb4`.
  E2E runs 38606285237 and 38606243039 failed because the Yealink check
  expected `210-2`. The author changed it to use the API response. Only the
  corrected head `4e63b382b7457f51fbc529df60e28631f319c651` is selected.
- Keep the primary checkout, its unpublished documentation commit, and its
  three untracked Dahua summaries. Do not import them. Keep foreign worktrees.
- Publish only after final integration PR checks and resulting main CI/E2E
  pass. Do not move old tags or use force pushes.

## Checks

- Passed: source #23 final PR CI 38612725665 and E2E 38612725857;
  push CI 38612719316 and E2E 38612719505.
- Passed: source #24 PR CI 38611016389 and E2E 38611016372;
  push CI 38610991572 and E2E 38610991539.
- Passed locally: `cargo fmt --all --check`; lock-file comparison with the
  base confirms no external dependency changes; EN/DE have the same 1,331
  keys; 36 unique migration IDs; no migration changes; `git diff --check`.
- Passed locally: `npm ci`, `npm run lint`, `npm run check` (0 errors,
  0 warnings), `npm test` (6 passed), and `npm run build`.
- Not completed locally: the Rust regression test build cannot find
  `dlltool.exe` in the Windows GNU toolchain. The Linux CI must run the test,
  clippy, and the full DB/LDAP workspace suite.
- Not run locally: Compose, image smoke tests, SIPp and browser E2E. The Linux
  Docker daemon is unavailable. Use the existing Linux CI/E2E workflows.
- FreeSWITCH sources and workflow are unchanged. E2E still builds the image.
- Passed: final candidate PR CI 38616972557 and E2E 38616972568;
  push CI 38616966900 and E2E 38616966909. All 12 final head checks passed.
  CI logs show both numeric-name resolution and database-error tests passed.
  Smoke output shows `talkops 1.7.12` and `talkops-media-worker 1.7.12`.
- Passed: merge commit tree equals candidate tree; both sources are ancestors.
  All source PRs closed as merged. No other open PR remains.
- Passed: exact-main CI 38619610046 and E2E 38619610127 on
  `6276e7d5e1b3bbe2a97e0fe237758b3f3a0e44b4`. Main smoke output reports
  server and media worker version 1.7.12.
- Passed before tag push: fresh main/scope/unused-tag/version/source gates.
- Passed: Release 38621735634, all six builds, three manifests and GitHub release.
- Passed: additional tag CI 38621735931 and FreeSWITCH image 38621735848.
- Passed: three registry verification scripts; all versioned and moving tags,
  both architectures, and all version/revision labels match.
- Passed: release metadata, remote peeled tag, safe remote branch cleanup,
  and the final open-PR inventory.
- Passed: additional tag E2E 38621736102 on the exact released commit.
- STE review: manual review with `docs/documentation-style.md`. The full
  official standard is unavailable; the full language review is pending.
- CodeRabbit CLI is unavailable. Focused integration review used the actual
  source diffs, contracts, tests, migrations and translations.

## Dependencies and open work

- No source dependencies, migrations or dependency upgrades.
- Real Dahua VTO/VTH registration and calls with numeric names need hardware
  checks. Configure the same name in TalkOps and on the device after a rename.
- The percent-encoded FreeSWITCH CDR spool import defect remains separate
  work. Preserve `cdr-spool/rejected/` files for later recovery.
- A deployed UI version check is unavailable in this environment. Validate
  source versions and CI container smoke output before reporting success.

## Next action

None for this release. Future release tasks must read the final audit summary.
The hardware checks and separate CDR spool work remain with development.

## References

- `docs/ai-memory/20261014-claude-free-sip-usernames.md`
- `docs/ai-memory/20261014-claude-remove-test-calls.md`
- `docs/ai-memory/20261014-codex-release-1.7.11.md`
- `docs/release-workflow.md`
