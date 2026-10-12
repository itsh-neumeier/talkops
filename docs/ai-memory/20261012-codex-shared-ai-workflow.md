# Shared AI workflow and task memory

## Task state

- Task ID: 20261012-codex-shared-ai-workflow
- Date: 2026-10-12, Europe/Berlin
- Tool: Codex
- Status: Completed
- Branch: ccr-df1c8083-0ce1he
- Base commit: 1a77879e8efe4fc54f1efa92149b6638feaea0bd
- Related commit: 5d4afd85f89c651c65db7731f6f9106467af18b5
- PR: None. The user requested a commit and push of the setup files.

## Goal

Let Claude Code and Codex develop TalkOps at the same time.
Give a separate Codex chat control of merges and releases.
Save task results for other models and chats.
Use ASD-STE100 for technical documentation and task summaries.

## Result

The initial workflow and both start prompts were committed and pushed.
Commit `5d4afd8` contains that setup. The new documentation rules, task memory
files and prompt updates are complete. This summary is saved with those changes.
Use Git history and the remote branch to check their publication state.

## Changes

- `AGENTS.md`: Tell Codex to read the shared project rules and task memory.
- `CLAUDE.md`: Define shared work, documentation and task memory rules.
- `README.md`: Link the workflow, prompts, documentation rules and memory.
- `docs/release-workflow.md`: Define contribution and release transfers.
- `docs/development-chat-prompt.md`: Start a development task in either tool.
- `docs/release-manager-prompt.md`: Start the separate release chat.
- `docs/documentation-style.md`: Define the STE target and project terms.
- `docs/ai-memory/README.md`: Define how to read and save task results.
- `docs/ai-memory/TEMPLATE.md`: Give each task the same summary structure.

## Decisions

- Keep shared rules in `CLAUDE.md`. Use `AGENTS.md` as the Codex entry point.
- Give each active task its own worktree and branch. This prevents shared
  working files and index changes between parallel tasks.
- Developers submit PRs. The release chat controls target branch merges,
  release versions, tags and publication.
- Each task owns one summary file. A shared index is not required.
  This reduces conflicts between parallel contributions.
- Read relevant summaries at task start. Check old statements against the
  current code and PRs.
- Use English STE for new technical documentation and all task summaries.
  Keep German translations and user-facing prompts in German.
- Record the language review result. Do not claim full STE compliance from
  clear text alone. The full official standard is not available locally.

## Checks

- Passed: Initial workflow documentation links and `git diff --check`.
- Passed: Initial push. The remote branch contained commit `5d4afd8`.
- Passed: Local Markdown links and whitespace in all 10 changed documents.
- Passed: `git diff --check` for the memory changes.
- Not run: Application tests. This task changes documentation only.
- STE review: Clear English review only. The full standard and dictionary
  were not available. Full language review is pending.

## Dependencies and open work

- No development or release worktrees were created in this chat.
- No development PR, release tag, release or automation was created.
- The observed remote default branch is `ccr-df1c8083-0ce1he`.
  Check the remote again before a new task chooses its base.
- The release workflow starts on a version tag push. It does not wait for
  normal CI or E2E. The release chat must verify those checks before tagging.
- The language rules replace the old German-only rule for new ADRs.
  Existing German architecture documents do not need an unrelated rewrite.

## Next action

Start development tasks with `docs/development-chat-prompt.md`.
Start the release chat with
`docs/release-manager-prompt.md`. Create separate worktrees for active tasks.

## References

- [Release workflow](../release-workflow.md)
- [Development prompt](../development-chat-prompt.md)
- [Release prompt](../release-manager-prompt.md)
- [Documentation rules](../documentation-style.md)
- [Memory rules](README.md)
