# README and bilingual Wiki handbook

## Task state

- Task ID: 20261020-codex-readme-wiki-proposal
- Date: 2026-10-20, Europe/Berlin
- Tool: Codex
- Status: Implementation complete; final PR checks and release handoff in progress.
- Branch: codex/readme-wiki-proposal
- Base commit: b1fa28350dc1830e222a27ee770641edb1935ccc
- Related PR: https://github.com/itsh-neumeier/talkops/pull/58
- Proposal commits: b8a8783 and ac6f51b. Git history records implementation commits.

## Goal

Update the README and prepare a complete German and English GitHub Wiki handbook.
The user required review first and approved the proposal with "Ja mach das so".
Commit, push and PR updates are authorized. The release chat owns merges,
release versions, tags and publication, including the live Wiki.

## Result

Implemented English and German README entry points, 34 topic pairs, two language
indexes and a reproducible Wiki build. The output has 73 Markdown pages, including
Home, sidebar and footer. Existing guide paths remain valid.
The live Wiki is not published. Use the release-chat procedure in docs/wiki/README.md.

## Changes

- README.md and README.de.md: current capabilities, matching 1.8.5 configuration
  and image examples, installation, reading paths, development and support links.
- docs/en and docs/de: indexes, navigation and role/prerequisite/result checks
  for existing guides; nine new topic pairs for overview, trunks, extensions,
  phone book, prompts, settings/storage, updates, troubleshooting and reference.
- Existing guides: remove stale phase/version examples, include fax in backup
  contents, pin Portainer/Compose examples and document German permissions and
  recovery target JSON without requiring English for those procedures.
- docs/wiki/pages.json: explicit pair, title, slug, section and UI-route mapping.
- scripts/docs/build-wiki.mjs: validate sources, rewrite links, copy local images,
  generate navigation and pin attribution to the source commit. No push behavior.
- tests/docs/wiki.mjs: 13 tests for conversion, negative cases, navigation,
  coverage, determinism and preservation of existing files.
- .github/workflows/ci.yml: a Node 22 documentation job, without npm dependencies.
- .gitignore: exclude generated build/wiki output.
- docs/wiki/README.md: maintenance and explicit release-chat publication steps.
- docs/proposals/readme-wiki-de-en.md: retain the agreed proposal and record approval.

## Decisions

- Reuse the exclusive worktree at C:/Users/timo/.codex/worktrees/readme-wiki-proposal/talkops.
  Verified remote main still points to the base before implementation.
- Preserve the original checkout's three untracked Dahua summaries and all
  unrelated worktrees. Do not modify application behavior or release metadata.
- Keep docs/en and docs/de as the only edited handbook sources. Generate the
  Wiki copy and preserve independent existing Wiki content on publication.
- Build previews identify uncommitted working-copy content. --release requires
  a clean source checkout. Output refuses symlinks and unmanaged existing files.
- Local images use source-path hashes to prevent filename collisions. Source-only
  links point to the exact source commit; cross-language links stay in the Wiki.
- Template coverage is separate from real hardware/provider verification.

## Checks

- Passed: verified remote main and published v1.8.5 metadata with gh release view.
- Passed: node tests/docs/wiki.mjs, 13 tests, no failures/skips on local Node 24.
- Passed: node scripts/docs/build-wiki.mjs --check, 73 pages and 646 local links.
  Both builds in the reproducibility check produce identical bytes.
- Passed: working-copy Wiki generation and generated navigation/fragment checks.
- Passed: manifest covers all 34 pairs and every current UI +page.svelte route.
- Passed: README and maintainer links/fragments, case-insensitive slug uniqueness,
  missing translation/target/fragment, external URLs with parentheses, code
  examples, source-only references, local images and input/output symlink cases.
- Passed: git diff --check. Application crates, migrations, versions, Compose
  defaults and package dependency files remain unchanged.
- Not run locally: Rust/database, web application, Docker image, SIP/hardware,
  provider fax and production recovery tests. No runtime behavior changed.
  The existing full CI/E2E workflows also run on the PR; consult exact-head results.
- Not run: CodeRabbit review. The CLI is unavailable. Manual generator and
  documentation review used current sources and the regression fixtures.
- Historical capability check: Wiki Git remote returned Repository not found,
  while GitHub reported Wiki enabled. No Wiki content was read or written.
- STE review: repository clarity rules applied. Full official standard unavailable;
  formal language review remains pending.

## Dependencies and open work

- No PR dependency, new packages, migration IDs, runtime or upgrade changes.
- Full PR CI status is recorded in the PR, rather than inferred from this summary.
- Release chat must check Wiki access/initialization and existing content, build
  from the agreed clean source commit, publish and verify rendered pages.
- Local documentation checks do not certify real hardware, provider fax or
  recovery behavior. Their existing deployment acceptance checks still apply.

## Next action

Finish PR checks and mark PR #58 ready after relevant documentation checks pass.
The release chat integrates the PR, builds the clean source with --release and
publishes the Wiki through the recorded procedure. Do not force-push Wiki history.

## References

- [Approved proposal](../proposals/readme-wiki-de-en.md)
- [Wiki maintenance and publication](../wiki/README.md)
- [English handbook](../en/index.md)
- [German handbook](../de/index.md)
- [Documentation rules](../documentation-style.md)
- [Release workflow](../release-workflow.md)
