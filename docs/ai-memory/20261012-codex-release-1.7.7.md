# TalkOps 1.7.7 release

## Task state

- Task ID: 20261012-codex-release-1.7.7
- Date: 2026-10-12; completed 2026-10-13, Europe/Berlin
- Tool: Codex
- Status: Completed
- Integration branch: codex/release-1.7.7 (remote branch removed after verification)
- Base commit: 4503fb3b96167aa77320cc936597fc6147b392c3
- First integration PR: https://github.com/itsh-neumeier/talkops/pull/10
- Completion integration PR: https://github.com/itsh-neumeier/talkops/pull/11
- Audit archive destination: audit/release-1.7.7

## Goal and result

Publish all complete contributions as 1.7.7. Preserve unfinished work.
All ready PRs #3 through #9 are merged with their source histories.
PR #8 passed the requested readiness check. PR #9 became ready before publication.
The user's original all-ready request determines scope. The requested version
remains 1.7.7. The changelog states the pickup and admin API compatibility changes.

Release commit: `21f5a5158978bfdddb86c293cdf4688c73223e5b`.
The target tree equals checked candidate `9773e412d66db1e910df8657d72bd9cc57f5d2b7`.
The new annotated `v1.7.7` tag points to this exact commit. No tag was moved.
The Release workflow and GitHub release succeeded. All images are verified.

- Release: https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.7
- Workflow: https://github.com/itsh-neumeier/talkops/actions/runs/38348018997
- Published: 2026-10-13 04:51:39 UTC; stable, not draft
- Primary branch: main, at the release commit

This summary update follows publication. It does not change the release code.
Archive its commit under the audit destination. The published tag stays fixed.
The integration PR contains the final audit reference and current branch inventory.

## Sources

| PR | Contributor / change | Verified head |
| --- | --- | --- |
| #3 | Codex: persistent softphone and identity selection | `ec0560b69eb78d5f781a748c9ac7149443156a74` |
| #4 | Codex: encrypted backup and guided recovery | `1b02ed7071b214e6e9f6c916cbe1d396474716be` |
| #5 | Codex: explicit internal phonebook dialing | `a35169dc6f3ba31010756dbdbb85b3d316dca9f1` |
| #6 | Claude Code: task memory backfill | `6ce43b6a323c4ce40634455aac23d1994f5edb17` |
| #7 | Codex: active call identities and local live previews | `8b9081149bd651dbf05dfda786ff0ad2e3dbc249` |
| #8 | Claude Code: persistent CDR spool and importer | `d2462b5cd12d51e4ce5ea9c7448984d5493aa6a4` |
| #9 | Claude Code: device codec profiles and iLBC | `13600923aca14af900ecb8e3531e6f1d2be3391f` |

All seven source heads are ancestors of the release commit. No source history
was rewritten. The release also preserves the final 1.7.6 audit commit `6e151b7`.

## Integration changes

- Preserve recovery and active-call browser suites in the CI merge conflict.
- Preserve ADRs 0020 through 0023. Keep all released migrations unchanged.
- Include the PR #8 CDR spool in the PR #4 backup storage, archive validation
  and restore. Server and backup use the same configured spool path.
- Extend the encrypted backup round trip to preserve pending and rejected CDRs.
  Verify CLI storage path selection and replacement of post-backup spool data.
- Update all Cargo workspace and web root versions to 1.7.7. No unrelated
  Rust or npm dependency upgrade. PR #4's reviewed dependencies are retained.
- Write combined English release notes and upgrade instructions.

## Checks

- Passed all exact-head source CI/E2E checks; #8/#9 also passed FreeSWITCH.
- Passed local Rust format, whitespace, ancestry and migration checks.
- Passed 32 unique migration IDs and 1,135 matching unique DE/EN keys.
- New migration IDs: `20261026000002`, `20261026000003`, `20261027000001`.
- Passed local web lint/check, Node tests and build for the first batch.
  Normalize Windows CRLF only in the owned checkout to run lint.
