# All-vendor provisioning and phone configuration copy

Status: Proposed. User review and feedback are required before implementation.
Date: 2026-10-17, Europe/Berlin.

## Goal and review boundary

Support all usable vendor templates in the FusionPBX provisioning tree.
Let an administrator add an identical phone with settings from an existing phone.
Keep each phone's identity and SIP credentials separate.

This change contains the proposal and source inventory only. No runtime code,
database migration, active template or production setting changes in this PR.
The earlier Yealink approval applies to PR 49. It does not approve this extension.
After user approval, implement the work below on the feature branch. Keep the
PR as a draft until implementation and the required checks are complete.
The release chat owns integration, release versions, tags and publication.

## Verified starting point

- Target: `itsh-neumeier/talkops`, remote default branch `main`.
- Base: `c4765b0495adf004f0b2b0f5099cbd098f384f86`.
- PR 49 is merged. ADR 0038 defines vendor dispatch, semantic keys, template
  revisions, device transport overrides and download diagnostics.
- `adapter.rs` dispatches configuration and phonebooks only for Yealink.
  Its request type is still `yealink::PhoneSetup`.
- `library.rs` requires two CFG files and a Yealink header. The source library,
  source API, template creation and activation remain specific to Yealink.
- The public download route resolves Yealink files and phonebooks. Firmware
  detection, key validation and resync also need vendor-specific behavior.
- The catalog supports a vendor field and scoped settings. Model IDs are global.
  Family variants cover desk, DECT, conference and Wi-Fi devices only.
- Phone creation starts with empty keys. There is no phone-copy operation.
  Template cloning copies template text; it does not copy a phone's settings.

Relevant files:
`crates/talkops-provisioning/src/{adapter,catalog,keys,library,yealink}.rs`,
`crates/talkops-api/src/routes/{phones,provisioning,provisioning_admin}.rs`,
`crates/talkops-core/src/{phones,phone_provisioning}.rs`,
`web/src/lib/{api.ts,phoneAssign.ts}` and `web/src/routes/phones/`.

## Source coverage

