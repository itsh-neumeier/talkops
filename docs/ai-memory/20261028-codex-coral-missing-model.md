# Coral reports no model after the 1.8.13 release

## Task state

- Date: 2026-10-28
- Tool: Codex
- Branch: codex/coral-model-install
- Base: v1.8.13, commit 4b44cd6
- Status: PR preparation for 1.8.14; production installation is user-owned
- PR: https://github.com/itsh-neumeier/talkops/pull/88

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

The user requested a PR for release 1.8.14. Include the install helper and
handbooks. Do not change release metadata in this development PR.
The package is not in Git and is not a public release asset. Its metadata
contains `license_verified: false`. The base model is NVIDIA
`stt_de_quartznet15x5`, version `1.0.0rc1`. The current model card refers to
NGC Terms of Use, not CC-BY-4.0:
https://catalog.ngc.nvidia.com/orgs/nvidia/nemo/models/stt_de_quartznet15x5
Checked on 2026-10-28. The user then asked to include the model itself.
The NGC page loads its terms from the public products API:
https://api.ngc.nvidia.com/v2/products?show-hidden=true
The `organization` product points to:
https://ngc.nvidia.com/legal/consolidated-tou.json
This document refers to the current Technology Access Terms:
https://developer.nvidia.com/legal/terms
The terms are dated 2026-08-20. Section 4 limits the default grant to the
recipient's use. Section 5(e) requires express authorization in a Product
Agreement for distribution, including public repositories. The German
model card refers to NGC terms. The downloaded source checkpoint contains
only `model_config.yaml` and `model_weights.ckpt`, without a separate
license or notice. No separate distribution grant was found.
Redistribution is not verified. Keep this model out of public images and
assets until a specific grant is available. This is a source check, not a
legal opinion. The PR must state that an application release alone still
does not install weights. Common Voice training data does not replace the
base checkpoint's terms. No local-use or production-use permission was
established by this check.

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

Publish the development PR and give its handoff to the release manager.
For the user-owned trial, copy the separate package and helper to the Docker
host. Run the helper there. Then test the connection and select `coral-ctc-de`.
Public model distribution needs the license review and required notices.
