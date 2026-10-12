# Initial joint release inventory

Snapshot: 2026-10-12 (Europe/Berlin). This batch records preparation only.
No merge into the target, version bump, tag push or publication is authorized
by the inventory request. The integration PR is a draft tracking record.

## Repository and baseline

- Repository: `itsh-neumeier/talkops`; fetch and push remote:
  `https://github.com/itsh-neumeier/talkops.git`.
- Verified target: `ccr-df1c8083-0ce1he`, confirmed by GitHub's default branch
  and `git ls-remote --symref origin HEAD`.
- Current target commit: `5d4afd85f89c651c65db7731f6f9106467af18b5`.
  It advanced during the inventory with committed collaboration/release guidance
  (six documentation files only). This commit was merged into the integration
  branch without rewriting history. Its [CI](https://github.com/itsh-neumeier/talkops/actions/runs/38248224452)
  and [E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38248224423)
  are pending at this snapshot; it is not yet a validated release candidate.
- Latest published stable release: [TalkOps 1.7.5](https://github.com/itsh-neumeier/talkops/releases/tag/v1.7.5).
  The tag resolves to the original target baseline,
  `1a77879e8efe4fc54f1efa92149b6638feaea0bd`. Sixteen published tags and
  releases were listed; no draft release or prerelease was listed.
- Before opening this tracking PR, GitHub listed no open or historical PRs,
  no open issues, and only the target branch. There are no remotely handed-off
  feature commits beyond the published baseline.
- GitHub reports the target branch unprotected; branch protection lookup
  returns `404 Branch not protected`, and repository rulesets and effective
  branch rules are empty. Merge commits, squash and rebase are enabled;
  auto-merge is disabled. For feature integration, use merge commits to
  preserve source history, and recheck rules before any target merge.
- Own worktree: `C:/Users/timo/.codex/worktrees/release-initial-batch/talkops`.
  Integration branch: `codex/release-initial-batch`, created from the exact
  verified target commit. The worktree was initially clean.

## Contribution ledger

| Contributor / contribution | Source PR and head commit | Status | Dependencies / migrations | Release inclusion |
| --- | --- | --- | --- | --- |
| Claude Code: next contribution | No PR or pushed feature branch available | Handoff missing; task identity and readiness unknown | Not supplied | Await agreed scope and handoff |
| Codex: next contribution | No PR or pushed feature branch available | Handoff missing; task identity and readiness unknown | Not supplied | Await agreed scope and handoff |
| Shared collaboration and release guidance | No source PR; target commit `5d4afd85f89c651c65db7731f6f9106467af18b5`, authored by Timo Neumeier | Already committed and pushed to target during inventory | Documentation only; no migrations | Already in target and integration ancestry; CI/E2E pending |
| Release manager: this inventory | Tracking integration PR; documentation commit only | Draft / preparation | No new migrations or runtime changes | Tracking only; no release candidate yet |

Included source PRs: **none**. No ready source PR has been excluded. No named
feature has been promised in repository issues or PRs visible at this snapshot;
the absence of a handoff does not imply that either developer has no local work.
Already included: the entire `v1.7.5` baseline. Its recent commits are attributed
to Claude and include resync/restart separation, phone-page extension assignment,
door-station settings navigation and ringing several extensions per number.
There is no PR handoff establishing a new Claude or Codex contribution to this
batch. Developer chats were not accessed or messaged.

At the first inspection, the original checkout at
`A:/aislop/codex/talkops/talkops` had these changes:

- Modified: `CLAUDE.md`, `README.md`.
- Untracked: `AGENTS.md`, `docs/release-workflow.md`,
  `docs/release-manager-prompt.md`.

These local changes were read as instructed but were not staged, committed or
copied by the release manager. At the final inspection, the owning checkout
was clean and the shared rules had been committed and pushed as `5d4afd8`,
alongside `docs/development-chat-prompt.md`. They were then consumed exclusively
from the fetched remote target commit. The shared-guidance handoff blocker is
resolved; the absence of a source PR is recorded rather than inventing one.

## Validation evidence for the published baseline

| Check | Result / evidence |
| --- | --- |
| Rust fmt, clippy and workspace tests | Successful [target-branch CI](https://github.com/itsh-neumeier/talkops/actions/runs/38080557342), including PostgreSQL/LDAP and backup-test prerequisites |
| Web lint, check and build | Successful job in the same target-branch CI run |
| Compose validation | Successful job in the same target-branch CI run |
| Server and media-worker image build and version smoke tests | Successful jobs in the same target-branch CI run |
| SIPp and browser softphone E2E | Successful [target-branch E2E](https://github.com/itsh-neumeier/talkops/actions/runs/38080557304); workflow runs `WEBRTC_E2E=1` |
| FreeSWITCH build, health, configured modules and UID | Successful [FreeSWITCH image run](https://github.com/itsh-neumeier/talkops/actions/runs/38080628421) at the same commit, triggered by the existing tag |
| Existing release workflow | Successful [Release run](https://github.com/itsh-neumeier/talkops/actions/runs/38080628382): version gate, all six native architecture builds, three manifest jobs and GitHub release |
| Release image manifests | Registry reads confirm `linux/amd64` and `linux/arm64` for all three `1.7.5` images; each image's `1.7` and `latest` tags point to the same architecture manifests |
| Version metadata | `Cargo.toml`, all six workspace package entries in `Cargo.lock`, `web/package.json`, and both root versions in `web/package-lock.json` are `1.7.5`; `[Unreleased]` is empty |
| Migration IDs | 28 SQL files match the 14-digit naming convention; no duplicate IDs; sorted IDs range from `20261004000001` to `20261025000002` |
| DE/EN catalogs | 1,018 keys in each catalog; no duplicate keys or key-set differences |

Versioned OCI index digests observed:

| Image | `1.7.5` index digest |
| --- | --- |
| `talkops-server` | `sha256:a863b7051b60eb172a9b9e25cd9edc55355500c94db5a967765bedbb4af7384f` |
| `talkops-media-worker` | `sha256:dca9dbe5980ea97f18a418f5ebcceaf2e77cacbf7ed123f26eea141e621b884c` |
| `talkops-freeswitch` | `sha256:036748b79ef90633f95d6303a66460ec257ae5caacee3d0966497426afe5aa0b` |

No local runtime suite was rerun for this inventory. The Docker CLI is present,
but the Docker Desktop Linux daemon is unavailable; SIPp and `psql` are absent
from PATH. Registry manifest reads work without that daemon. No deployed web UI
was opened, so its displayed version was not independently observed. Published
migrations have future-looking IDs, including `20261025000002`; new contributions
must respect the released sequence rather than choosing an earlier ID solely
from today's date. Uniqueness and catalog key parity do not establish semantic
migration correctness, translation quality or complete API/UI compatibility.
Those require inspecting and testing the actual combined contributions.

## Missing before the first joint release

1. Establish the intended tasks and release scope for Claude Code and Codex,
   including any promised contribution that is not yet represented by a PR.
2. Receive each ready PR against `ccr-df1c8083-0ce1he`, with contributor, task,
   intended release, full head SHA, acceptance criteria, executed and omitted
   checks, dependencies, migration IDs, upgrade/compatibility implications,
   suggested English release notes and explicit readiness. Keep unfinished
   contributions as drafts. Missing migrations/dependencies should be stated
   as `none`, not silently omitted.
3. Shared guidance: resolved by target commit `5d4afd8`. For future work, consume
   only committed contributions and record ready feature PR handoffs.
4. Integrate the agreed source heads using merge commits from a freshly fetched
   target. Record ancestry, dependencies and conflict decisions in the tracking
   PR, preserving both contributors' behavior.
5. Inspect combined migration ordering and released checksums, API/UI contracts,
   translations and upgrade behavior. Run all relevant CI/E2E on the final
   integration revision and on the resulting target commit after an authorized
   merge. Pending or failed relevant checks block publication.
6. Choose the next SemVer from the actual scope: patch for compatible fixes,
   minor for compatible features, major for breaking public contracts. The next
   version is **TBD**, not yet assigned; `1.7.6` and `1.8.0` are conditional
   examples, not release decisions. Update all version locations and the
   changelog consistently without unrelated dependency upgrades.
7. Obtain a release request. Only then push a previously unused `vX.Y.Z` tag on
   the verified target commit, use the existing workflow, and verify all images,
   architecture manifests, stable moving tags, release and displayed versions
   where executable. The release workflow does not wait for ordinary CI/E2E.

## Resume protocol

Keep the integration PR draft until the agreed scope is complete and ready.
Update its body with source PR URLs and full head SHAs, target SHA, chosen
version, contribution readiness, dependencies, migrations, omitted/blocked
contributions with reasons, candidate SHA and exact check-run links. Re-fetch
the target and re-enumerate open PRs before each integration request. Refresh
and revalidate when the target or any source head changes. No scheduled monitor
was created.
