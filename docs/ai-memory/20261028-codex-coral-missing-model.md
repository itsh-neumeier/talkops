# Coral reports no model after the 1.8.13 release

## Task state

- Date: 2026-10-28
- Tool: Codex
- Branch: codex/coral-model-install
- Base: v1.8.13, commit 4b44cd6
- Status: Install helper prepared; production installation is user-owned

## Findings

Verified the tagged service and Compose sources. The image contains no
weights. The service expects `/var/lib/talkops/models/coral/de/model.json`
and `model_edgetpu.tflite` in the mounted shared `models` volume.
Releasing the application does not install those files.

The local German package is present at
`build/coral-work/coral-production-trial/coral-de-candidate-20261025.tar.gz`.
Its archive SHA-256 is
`d8f2924d6c826bd0d449b981084315195c18a2cd74f59524e97a186ff1133423`.
It contains only the compiled model, metadata, checksums and instructions.
No audio or transcripts are included. The earlier quality gate is still pending.

## Changes and checks

- Added `scripts/coral/install-de-candidate.sh` for this specific package.
  It checks the archive digest, detects the named model volume, backs up
  existing German files and installs only the two expected files.
  It restarts only Coral. Other services and language models stay intact.
- Custom mounts/model directories require explicit manual installation.
- Added the helper procedure to both recording handbooks.
- Set LF endings for Coral shell helpers in `.gitattributes`.
- Passed `/bin/sh -n` in the cached Coral trial image.
- Passed an isolated Docker check with a temporary named volume: mount and
  environment discovery, first copy, backup on a second copy, model digest,
  non-root file access, restart and mock-backend health check.
  The test used `coral-test-ab7d13724f3e`, not a physical TPU or production.
  Temporary test containers and volumes were removed.
- No production host access or model upload was performed.
- STE review: full standard unavailable; full review pending.

## Next action

Give the user the package and helper, to copy to the Docker host. Run the
helper there. Then test the Coral connection and select `coral-ctc-de`.
