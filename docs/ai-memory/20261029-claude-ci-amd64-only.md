# CI image builds on amd64 only

## Task state

- Task ID: 20261029-claude-ci-amd64-only
- Date: 2026-10-29
- Tool: Claude Code
- Status: Completed
- Branch: claude/ci-amd64-only
- Base commit: 45e2b25 (main, 1.8.17 candidate)
- Related commits or PR: Pending

## Goal

Remove the arm64 Coral image build from CI. The owner asked for this after
releases became amd64 only (ADR 0050).

## Result

- `.github/workflows/ci.yml`: the image matrix has no arm64 entry.
- `docs/adr/0050-release-architectures.md`: the consequence about CI changed,
  with a dated amendment section.

## Decisions

- No new ADR: the change only extends the scope of ADR 0050 from releases to
  CI. The amendment section records the decision.
- Before a release with an arm64 request, add the arm64 CI entry again in a
  pull request. A manual release dispatch is a publication, not a test.

## Checks

- CI on the PR is the check. No local run.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- The owner registered a self-hosted runner (`docker-desktop-talkops`,
  labels `self-hosted,Linux,X64,talkops`). It is not used yet.

## Next action

Merge the PR.
