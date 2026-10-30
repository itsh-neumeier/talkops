# 0050 - Release image architectures: amd64 by default, arm64 on request

- Status: Accepted
- Date: 2026-10-28
- Supersedes: the image architecture part of
  [0007](0007-deployment-und-netzwerk.md)

## Context

ADR 0007 publishes all images for amd64 and arm64. Each release builds four
images on two native runner types. The arm64 builds add release time and runner
use. The project owner does not need arm64 images by default. The owner decided
on 2026-10-28 that releases build arm64 only on an explicit request.

## Options

1. Keep amd64 and arm64 for every release.
2. Build amd64 only. Remove arm64 from the release workflow.
3. Build amd64 by default. Add arm64 for a release when the owner requests it.

## Decision

Use option 3. The release workflow builds amd64 images. A tag push adds arm64
only when the repository variable `RELEASE_ARM64` is `true`. A manual
dispatch can add arm64 with the `arm64` input. The release manager sets the
variable before the tag push only when the owner explicitly requests arm64 for
that release, and removes it after the release.

The workflow selects the architectures in one step and passes them to the build
matrix. The manifest job publishes the digests that the build jobs produced.

## Consequences

- Releases are faster and use no arm64 runners by default.
- `1.8`, `latest` and the version tags of a default release contain amd64 only.
  An arm64 host that pulls such a tag gets no matching image. Operators on
  arm64 must pin a release whose notes list arm64.
- Release notes state the published architectures.
- CI builds and tests images on amd64 only. An arm64 release request builds
  the arm64 images for the first time in the release workflow. Before such a
  release, add the arm64 CI entry again in a pull request and let CI pass.

## Amendment 2026-10-29

The first version of this ADR kept the arm64 Coral image build in CI. The
owner decided on 2026-10-29 to remove it, because arm64 is not a default
target. The consequence above replaces the earlier statement.
