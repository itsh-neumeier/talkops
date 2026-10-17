# 0039 - Source-derived manufacturer provisioning and device copies

- Status: Accepted
- Date: 2026-10-17
- Extends: [0038](0038-vendor-provisioning-profiles.md)

## Context

The user approved the complete FusionPBX library, device copies and categorized
manufacturer settings. Source files use different protocols and formats.
Some files have incorrect credential bindings. Two entries have no template.
Source coverage does not establish firmware capabilities or hardware limits.

## Decision

- Pin all original provisioning blobs and the literal function catalog at
  `20143aec4366afc2755f336d3058243479d49d44`. Never execute PHP or Smarty.
  Convert templates to bounded MiniJinja files at build time. Preserve source
  attribution and MPL-1.1 terms. Check reproducible generation in CI.
- Keep existing Yealink IDs and their adapter. Use vendor-prefixed IDs for
  additional profiles. Give each profile an explicit artifact manifest.
  Deliver files under an authenticated device directory. Reject undeclared
  paths. Never use a shared filename to select an arbitrary phone.
- Derive scalar field metadata from reproducible fixtures. Group fields by
  function and show source comments. Keep credentials and device identity
  under TalkOps control. Validate overrides against the model schema.
  Replace value ranges only. Escape XML and reject unsafe text delimiters.
- Store additive settings in `phones.provisioning`. Encrypt secrets and URLs
  with credentials. Mask them in API responses and previews. Exclude them
  from device copies.
- Extend semantic keys with an optional vendor catalog function UUID. Resolve
  the UUID only in that vendor's catalog. This extends the semantic-key
  decision in ADR 0038. Keep legacy Yealink key behavior unchanged.
- Treat source account bounds as rendering constraints. Without a source
  bound, start with one account. Permit a documented device count up to 100.
  Apply that count to assignment, rendering, validation, fields and copies.
  Reject a reduction that leaves an account outside the configured count.
- Copy a same-model configuration snapshot. Require a new name and identity.
  Generate independent SIP credentials for selected extensions. Exclude
  account labels, runtime state and device-specific values. Use one transaction
  for the phone, accounts and audit entry. Reject stale previews. Defer copied
  keys until their account is assigned. Send no resync.
- Give apps independent generated identifiers. Expose declared files and QR
  exports. Do not offer remote commands without a manufacturer adapter.
- Keep empty source entries blocked and visible. Do not create a selectable
  device from an empty source file.

## Consequences

No new migration is required. Migration `20261105000001` supplies JSON storage
and template revisions. Existing clients can omit the new settings. Existing
Yealink phones keep their model, assignments and credentials.

The embedded source library is larger. Schema caches, source sizes, evaluation
fuel and output sizes have explicit bounds. Fixtures test structure and
credential bindings. Real device and firmware tests remain separate evidence.

Spectralink contains only `empty.txt`. Grandstream Wave has an empty template.
The user explicitly accepted Spectralink as a missing-template entry.
Additional source files or model implementations are required for these entries.

## References

- [Approved proposal](../provisioning-all-vendors-proposal.md)
- [Original blob manifest](../research/fusionpbx-provisioning-files.json)
- [Function source manifest](../research/fusionpbx-function-sources.json)
- [Third-party notices](../../THIRD_PARTY_NOTICES.md)
