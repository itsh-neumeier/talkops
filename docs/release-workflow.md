# Parallel development and releases

Claude Code and Codex use the same guidance in `CLAUDE.md`. Give each concurrent
task its own Git worktree and branch. Keep a dedicated release-manager chat
with its own integration worktree. Two tools must not edit the same checkout
at the same time, even if their intended files differ: the index, HEAD and
working files belong to that checkout.

Commit the shared guidance before creating new worktrees so every checkout
contains it. Verify the remote target branch before choosing a base; do not
assume that it is named `main` or that a local `origin/HEAD` is current.

## Development tasks

- Use `codex/<task>` for Codex branches and `claude/<task>` for Claude Code
  branches, unless the user chooses another naming convention.
- Develop, test, commit and push the task on its own branch. Submit a focused
  PR against the verified target branch. Keep unfinished PRs as drafts.
- Leave release version bumps, release tags, publication and merges into the
  target branch to the release manager. Propose release-note text in the PR
  body so concurrent tasks do not compete over the same release metadata.
- Identify database migration IDs in the PR. Concurrent tasks can otherwise
  create migrations with duplicate IDs even when the filenames differ.
- When handing off a ready PR, use this format:

```text
Contributor: Claude Code / Codex
Task and intended release:
PR URL, source branch and head commit:
Changes and acceptance criteria:
Checks run and results; checks not run:
Dependencies on other PRs:
Migration IDs, upgrade implications and compatibility changes:
Suggested release notes:
Status: ready / blocked / in progress
```

The PR and its readiness state are the handoff. Chat history is not shared.
When direct publication of a PR is unavailable, supply a pushed branch and
commit plus the same handoff details to the release manager.

## Release-manager task

Use `docs/release-manager-prompt.md` as the initial prompt. The role does not
start feature work; it integrates the agreed contributions, resolves ordinary
merge conflicts, maintains release metadata and verifies publication.

For each batch, record the target branch, included PRs and source head commits,
expected contributions still pending, dependencies, integration result and
check results in the integration PR. Choose a version according to the
repository's Semantic Versioning convention; ask about unresolved breaking
behavior or release scope when the available evidence cannot settle it.

Integrate the selected contributions into `codex/release-<version-or-batch>`
based on the latest verified remote target. Merge their commits into this branch
without rewriting the source branches. Include every contribution in the agreed
scope, or explicitly resolve a blocked contribution with the user before
publishing. A release need not contain a contribution from each tool if that
tool has no contribution assigned to the batch.

Check the combined result, including migration ID uniqueness and ordering,
API/UI compatibility, translations and dependencies between changes. Resolve
mechanical conflicts while preserving both behaviors; raise semantic conflicts
that require product decisions. Release metadata includes the workspace version
in `Cargo.toml`, workspace package versions in `Cargo.lock`, `web/package.json`,
the root package versions in `web/package-lock.json`, and `CHANGELOG.md`.
Regenerate lock metadata with the package tools without unrelated upgrades.

Use the existing workflows as the validation contract:

- `.github/workflows/ci.yml`: Rust formatting, clippy and tests; web lint,
  check and build; Compose validation; server and worker build/smoke tests.
- `.github/workflows/e2e.yml`: SIPp and browser softphone tests.
- `.github/workflows/freeswitch.yml`: additional FreeSWITCH checks when its
  sources or workflow change.
- `.github/workflows/release.yml`: version validation, three images for amd64
  and arm64, multi-architecture manifests and GitHub release creation.

Wait for the applicable checks on the final integration PR revision. Merge the
integration PR using the repository's permitted merge method and verify the
resulting target commit and its CI/E2E runs. If the target advances, refresh the
integration and check the changed candidate again. Tag only the validated
commit on the target branch, with an unused `vX.Y.Z` tag matching the code.

The release workflow starts on tag pushes and can also be manually dispatched.
It does not depend on CI/E2E, so a tag push is a publication action, not a test
run. Use one tag push to trigger the existing publication workflow; do not
create a competing release or bypass checks through a manual dispatch.

Verify the workflow succeeded, the GitHub release points to the intended tag,
and all three versioned image manifests contain amd64 and arm64. Check the
server and worker's reported versions where execution is available; FreeSWITCH
has its own upstream version. For stable releases, verify the expected moving
image tags from the workflow as well. Report publication or verification
failures explicitly. Never move an already published tag to a different commit.

This is a workflow for active chat requests. Background monitoring needs a
separately requested automation. Neither the files nor the prompt create an
automation, worktree, branch protection rule or scheduled release on their own.