The inspected FusionPBX revision is
[`20143aec4366afc2755f336d3058243479d49d44`](https://github.com/fusionpbx/fusionpbx/tree/20143aec4366afc2755f336d3058243479d49d44/resources/templates/provision).
The complete Git tree response was not truncated. It contains 31 top-level
vendor or app directories and 1,037 files in the provisioning tree.
The [JSON inventory](research/fusionpbx-all-vendors-inventory.json) records
directory names, file counts, filename patterns and example paths.
The counts below include root files, assets and documentation.
Template directories are immediate child directories, not verified model counts.

| Vendor or app | Files | Template directories |
| --- | ---: | ---: |
| 2n | 2 | 2 |
| aastra | 12 | 6 |
| acrobits | 1 | 1 |
| algo | 7 | 7 |
| atcom | 1 | 1 |
| avaya | 9 | 4 |
| cisco | 49 | 26 |
| digium | 18 | 6 |
| escene | 4 | 1 |
| fanvil | 33 | 32 |
| flyingvoice | 168 | 42 |
| grandstream | 113 | 69 |
| groundwire | 2 | 1 |
| htek | 10 | 6 |
| linksys | 5 | 5 |
| linphone | 1 | 1 |
| mitel | 12 | 6 |
| obihai | 4 | 4 |
| panasonic | 16 | 9 |
| poly | 4 | 3 |
| polycom | 80 | 38 |
| sangoma | 3 | 3 |
| sipnetic | 1 | 1 |
| snom | 35 | 32 |
| spectralink | 1 | 0 |
| swissvoice | 4 | 2 |
| telekonnectors | 2 | 2 |
| vtech | 2 | 1 |
| yealink | 433 | 70 |
| yeastar | 3 | 3 |
| zoiper | 2 | 1 |

Spectralink contains only `empty.txt`. It has no usable configuration template
in this revision. Record this gap explicitly; do not offer a working profile.
Acrobits, Groundwire, Linphone, Sipnetic and Zoiper need app setup or export
flows. Gateways, ATAs, DECT bases, paging devices and intercoms also need
different controls from desk phones. Provisioning an intercom does not add
door opening, camera access or vendor event integration.

## Proposed design

### 1. Complete source library and coverage record

Import the complete pinned source tree as reference data. Keep original bytes,
source paths and Git blob hashes. Check notices and license requirements for
each imported file. Extend `THIRD_PARTY_NOTICES.md` and the reproducible source
check. Keep the existing Yealink snapshot stable until its differences are
reviewed. Record each source revision if both snapshots remain in use.

Do not execute upstream PHP or Smarty. Adapt configuration templates to native
MiniJinja with explicit context fields and output validation. Inventory required
variables, loops, includes, encodings and assets before each adaptation.
Where a template needs FusionPBX application data, map that data to TalkOps
or record the missing feature. Do not silently emit empty credentials or URLs.

Give every source template directory a coverage record. Use separate fields
for adaptation (`reference_only`, `adapted`, `blocked`) and device validation
(`not_tested`, `verified`, with model and firmware evidence). Record block
reasons. Family directories and shared files must map to explicit profiles.

### 2. Vendor-neutral rendering and request routing

Extract common phone, account, network, locale, contact and media types from
the Yealink setup. Keep vendor parameter names and numeric codes in adapters.
Use an adapter contract for these operations:

- Resolve a requested path to a model, device and allowed output artifact.
- Describe required common, boot, device, directory and app export artifacts.
- Render each artifact with the correct media type, encoding and escaping.
- Validate supported settings, keys, account limits and firmware profiles.
- Describe firmware download, resync and reboot support and their parameters.
- Parse known user agents without guessing unsupported firmware capabilities.

Use an explicit artifact manifest per profile. Examples include
`SEP<MAC>.cnf.xml`, `cfg<mac>.xml`, `MN_<mac>.cfg` and multi-file boot flows.
Shared filenames such as `directory.xml` need a vendor/model route context.
Use scoped paths for new profiles. Preserve the current Yealink URLs.
For a phone that requires a fixed root path, define an unambiguous bootstrap
binding. Do not select a vendor from an ambiguous extension or user agent alone.
Shared artifacts must not contain one phone's accounts or device overrides.

Generalize template validation from two Yealink files to the profile manifest.
Keep immutable revisions, tenant scope, strict variables and rendering limits.
Use format-specific escaping and parsers for XML, CFG, CSV and app exports.
Validate generated paths, included assets and all generated configuration URLs.
Preserve secret redaction, authentication, audit records and download limits.
Return an explicit unsupported result for an unavailable operation.
Keep NOTIFY delivery separate from a later configuration download.

### 3. Catalog and user interface

Keep existing Yealink IDs and saved data. Add vendor-prefixed IDs for new
models, for example `snom-d785`. Keep upstream vendor names as source identities;
do not merge `poly` with `polycom`, or `cisco` with `linksys`, without a mapping.
Extend device families for ATAs, gateways, intercoms, paging devices and apps.
Map these families to existing SIP device behavior where possible.

Show vendor, model, adaptation status and device validation separately.
Offer native profiles only when their required configuration flow is implemented.
Render forms from the model's capabilities. Hide unsupported key groups,
phonebooks, wallpapers and resync actions. Add types for validated text, numeric,
URL and secret settings when a profile needs them. Store secret settings with
the existing encryption service; never in plain settings JSON or template text.

Keep common actions such as BLF and speed dial semantic. Each adapter maps them
to its own function codes. Do not reuse Yealink's account or key limits.
Preserve the existing settings precedence: device override, then tenant default.
Add English and German UI text and operation documents.

### 4. Copy settings when adding an identical phone

Add **Use settings from an existing phone** to phone creation. Add **Clone**
to the source phone's action menu. Both actions open the same creation form.
Limit the first version to the same vendor, model and compatible profile.
A different model requires a separate, explicit conversion design.

The form shows copied fields and excluded fields. Require a new name and MAC
for hardware. App profiles use their declared identity scheme. Let the admin
select new extension assignments before creation. Copy a snapshot; later source
changes do not update the new phone.

| Setting or state | Proposed copy rule |
| --- | --- |
| Model, compatible firmware profile | Copy |
| Comfort settings, keys, expansion modules, call-state layouts | Copy validated settings |
| Ring tone, wallpaper, phonebook sections | Copy compatible references in the same tenant |
| Native template selection | Copy the same model-scoped reference; keep existing revision activation behavior |
| Provisioning endpoint and SIP server override | Copy after validation; identify an inherited HTTP exception in the form |
| BLF and speed dial destination numbers | Copy and show them for review |
| Key account indices | Keep slot numbers; bind them only to new assignments |
| Source account labels, display names, extension assignments | Do not copy automatically; select new assignments |
| Phone ID, MAC, device IDs, SIP usernames and passwords | Never copy; require or generate new values |
| Per-device hostname, static IP, certificates, private keys, tokens | Never copy |
| Last IP, firmware report, last seen, registrations, diagnostics | Start empty |
| Tenant provisioning and phone admin credentials | Resolve through the existing tenant policy; do not copy or rotate them |
| Unknown fields or raw vendor parameters | Reject or exclude with a visible reason |

Use a server-side allowlist for copyable settings. Mark identity and secret
fields in each profile. Validate URL actions for embedded source MACs, device
IDs and tokens. Exclude an action that cannot be safely rebound. Rebuild all
generated callback, firmware and provisioning URLs for the new phone.

Propose `POST /api/v1/phones/{id}/clone-preview` for a redacted, read-only draft
and `POST /api/v1/phones/{id}/clone` for creation. The final request contains the
source ID, reviewed copy options, new identity and explicit account assignments.
Return a conflict if the source changed after preview. Validate again on submit.
Use one transaction for the new phone, optional new SIP devices and audit entry.
Generate separate SIP credentials even when the same extension is selected.
Show account-dependent keys as unassigned until the required new account slots
are selected. Do not provision those keys with the source phone's account data.
Roll back on an invalid assignment or duplicate MAC. Require `phones.manage`
and all permissions needed to create the requested SIP devices. Enforce tenant
scope for the source, template, media, sections and extensions.
Do not resync either phone automatically after cloning.

## Implementation sequence and completion criteria

1. Generalize the adapter contract, manifests, validation and scoped routing.
   Preserve all current Yealink behavior with regression checks.
2. Implement phone copy with its allowlist, account binding, transaction and UI.
3. Import the full reference tree and implement all usable template families.
   Start with Grandstream, Snom, Fanvil and Poly/Polycom, then complete the
   remaining hardware vendors and app flows. This order does not reduce scope.
4. Complete the coverage matrix, documentation and final automated checks.
   Record model and firmware evidence from available real devices.

Complete software support means that every usable source family has an adapter,
native artifacts, required data mappings and passing fixtures. Each capability
exposed in the UI must be implemented. A reference import alone is incomplete.
Record empty or unusable upstream entries as explicit source gaps. Report any
other blocked family before calling the requested all-vendor task complete.
Never label a profile as device-verified without a recorded device check.

## Validation plan

- Renderer fixtures for every adapted profile and artifact, including maximum
  accounts, unused account reset, supported keys, assets and encodings.
- Routing tests for filename collisions, MAC case and separators, shared files,
  bootstrap bindings, path traversal, unknown models and unsupported operations.
- Preview and template tests for redaction, escaping, invalid output, missing
  variables, activation, restore, size limits and bounded evaluation.
- PostgreSQL integration tests for copy allowlists, source preservation, distinct
  SIP credentials, same-extension copies, empty runtime state, account mapping,
  duplicate MAC races, stale preview, rollback, tenant isolation and permissions.
- Browser tests for creation from a source, all capability-driven form groups,
  preview exclusions, new assignments, errors and mobile/desktop layouts.
- Source integrity checks for every imported blob and coverage entry.
- Run the CI commands in `.github/workflows/ci.yml`: Rust fmt/clippy/tests,
  web lint/check/tests/build, source and browser checks, Compose and image checks.
  Run SIPp/browser softphone E2E for the final implementation.
- Device acceptance per available model/firmware: cold boot, download, unique SIP
  registration, resync, key behavior and contacts. Check existing T41P HTTP and
  AX86R HTTPS behavior. Record unavailable hardware checks explicitly.

## Data, dependencies and release handoff

This proposal adds no migration and no runtime dependency. Reuse migration
`20261105000001` as the existing schema baseline; never edit it.
Phone copy can use existing phone and SIP device tables. Select new migration
IDs only if implementation needs persistent profile, identity or setting fields.
Check IDs against the current target and concurrent PRs before adding them.

Extend ADR 0038 with an additive ADR if the approved design requires new storage
or bootstrap identity rules. Check renderer and parser needs before selecting
new crates. No PHP service or Smarty runtime is proposed. Hardware access is
needed for device verification; software adaptation can proceed separately.

Suggested future release notes, only after implementation:

> Add provisioning adapters and model capabilities for the usable FusionPBX
> vendor templates. Show adaptation and device validation status separately.
> Add phone creation from an identical phone's settings with separate identities
> and new SIP credentials. Preserve existing Yealink provisioning URLs and data.

## Requested feedback

Review the full vendor scope, the implementation order and the copy rules.
The recommended default is an identical-model snapshot with new identities and
explicit extension selection. User approval of this proposal is the next step.

## References

- [Existing accepted design](adr/0038-vendor-provisioning-profiles.md).
- [Earlier Yealink proposal](provisioning-multivendor-proposal.md).
- [Existing implementation PR 49](https://github.com/itsh-neumeier/talkops/pull/49).
- [Pinned upstream request resolver](https://github.com/fusionpbx/fusionpbx/blob/20143aec4366afc2755f336d3058243479d49d44/app/provision/resources/classes/provision.php).
- [Task summary](ai-memory/20261017-codex-provisioning-all-vendors-proposal.md).
