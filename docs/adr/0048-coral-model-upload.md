# 0048 - Coral model upload by the administrator

- Status: Proposed on 2026-10-28.
- Extends: ADR 0046 (Coral Edge TPU speech recognition).

## Context

The `coral-stt` image contains no model weights. The German candidate is
based on NVIDIA `stt_de_quartznet15x5`. Its model card refers to the NGC
Terms of Use. No redistribution grant was found (see
`docs/ai-memory/20261028-codex-coral-missing-model.md`). The project cannot
publish these weights or weights derived from them.

Until now, an administrator copied model files into the `models` volume by
hand or with a package-specific script. The volume was read-only in
`coral-stt`. The service loads models only at startup.

## Decision

1. `coral-stt` has admin endpoints under `/v1/admin/`: list, install,
   restore and remove a model slot (`de`, `en`, `langid`), and restart.
   The optional service key protects them like all `/v1/*` endpoints.
   `TALKOPS_CORAL_MODEL_ADMIN=false` disables them.
2. An install writes into a staging directory first. The service checks
   the files with the startup loading code. It also checks the TFLite
   identifier and the `edgetpu-custom-op` marker. Only a valid upload
   replaces the slot. The previous slot moves to `.backup/<slot>`.
3. The upload must include a license and a source statement. The service
   stores them in `install.json`. TalkOps does not check them. The UI
   requires a confirmation that the license permits the use.
4. The service does not reload models while it runs. A restart request
   stops the process; the container restart policy starts it again.
   Open connections close after at most ten seconds.
5. The TalkOps API forwards these requests to the configured Coral
   connection. It requires the `transcription.settings` permission and
   writes an audit entry with slot, digest, license and source.
6. The `models` volume is writable in `coral-stt`.

## Consequences

- Administrators can use models that they may use but that TalkOps may not
  redistribute. Each administrator is responsible for this use.
- A model change interrupts live previews for a few seconds.
- An uploaded model that fails on the TPU does not stop the service from
  starting. The administrator can restore the previous model in the UI.
- Without a restart policy, a restart request stops the service.
- `coral-stt` can now write to the `models` volume. The upload size is
  limited to 64 MiB for each model file.
