# TalkOps 1.7.8 release

## Task state

- Task ID: 20261013-codex-release-1.7.8
- Date: 2026-10-13, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.7.8 (remote removed after verification)
- Base commit: 21f5a5158978bfdddb86c293cdf4688c73223e5b (1.7.7)
- Integration PR: https://github.com/itsh-neumeier/talkops/pull/15
- Audit archive destination: audit/release-1.7.8

## Goal and result

Complete and publish all open PRs as the explicitly requested 1.7.8.
All three open feature PRs were ready. All are merged with their histories.
No Codex feature PR was open. Codex supplied integration corrections and
release metadata. No assigned contribution was omitted.

| PR | Contributor / change | Exact source head |
| --- | --- | --- |
| #12 | Claude Code: transcript timing and speaker order | `6dd4bea978d3255e8e81a9bb452f120dbeb9756a` |
| #13 | Claude Code: theme and responsive shared UI | `5a7ff06f6974cf7a9c153d6fd9352b655d53eb97` |
| #14 | Claude Code: engines per purpose, cloud/realtime and usage | `ce728bc6497d24d1ea45383112e43e8616088cca` |

All source heads are ancestors of release commit
`8fe0192a8067d0dcecb005075b876ea0f24bd7b3`. Its tree exactly matches candidate
`eceed00f5726fcf1c7cb336291e208b5925bfed7`. Tag `v1.7.8` was pushed once on
that checked target. No published tag moved. GitHub release and all images
are verified. Published 2026-10-13 12:24:28 UTC, stable and not draft.

- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.8
- Workflow: https://github.com/itsh-neumeier/talkops/actions/runs/38396318948

This summary follows publication. It changes no release code. Archive this
summary commit under the audit destination and record the reference in PR #15.

## Integration changes and decisions

- Preserve word timestamps, VAD mapping and line order from #12 with #14's
  accepted cloud audio accounting. Test both words and rejection fallback
  through a fake HTTP server, including two speakers and accepted duration.
- Keep all recovery, active-call, layout and transcription settings browser
  suites. Apply container grids and scroll-x to the new component. Extend
  layout fixtures to the cloud settings page, long names and usage.
- Keep both DE/EN timing guidance and cloud documentation. Remove the replaced
  component. Preserve the completed 1.7.7 audit in the integration history.
- Use the explicitly requested patch number. #14's developer proposed a minor
  version; the user's requested scope/version controls this release.
  The removed API endpoints and fields are stated in the upgrade notes.
- Update all six workspace and web root versions through the package tools.
  Cargo external lock equals reviewed #14; npm dependency lock unchanged.
  No unrelated dependency upgrade and no source branch rewrite.

## Verification

- Passed source exact-head CI/E2E for all three PRs. Handoffs complete.
- Passed local cargo fmt, whitespace, ancestry, 33 unique migration IDs and
  1,173 matching unique DE/EN keys. No released migration changed.
- New ID: 20261028000001_transcription_profiles, after 20261027000001.
- Passed local npm ci/lint/check (zero errors/warnings), two Node tests/build.
  Initial CRLF lint failure resolved in the owned checkout without unrelated
  content changes. A transient index lock cleared on retry; no lock removed.
- Passed candidate PR CI/E2E: 38392035664, 38392035782. Corresponding push
  CI/E2E: 38392027595, 38392027592. All final-head rollup checks passed.
- Passed resulting target CI/E2E: 38394131797, 38394131652 before tag push.
  Both binaries report 1.7.8. The cloud timing/usage regression passed.
- Passed Release workflow 38396318948: version gate, six native builds,
  three multi-architecture manifests and GitHub release.
- Passed tag CI/FreeSWITCH checks: 38396318912, 38396318835.
- Passed tag E2E 38396318871. All tag checks completed successfully.
- Passed registry: server, media worker and FreeSWITCH each contain linux/amd64
  and linux/arm64. All six configs identify v1.7.8 and the release commit.
  Moving tags 1.7 and latest match both versioned platform digests.
- Current official OpenAI realtime schema and list prices match the source.
  Real OpenAI access was not tested; no key was available. Mock HTTP and
  Realtime WebSocket tests passed. Real handset and deployed UI observation
  were unavailable. The Linux/DB/LDAP/stack evidence comes from CI.
- Review: manual integration review; CodeRabbit CLI unavailable.
- STE review: official standard unavailable; full language review pending.
  This is separate from functional verification.

## Cleanup and remaining work

Deleted four integrated remote branches atomically after fresh head/ancestry
checks. No force push. Only main remains remotely, at the release commit.
No open PR remains. Foreign worktree files and local feature branches stay.
The primary local main has unpublished docs commit
`397fc83ec5a0c1032e3c8b944eab337b3ef495f8` for the earlier Compose delivery.
It has no PR handoff, remains outside this batch and was not overwritten.
No automation or message to a development chat was created.

Operators must update server and worker together. The new migration copies
old settings into providers/profiles. Local live preview remains the default.
External clients must replace transcription-api endpoints and old settings
fields. Cloud profiles send audio to the chosen provider. Realtime does not
use HTTPS_PROXY. Costs are estimates. Existing transcripts remain unchanged.
Restarting FreeSWITCH ends active calls. Follow the release upgrade notes.

## Next action

None for this release. Keep v1.7.8 fixed. Operator deployment is separate.

## References

- [Release workflow](../release-workflow.md)
- [Previous release summary](20261012-codex-release-1.7.7.md)
- [Changelog](../../CHANGELOG.md)
- [OpenAI Realtime transcription](https://developers.openai.com/api/docs/guides/realtime-transcription)
- [OpenAI pricing](https://developers.openai.com/api/docs/pricing)

## Verified platform digests

| Image | amd64 | arm64 |
| --- | --- | --- |
| talkops-server | sha256:1f54f878eb051102ba9989a7085f5bc851ec530725f66685e38c8dc35aff41bb | sha256:f7d079940eb0f6d2d024a477b1ed6e53a537a91f73343953dbffa64d56b4e7d0 |
| talkops-media-worker | sha256:a27589bf4d71de6cbb944be6a9f0de20f3ffb96b207452a0550df7a7d5b81c24 | sha256:195bde026ab06d3facaf05ada262c07ce1fd0ed28bd3d447178741d9cdf5d0e6 |
| talkops-freeswitch | sha256:be1298b3051d72c72f5c3544c7e5c34c5d7ae65c7db4da97e9e9fdefa239f453 | sha256:8b113b761b22e389a9d55a401a17a19ee54a8f775069ecd01776bd0a76ddec31 |
