# 0038 - Vendor provisioning profiles and device transport overrides

- Status: Accepted
- Date: 2026-10-16

## Context

TalkOps had a vendor field in its phone catalog. Rendering and key controls
were specific to Yealink. T41P firmware 36.83.0.120 needs programmable keys
in addition to the existing line-key editor. The source templates contain
copied parameter blocks and different numeric function codes. Their presence
does not prove a model capability.

The user reports that AX86R provisioning works through HTTPS and DHCP option
66. T41P provisioning and resync work through internal HTTP but fail through
HTTPS. No phone trace establishes the TLS or authentication failure cause.
The web browser address must not force every phone to use the same transport.

## Decision

- Keep semantic key functions in the API. Use explicit model capabilities
  and firmware profiles to map the functions to vendor parameters.
- Use a vendor adapter for configuration and phonebook rendering. Reject a
  vendor with no implementation. Yealink is the first implemented adapter.
- Keep the existing line-key storage and API fields. Store additional key
  groups, call-state layouts, template selection and transport overrides in
  `phones.provisioning`. An old API client that omits this field preserves it.
- Keep the complete, pinned FusionPBX Yealink source tree separate from the
  native MiniJinja files. Store original bytes and verify the Git blob IDs.
  Never execute upstream PHP or Smarty. An imported source directory does not
  automatically become a supported phone model.
- Provide native template copies per tenant and model. Revisions are
  immutable. Activation validates the native files, model fixtures and assigned
  phones before the active revision changes. An older revision can be activated
  again. No filesystem or network template loader is installed.
- Limit template source size, evaluation fuel, recursion and output size.
  Replace generated secrets before rendering a browser preview.
- Store a tenant provisioning address and SIP host separately from the web UI
  address. A phone can override either address. An empty value preserves the
  existing request-derived behaviour. No transport is downgraded automatically.
- Repeat the effective common parameters in the device file. This applies
  device-specific URLs after a common-file request that cannot identify the
  device. Use that same endpoint for phonebooks, firmware, media and softkeys.
- Record bounded configuration filenames, MAC, scheme, HTTP result and time.
  Store no headers, query strings, credentials or configuration bodies. Keep
  the latest 2,000 records per tenant. Return at most 100 records per query.
- Treat SIP NOTIFY and the later configuration download as separate results.
  A TLS failure before HTTP reaches TalkOps cannot appear in the request list.

## Consequences

- Existing accounts, phone IDs, secrets and line-key overrides remain valid.
- The UI can select functions and fields from model metadata. Settings are
  scoped to vendor, family and optional model IDs. New vendor setting keys
  must be unique; use a vendor prefix for a new vendor's keys.
- A model-specific HTTP exception permits a legacy phone on a trusted LAN
  while other devices use HTTPS. Certificate verification remains active.
- The database needs migration `20261105000001_provisioning_profiles`.
- The imported source adds about 13 MB to the repository and to embedded
  source data. License notices are included in the source and server image.
- Real-device checks remain necessary. Automated rendering does not prove
  that a firmware implements a function or accepts a certificate chain.

## References

- [Approved proposal](../provisioning-multivendor-proposal.md).
- [Source inventory](../research/yealink-template-inventory.md).
- [Third-party notices](../../THIRD_PARTY_NOTICES.md).
