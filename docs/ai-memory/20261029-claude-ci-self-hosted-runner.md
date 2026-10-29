# Optional self-hosted runner for heavy CI jobs

## Task state

- Task ID: 20261029-claude-ci-self-hosted-runner
- Date: 2026-10-29
- Tool: Claude Code
- Status: Completed
- Branch: claude/ci-self-hosted-runner (based on claude/ci-amd64-only)
- Base commit: 45e2b25 (main, 1.8.17 candidate)
- Related commits or PR: Pending

## Goal

The owner registered a self-hosted runner and asked to use it for faster CI.

## Result

- `ci.yml`: the `rust` and `images` jobs select
  `[self-hosted, talkops]` when the repository variable `CI_RUNNER` is
  `talkops` and the run is a push or a pull request from this repository.
  Otherwise they use `ubuntu-24.04`.
- The LDAP test step deletes its old directory, because a reused runner keeps
  it.
- `docs/release-workflow.md` describes the switch.

## Decisions

- Only the heavy jobs move. E2E, FreeSWITCH and release stay on GitHub-hosted
  runners until the self-hosted runner proves stable.
- A repository variable switches the runner, so a powered-off host does not
  block CI.
- Fork pull requests never use the self-hosted runner (public repository).

## Runner facts (owner's PC)

- `myoung34/github-runner:ubuntu-noble` in Docker Desktop 4.96.3 (WSL2),
  i7-12700 (20 threads), 31 GB for the VM, ephemeral, Docker socket mounted,
  compose file `C:\talkops-runner\docker-compose.yml`.
- Host networking in Docker Desktop works: a host-network container reaches
  published ports on 127.0.0.1 (tested).
- The runner container needs `network_mode: host`. The owner must add it; the
  assistant was not permitted to change that file.

## Checks

- CI on the PR (GitHub-hosted, variable not set) is the check.
- Not tested: a run on the self-hosted runner. It needs the variable and the
  runner change.
- STE review: repository writing rules applied; full standard unavailable.

## Next action

Merge PR100 first, then this PR. Then add `network_mode: host` to the runner,
set `CI_RUNNER=talkops` and check one CI run.
