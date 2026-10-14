# TalkOps 1.7.12 release

## Task state

- Task ID: 20261014-codex-release-1.7.12
- Date: 2026-10-14, Europe/Berlin
- Tool: Codex
- Status: In progress
- Branch: `codex/release-1.7.12`
- Base commit: `7b6e51f612475fc4473880f8a2f818c49147e9e2` (`origin/main`, 1.7.11)
- Integration PR: [#25](https://github.com/itsh-neumeier/talkops/pull/25)

## Goal

Include all open TalkOps PRs in 1.7.12. Check the combined code, merge it,
publish the release, and verify its images. Keep only `main` on GitHub after
publication if the source branches contain no new work.

## Result

Both source PRs and integration PR #25 are merged. Main is
`6276e7d5e1b3bbe2a97e0fe237758b3f3a0e44b4`. Its tree equals tested candidate
`45121e6c1e6c425f05dfd96b4618bd719be5a5d4`. Both source heads are ancestors.
No source history was rewritten. No release has been published yet.

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
- Main CI 38619610046 and E2E 38619610127: pending on the exact main commit.
  Publication remains blocked until both pass.
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

Wait for exact-main CI 38619610046 and E2E 38619610127. Then recheck scope,
main, versions and unused tag before publication. Keep the PR ledger current.

## References

- `docs/ai-memory/20261014-claude-free-sip-usernames.md`
- `docs/ai-memory/20261014-claude-remove-test-calls.md`
- `docs/ai-memory/20261014-codex-release-1.7.11.md`
- `docs/release-workflow.md`
