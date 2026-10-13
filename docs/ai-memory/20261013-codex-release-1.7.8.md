# TalkOps 1.7.8 release

## Task state

- Task ID: 20261013-codex-release-1.7.8
- Date: 2026-10-13, Europe/Berlin
- Tool: Codex
- Status: In progress. Target verified; publication workflow running.
- Branch: codex/release-1.7.8
- Base commit: 21f5a5158978bfdddb86c293cdf4688c73223e5b (remote main, 1.7.7)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/15

## Goal and scope

Complete and publish all open PRs as the explicitly requested version 1.7.8.
The inventory contains three ready Claude Code PRs. No Codex feature PR is open.
Use the owned release worktree. Do not copy the primary checkout's unpublished
commit or changes from other worktrees. No new feature work or chat message.

| PR | Change | Exact source head |
| --- | --- | --- |
| #12 | Transcript line times and speaker order | `6dd4bea978d3255e8e81a9bb452f120dbeb9756a` |
| #13 | Theme, responsive layout and shared UI | `5a7ff06f6974cf7a9c153d6fd9352b655d53eb97` |
| #14 | Engines per purpose, cloud, realtime and usage | `ce728bc6497d24d1ea45383112e43e8616088cca` |

All three are ready with successful exact-head CI/E2E checks. None was already
in the remote target. No planned contribution is excluded or missing a handoff.
PR #14 proposes a minor release; the user's explicit 1.7.8 version determines
this batch. The API replacement is stated in the release upgrade notes.

## Integration changes

- Merge all source histories. Preserve the completed 1.7.7 audit from its tag.
- Resolve worker conflicts by keeping #12 word timestamps and #14 accepted
  audio accounting. Keep shared timestamp fallback state across channels.
- Remove the replaced TranscriptionApiSettings component. Keep both layout and
  transcription-settings browser suites in CI, plus existing recovery tests.
- Apply #13 container grids and scroll-x to #14's new component. Extend the
  layout fixture to the new settings page with long connection names and usage.
- Add a fake HTTP server regression for word timestamps, rejected word options,
  speaker ordering and accepted audio duration. No live account or charge.
- Keep both DE/EN word-timestamp guidance and the new cloud documentation.
- Set all Cargo/web root versions to 1.7.8 with package tools. No external
  dependency upgrade; #14 enables dependencies already present in the lock.

## Checks

- Passed: source CI/E2E, current guidance and handoff review.
- Passed: cargo fmt, whitespace, 33 unique migrations and DE/EN parity
  (1,173 unique keys). Released migrations remain unchanged.
- New migration: 20261028000001_transcription_profiles, after 20261027000001.
- OpenAI Docs review: current official realtime session/event schema and list
  prices match the implementation. No real API key or live request was used.
- Passed: local npm ci/lint/check (0 errors/warnings), both Node tests and build.
  Initial lint failed on CRLF; LF normalization changed no unrelated content.
- Passed final candidate PR CI/E2E: 38392035664, 38392035782. Push CI/E2E:
  38392027595, 38392027592. All relevant rollup checks passed on eceed00.
- Integration PR #15 merged as `8fe0192a8067d0dcecb005075b876ea0f24bd7b3`.
  Its tree exactly matches candidate `eceed00f5726fcf1c7cb336291e208b5925bfed7`.
  All three source heads are ancestors. No open PR remains at this checkpoint.
- Passed: resulting target CI/E2E (38394131797, 38394131652).
  Both binaries report 1.7.8. The target remained unchanged before tagging.
- Tag v1.7.8 was created and pushed once on the checked commit
  `8fe0192a8067d0dcecb005075b876ea0f24bd7b3`. No open PR remained.
- Pending: publication workflow 38396318948 and registry verification.
- Local Linux Docker/DB/LDAP services unavailable; full runtime checks use CI.
- Manual integration review. CodeRabbit CLI unavailable.
- STE review: full official standard unavailable; full language review pending.

## Dependencies and open work

Update server and worker together. Migration copies old engine settings.
External clients must replace the old transcription-api endpoints. Cloud audio
goes to the configured provider; realtime does not use HTTPS_PROXY. Costs are
estimates. Real-provider access and complex phone detail layout tests are not
directly verified. Existing transcripts do not change.

## Next action

Verify workflow 38396318948, the GitHub release and all three amd64/arm64
manifests, labels and stable moving tags.
Save final publication and cleanup results in the PR and task summary.

## References

- [Release workflow](../release-workflow.md)
- [Previous release summary](20261012-codex-release-1.7.7.md)
- [OpenAI Realtime transcription](https://developers.openai.com/api/docs/guides/realtime-transcription)
- [OpenAI pricing](https://developers.openai.com/api/docs/pricing)