- Passed first batch integration CI/E2E: `38297401397`, `38297401356`.
- Passed first batch target CI/E2E: `38298375924`, `38298375922`.
- Passed final candidate CI/E2E/FreeSWITCH: `38345317841`, `38345317878`,
  `38345317834`; corresponding push checks also passed.
- Passed final target CI/E2E/FreeSWITCH: `38346664274`, `38346664278`,
  `38346664328`. Server/worker smoke tests report 1.7.7.
- Passed tag CI/E2E/FreeSWITCH: `38348018951`, `38348018945`, `38348018909`.
- Passed Release workflow `38348018997`: version gate, six native builds,
  three manifests and GitHub release.
- Passed registry checks for server, media worker and FreeSWITCH: linux/amd64
  and linux/arm64, revision labels equal the release commit, version labels
  equal v1.7.7. Moving tags 1.7 and latest match both versioned platform digests.
- Not run locally: full Linux/DB/LDAP stack, real handset tests, deployed UI
  observation. CI supplies runtime evidence. UI/API version comes from Cargo.
- Review: manual integration review; CodeRabbit CLI unavailable.
- STE review: repository writing rules used. Full official standard unavailable;
  full language review pending. This is separate from functional verification.

## Cleanup and remaining work

Renamed the remote default branch from `ccr-df1c8083-0ce1he` to `main`.
The commit did not change. Deleted ten integrated remote branches atomically
only after fresh head and ancestry checks. No force push was used.

Remaining remote branches:

- `main`: `21f5a5158978bfdddb86c293cdf4688c73223e5b`
- `claude/vibrant-allen-8j9w71`: `6394295b1ac75eb852cb9ed723bbdbd56bafac8c`

The second branch has no PR or ready handoff. It is excluded and preserved.
No open PR remains. Foreign worktree files, local feature branches and
uncommitted work were preserved. The primary local branch was renamed to main
and tracks origin/main; its checkout remains at its previous commit `2fbe319`.
No background automation or message to another development chat was sent.

Deployment instructions remain the operator's work:

- Deploy all three images and updated Compose together for the CDR volume.
- Copy undelivered CDRs out of old FreeSWITCH containers before replacement.
- Resync provisioned phones; internal calls use **number and pickup uses
  *8*extension. Update manual BLF and saved pickup keys.
- Resync AX83H/AX86R for G.722/iLBC. HD/Robust profiles offer audio only.
- Admin WebRTC clients must send extension_id. Complete recovery archives
  require a separate password and captured deployment files.
- Local previews need the base model and additional worker CPU/memory.
- PR #8's ESL/unresolved-row diagnostics and the production CDR root cause
  remain follow-up work. Codec analysis and automatic rules are future work.

## Next action

None for this release. A later development task must submit the remaining
branch through a PR before integration. Follow the release upgrade notes.

## References

- [Release workflow](../release-workflow.md)
- [Release 1.7.6 summary](20261012-codex-release-1.7.6.md)
- [Changelog](../../CHANGELOG.md)

## Verified platform digests

| Image | amd64 | arm64 |
| --- | --- | --- |
| talkops-server | sha256:16da86972d9140228a8330cf6cb27d6120c155e1738dcad8f58c81419ad69e24 | sha256:7764505b219707124e553471bfd299d67297ffdc263240509c48b3e056b98e41 |
| talkops-media-worker | sha256:9eae906d8b3bd85d43e595638b41a7f1f87cb75d65e8f04f6fec4429a18b87e3 | sha256:0da449b35fd0e6f3073556b75fcb473cb78d17af57460cc2350aeec4347e87b5 |
| talkops-freeswitch | sha256:129e29442a175130e951bdcc3dc51d4bde21f1c34cc43635a280da917ebd639f | sha256:ab83799fd1a8173f47636dbced377e7cafd7b2a106a21ac1c627a8a947e91c04 |
