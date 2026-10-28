# 0048 - Independently authored provisioning profiles

- Status: Accepted
- Date: 2026-10-28
- Extends: [0038](0038-vendor-provisioning-profiles.md) and
  [0039](0039-source-derived-multi-vendor-provisioning.md)

## Context

The pinned FusionPBX library has no Cisco 7800 profile. Cisco documents MPP
provisioning independently. New profiles must not appear to be pinned source
files or generated FusionPBX derivatives.

## Decision

Keep independently authored profiles in separate manifests and template paths.
Embed these files at build time. Combine profile metadata at runtime and reject
duplicate IDs. Keep the original FusionPBX manifests, generators and blob hashes
unchanged. New profiles use stable vendor-prefixed IDs with a firmware suffix.

Apply the existing artifact allowlist, bounded MiniJinja engine, XML validation,
manufacturer fields, preview masking, encrypted storage and copy exclusions.
Document manufacturer evidence and account limits. Keep hardware validation at
`not_tested` until a recorded device test succeeds.

Cisco 7800 MPP profiles use one declared XML artifact in the authenticated device
directory. Enterprise firmware is unavailable because TalkOps lacks the required
bootstrap and trust services. The UI and bilingual guide state this limit.
No firmware conversion, new transport service or migration is included.

## Consequences

Native profiles can exist without upstream source blobs. The authored revision
identifies the TalkOps implementation, not a Cisco release or upstream commit.
CI continues to verify all pinned sources and generated files. Shared profile
fixtures and dedicated protocol tests also check the authored profiles.

The source-library view still contains upstream sources only. The model catalog,
configuration files and native template editor contain the authored profiles.
Hardware certification and license procurement remain separate activities.

## References

- [Cisco 7800 evidence](../research/cisco-7800.md)
- [English setup guide](../en/manufacturer-provisioning.md#cisco-7800-mpp-phones)
