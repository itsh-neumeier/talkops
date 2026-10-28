# 0048 - Coral model library and upload by the administrator

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

1. `coral-stt` keeps a model library in `models/coral/library/<id>/`.
   The id is the first 12 hex digits of the SHA-256 of the Edge TPU file.
   Each slot (`de`, `en`, `langid`) uses one library model. The slot
   directory holds a copy, so the startup loader stays unchanged.
2. Admin endpoints under `/v1/admin/`: overview, add and delete a library
   model, select a model for a slot or empty a slot, and restart. The
   optional service key protects them like all `/v1/*` endpoints.
   `TALKOPS_CORAL_MODEL_ADMIN=false` disables them.
3. An upload and a selection write into a staging directory first. The
   service checks the files with the startup loading code. It also checks
   the TFLite identifier and the `edgetpu-custom-op` marker. Only a valid
   copy replaces the target.
4. The library holds at most 10 models and 512 MiB. A model that a slot
   uses cannot be deleted. A model that was copied into a slot by hand moves
   into the library with an empty license before a selection replaces it.
5. The upload must include a name, a license and a source statement. The
   service stores them in `install.json`. TalkOps does not check them. The
   UI requires a confirmation that the license permits the use.
6. The service does not reload models while it runs. A restart request
   stops the process; the container restart policy starts it again.
   Open connections close after at most ten seconds.
7. The TalkOps API forwards these requests to the configured Coral
   connection. It requires the `transcription.settings` permission and
   writes an audit entry with slot, model id, digest, license and source.
8. The `models` volume is writable in `coral-stt`.
9. Only one model per slot is active. Several active models of one language
   would need more Edge TPU memory and time and are not part of this decision.

## Consequences

- Administrators can keep and compare models that they may use but that
  TalkOps may not redistribute. Each administrator is responsible for this use.
- A model change interrupts live previews for a few seconds.
- An uploaded model that fails on the TPU does not stop the service from
  starting. The administrator can select another library model in the UI.
- Without a restart policy, a restart request stops the service.
- `coral-stt` can now write to the `models` volume. A selected model uses
  the space of its library copy and its slot copy.
