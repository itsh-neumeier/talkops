# README and bilingual Wiki review proposal

## Task state

- Task ID: 20261020-codex-readme-wiki-proposal
- Date: 2026-10-20, Europe/Berlin
- Tool: Codex
- Status: In progress; proposal prepared for user feedback.
- Branch: codex/readme-wiki-proposal
- Base commit: b1fa28350dc1830e222a27ee770641edb1935ccc
- Related PR: https://github.com/itsh-neumeier/talkops/pull/58 (draft)
- Initial proposal commit: b8a8783

## Goal

Propose an updated README and a complete German and English GitHub Wiki handbook.
The user requires review and feedback before the actual documentation changes.
Commit, push and PR creation are authorized. The release chat owns publication.

## Result

Prepared a proposal with README samples, a full source inventory, missing
chapters, Wiki generation, publication steps and acceptance checks.
Committed and pushed the proposal. Draft PR #58 is available for scope review.
README, current guides, application code and live Wiki are unchanged.
The proposed implementation is not complete and the PR must remain a draft.

## Changes

- `docs/proposals/readme-wiki-de-en.md`: reviewable scope and implementation plan.
- This file: continuation context and review boundary.

## Decisions

- Verified remote default `main`; fetched it before creating a separate worktree.
  Worktree: `C:/Users/timo/.codex/worktrees/readme-wiki-proposal/talkops`.
- The primary checkout is below the task working directory, at `talkops/`.
  Preserve its three untracked Dahua summaries and all other worktrees.
- Propose EN README plus DE README, 34 topic pairs, two indexes and a generated
  Wiki. The current sources contain 25 pairs; nine new topic pairs are proposed.
- Keep handbook sources in the application repository. The release chat
  publishes reviewed generated output to the separate Wiki repository.
- Do not implement the proposal until the user provides feedback or approval.

## Checks

- Passed: remote target, base version, clean new worktree and existing PR scan.
- Passed: source inventory against current guide paths, UI routes, Compose,
  environment examples, changelog and backup implementation.
- Passed: PowerShell local Markdown link check, 62 targets in both new files.
- Passed: inventory coverage check, all 25 EN and 25 DE sources in the proposal.
- Passed: `git diff --no-index --check` for both new files; no whitespace errors.
- Passed: diff inspection confirms no edits to README, user guides, runtime,
  workflows, versions or changelog.
- Failed: `git ls-remote https://github.com/itsh-neumeier/talkops.wiki.git HEAD`
  returned `Repository not found`. GitHub reports Wiki enabled. Access or first
  page initialization must be checked before publication; no content was read.
- Not run: application, database, Docker and SIP tests. Only proposal files change.
- STE review: repository clarity rules applied. Full official standard unavailable;
  formal language review remains pending.

## Dependencies and open work

- User feedback on language layout, handbook scope and Wiki generation is pending.
- No known PR dependency. Recheck the target and open PRs after approval.
- Migration IDs: None. No dependency, version or runtime changes.
- Wiki access, potential initialization and publication require a later check.
- Draft PR #58 contains the proposal and handoff; implementation is pending.

## Next action

Ask the user to review the proposal in draft PR #58.
After feedback, implement the agreed handbook and documentation checks on this
branch. Keep the PR draft until implementation and relevant checks are complete.

## References

- [Proposal](../proposals/readme-wiki-de-en.md)
- [Documentation rules](../documentation-style.md)
- [Release workflow](../release-workflow.md)
- [Shared workflow summary](20261012-codex-shared-ai-workflow.md)
- [Provisioning summary](20261017-codex-provisioning-all-vendors-proposal.md)
- [Fax summary](20261018-codex-fax-sip-proposal.md)
