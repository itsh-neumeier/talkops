# Parallel development and releases

Claude Code and Codex use the same guidance in `CLAUDE.md`. Give each concurrent
task its own Git worktree and branch. Keep a dedicated release-manager chat
with its own integration worktree. Two tools must not edit the same checkout
at the same time, even if their intended files differ: the index, HEAD and
working files belong to that checkout.

Commit the shared guidance before creating new worktrees so every checkout
contains it. Verify the remote target branch before choosing a base; do not
assume that it is named `main` or that a local `origin/HEAD` is current.

Use the [documentation rules](documentation-style.md) for new technical text.
Write technical documentation and task summaries in English with ASD-STE100.
At task start, read the [shared task memory rules](ai-memory/README.md).
Read the summaries that apply to the task and check their facts against the
current code and PR state.

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
- Save a task summary in `docs/ai-memory/`. Use `TEMPLATE.md` in that directory.
  Update the summary at useful checkpoints and before completion or transfer.
  Commit it with the task changes and include its path in the PR.
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
Task summary path:
Status: ready / blocked / in progress
```

The PR and its readiness state are the handoff. Chat history is not shared.
When direct publication of a PR is unavailable, supply a pushed branch and
commit plus the same handoff details to the release manager.

## Release-manager task

Use `docs/release-manager-prompt.md` as the initial prompt. The role does not
start feature work; it integrates the agreed contributions, resolves ordinary
merge conflicts, maintains release metadata and verifies publication.

Read the task summaries of all included contributions. Create a separate
summary for each integration or release. Record source PRs and commits,
decisions, checks, the release commit and tag, publication state and open work.
Save useful checkpoints and failed release results. Commit the summary and
include its path in the integration PR.

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
- `.github/workflows/release.yml`: version validation, the four images for
  amd64, image manifests and GitHub release creation. Releases build arm64
  only when the user explicitly requests it for that release
  ([ADR 0050](adr/0050-release-architectures.md)). Then set the repository
  variable `RELEASE_ARM64=true` before the tag push and remove it afterwards.

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
and all four versioned image manifests contain amd64. Verify arm64 only for a
release with an explicit arm64 request. Check the
server and worker's reported versions where execution is available; FreeSWITCH
has its own upstream version. For stable releases, verify the expected moving
image tags from the workflow as well. Report publication or verification
failures explicitly. Never move an already published tag to a different commit.

This is a workflow for active chat requests. Background monitoring needs a
separately requested automation. Neither the files nor the prompt create an
automation, worktree, branch protection rule or scheduled release on their own.

## Self-hosted CI runner

The `rust` and `images` jobs of `.github/workflows/ci.yml` can use the
owner's self-hosted runner. Set the repository variable `CI_RUNNER` to
`talkops` to use it. Remove the variable to go back to GitHub-hosted runners,
for example when the runner host is off.

- The runner must have the labels `self-hosted` and `talkops`, Docker access
  and the host network. The jobs reach the Postgres service container and
  test containers on `127.0.0.1`.
- Pull requests from forks always use GitHub-hosted runners. Keep the setting
  "Require approval for all external contributors".
- E2E, FreeSWITCH and release workflows use GitHub-hosted runners.
