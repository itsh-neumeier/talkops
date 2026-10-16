# Provisioning profiles, phone keys and transport setup

Status: Proposed. User review is required before application changes.
Date: 2026-10-16, Europe/Berlin.

## Goal and review boundary

Complete the Yealink T41P key controls. Import the complete FusionPBX Yealink
template library as a source for adaptation. Prepare the forms and rendering
interfaces for more than one phone vendor.

This proposal and its research records do not change runtime behaviour.
The user requested a proposal before implementation. Commit and PR permission
does not remove that review boundary. The release chat owns integration,
version changes, tags and publication.

## Verified starting point

- Remote target: `itsh-neumeier/talkops`, branch `main`.
- Base: `86772ec0c31990f7a29a639ffb90db6ebef217e2`, TalkOps 1.8.1.
- PR 47 is merged. TalkOps already has 21 desk phone profiles and a graphical
  editor for unused, line, BLF and speed dial keys.
- The phone catalog has a vendor field. The renderer, key types, setting
  parameters, firmware request parsing and phonebook output are Yealink-specific.
- `render_phone_config` always calls `yealink::render_phone`. Adding another
  vendor YAML file alone would not add a working vendor integration.
- Catalog setting keys share one namespace. Settings are filtered by phone
  family, not by vendor or model. This must change before another vendor uses
  the same form.
- Provisioning URLs and the SIP host are derived from request headers. The
  provisioning URL shown in the browser can differ from the address that an
  older phone can use.

Relevant code:
`crates/talkops-provisioning/src/{catalog,yealink}.rs`,
`crates/talkops-api/src/routes/{phones,provisioning}.rs`,
`presets/phones/yealink.yaml`, `web/src/lib/components/PhoneKeyEditor.svelte`.

## Template research

The source is pinned to FusionPBX commit
[`be0d0e3220e6b3666775fa46db6f1accc60c79c6`](https://github.com/fusionpbx/fusionpbx/commit/be0d0e3220e6b3666775fa46db6f1accc60c79c6).
All 433 files in all 70 Yealink directories were scanned: 159 CFG files,
75 boot files and 199 XML files. Family directories such as `t4x` are not
separate hardware models.

The [directory matrix and individual source links](research/yealink-template-inventory.md)
record the key groups in each directory. The
[JSON record](research/yealink-template-inventory.json) records every source
file, Git blob, key parameter pattern and first source line.

| Parameter group | Directories with assignments | Interpretation |
| --- | ---: | --- |
| `linekey.*` | 64 | Line key parameters; some files contain only a label field |
| `programablekey.*` | 63 | Idle softkeys and programmable hardware keys |
| `memorykey.*` | 28 | Legacy memory key blocks |
| `expansion_module.*` | 63 | Expansion parameters, often copied into unrelated profiles |
| Custom softkey layout | 61 | Call-state layout fields or layout URL fields |
| Defined handset keys | 1 | W80 handset controls |
| `dsskey.*` icon parameters | 2 | T58W and VP59 display assets |
| Alarm button parameters | 2 | AX83H and AX86R |

An assignment can be empty or use an unresolved placeholder. The table shows
template content. It does not certify hardware support. W52P, W56P, W60B,
W70B and W7XP have no editable key assignment in this scan. W80 uses handset
parameters instead of the desk phone key interface. VP59 has generic
programmable placeholders, but no programmable key loop. AX templates have
alarm parameters and a programmable loop; these do not prove desk key support.

The FusionPBX vendor registry has 49 function entries. Template comments have
older, incomplete lists. Neither list is a universal allowed-function list.

### T41P findings

The source contains six files. The common file is `y000000000036.cfg`.
The boot file includes the common file and `{$mac}.cfg`.

| Key group | Evidence and proposed use |
| --- | --- |
| Line keys | MAC template loop. Keep the verified TalkOps capacity of 15 logical keys. |
| Programmable keys | Common template loop for `programablekey.*`. Add these to the editor. |
| Idle softkeys | Part of the programmable group. Keep separate from in-call layouts. |
| Call-state softkeys | URLs for failed call, incoming call, connecting, dialing, ringback and talking. Add a separate layout editor and render its assets. |
| Memory and expansion keys | Present in the source, but do not enable them for T41P. |

The common file includes literal line key slots 1 through 27. This conflicts
with the T41P capacity. The programmable comment says 1 through 13. This is
also not an exact physical key map. The loop writes `pickup_value` and
`history_type` to key 1 instead of the loop key. These lines must not be copied
as working TalkOps logic.

The current [Yealink DSS reference](https://support.yealink.com/docs/sip-t42u/dss-keys/711335ef674b43e39937b51d22f7ae2d)
gives this T41P mapping:

| UI control | `programablekey` ID |
| --- | ---: |
| Softkey 1, 2, 3, 4 | 1, 2, 3, 4 |
| Up, down, left, right | 5, 6, 7, 8 |
| OK, cancel | 9, 10 |
| Mute | 13 |
| Redial, message | 17, 18 |

This matches the 13 controls in the user's screenshots. Do not renumber
redial and message as 11 and 12. The installed T41P firmware is `36.83.0.120`,
as reported by the user. Confirm the mapping on that firmware before activation.

The T41P template comments list these programmable function codes:

| Codes | Function names in the source |
| --- | --- |
| 0, 2, 5, 6, 7 | Unused, forward, DND, redial, call return |
| 8, 9, 13, 23, 32 | SMS, direct pickup, speed dial, group pickup, new SMS |
| 22, 27, 47 | XML group, XML browser, XML phonebook |
| 28, 29, 30, 33 | History, directory, menu, status |
| 31, 40, 50 | Switch account, prefix/PTT, keypad lock |
| 43, 44, 45, 46 | Local phonebook, BroadSoft phonebook, local group, BroadSoft group |

These are source hints. For example, the FusionPBX registry maps account
up/down to 48/49, while the current Yealink reference uses 51/52. The current
reference also uses directory code 61. Do not choose a code from one source
and apply it to all firmware generations. Use explicit firmware profiles and
keep unverified mappings visible as untested.

## Proposed implementation

### 1. Import and adapt the template library

- Import all 433 source files at the pinned commit into a separate reference
  directory. Preserve original bytes, paths, copyright and license metadata.
  Record source revision, file hashes and an update procedure.
- Keep the upstream reference separate from active TalkOps templates. No
  upstream PHP or Smarty code runs in TalkOps.
- Use the existing MiniJinja engine for native templates. Keep a clear
  `vendor/family/model` structure with boot, common, device and XML asset files.
  Reuse shared fragments when their parameters are the same.
- Add explicit profiles for model, firmware generation, requested filenames,
  template IDs and validated capabilities. A family folder is a source
  template. It is not automatically a selectable phone model.
- Give each imported source profile a visible adaptation state. Keep the
  current supported models usable. Do not advertise an imported profile as
  hardware-tested. Add new selectable profiles when their identity, filenames
  and capacity are known.
- Provide a template library with source view, clone and draft edit. Validate
  syntax, allowed variables, includes and output filenames before activation.
  Keep version history and a restore action. Scope custom templates to the
  tenant. A template cannot read files, run commands or access other tenants.
- Put phone-specific programmable keys in the per-device output. Do not put
  one phone's key values in a common file shared by all phones.

Upstream `resources/licenses.php` declares MPL 1.1. Imported files need the
applicable notices and provenance. Record this in `THIRD_PARTY_NOTICES` when
the import is implemented. This proposal imports no upstream template text.

### 2. Model capabilities and vendor adapters

Keep the common phone record and account management. Add a small vendor
adapter interface for requested file resolution, config rendering,
phonebook format, assets and resync capability. Implement Yealink first.
An unavailable adapter must return an explicit unsupported result.

Keep vendor parameter names and numeric function codes in the adapter profile.
Store semantic actions such as `speed_dial`, `directory` and `history` in the
common form data. Do not store Yealink function numbers as universal actions.

Profiles declare:

- Key groups, physical IDs, labels, page positions and supported functions.
- Required fields per function: target, account, URL, history source,
  phonebook, prefix or multicast address/channel.
- Account modes per function. Local menu functions need no SIP account.
  Some functions use an automatic account; this is not an unassigned account.
- Value limits, firmware requirements and required phone/PBX features.
- Model-specific settings and media capabilities. Vendor defaults cannot
  write a parameter from one vendor into another vendor's config.

A second adapter can then use the same forms without Yealink field names.
Preparing the interface does not claim tested provisioning for another vendor.

### 3. Key forms

Keep the existing graphic and list editors. Add key groups rather than one
long list of interchangeable slots:

1. Line keys and pages.
2. Idle softkeys and hardware/navigation keys.
3. Call-state softkey layouts.
4. Memory keys and expansion modules, only where the profile permits them.

For the T41P, show all controls in the screenshots with their real IDs.
Select a function first. Show only the fields that the function uses.
History gets a history source. A directory key gets a phonebook selector.
A local menu/status action has no target number. Speed dial keeps contact,
internal extension and manual target selection.

Offer unused and inherited/default as different states. Show whether a value
comes from the template, a phone default or a device override. Preserve
existing overrides and unsupported old values during unrelated edits.
Before save, show a redacted configuration preview and a parameter diff.
Keep Save and Save & resync, including their existing failure handling.

Add the local functions that the profile supports: DND, forward, redial,
call return, history, directory, menu, status, prefix, lock, account switching,
XML browser and phonebook access. Add line-key actions such as voicemail,
pickup and DTMF with validated TalkOps parameters.

Keep three states for functions: supported with TalkOps, phone-local, and
requires an external or missing service. SMS, BroadSoft services, ACD,
shared lines, hot desking and vendor recording must not appear as working
TalkOps integrations merely because a source entry exists. Distinguish
phone-local DND/forward from PBX-side state; preserve and test synchronization.

### 4. Provisioning server and DHCP setup

The user confirmed that both attempts used `/provisioning/`. The spelling
in the first message was a chat typo. It is not the reported failure cause.
The successful path was manual internal HTTP. HTTPS through DHCP did not work.
The user also confirms that HTTPS provisioning works on the AX86R through
DHCP option 66. The installed T41P firmware is `36.83.0.120`.

Read-only requests from the development computer confirmed:

- `https://talk.neumeier.cloud/provisioning/y000000000036.cfg` returns 401
  with the TalkOps Basic-auth challenge. The computer completed TLS without
  bypassing certificate checks.
- `http://192.168.140.30:8095/provisioning/y000000000036.cfg` returns the
  same challenge.
- Local DNS resolves the domain to `192.168.140.30`.

These checks do not prove that the phone has the same DNS result, accepts
the certificate, sends credentials or reads option 66. No authenticated
phone configuration was downloaded. No phone or DHCP setting was changed.
The AX86R result makes a general HTTPS endpoint failure less likely. It does
not distinguish a T41P TLS/authentication problem from a DHCP delivery problem.
Compare the device-specific DHCP replies rather than assuming their contents
are identical. The next useful phone test is manual HTTPS on the T41P. Record
whether a request reaches TalkOps and which authentication result it receives.
Do not assume that T41P firmware has no HTTPS support.

Proposed changes:

- Add an explicit device provisioning endpoint and optional per-profile
  override. Keep the web UI URL, provisioning endpoint and SIP server address
  as separate settings. Use the configured endpoint for boot includes,
  phonebooks, firmware, media and action URLs.
- Show the scheme, host, port, path, authentication and copied DHCP value
  separately. Provide separate manual-setup instructions.
- Explain that DHCP option 66 provides a server value. It does not select
  TFTP by itself. TalkOps currently serves provisioning through HTTP(S).
  A TFTP address cannot use the existing HTTP handler.
- Keep HTTPS as the normal option. Permit an explicit LAN HTTP endpoint for
  older phones. Do not silently downgrade HTTPS or disable certificate checks.
- Add diagnostics for requested filename, model/firmware when known, result
  and auth outcome. Never log credentials, URL userinfo or query secrets.
- Make missing phone files distinguishable from the SPA fallback. Test
  unsupported `.boot` requests and exact model/common filenames before adding
  boot-file handling.

The hardware investigation changes one variable at a time:

| Test | Address delivery | Transport | Question |
| --- | --- | --- | --- |
| Existing working case | Manual | LAN HTTP | Confirm the current baseline with firmware and auth mode recorded |
| Next | DHCP 66 | Same LAN HTTP | Does the phone read the option and use its credentials? |
| Next | Manual | HTTPS | Does DNS, time, TLS and authentication work on the phone? |
| Final | DHCP 66 | Same HTTPS | Does the complete intended setup work? |

Inspect DHCP options 66, 43 and any configured custom priority; do not assume
that the firmware selects option 66 first. Check phone time, certificate
chain, firmware, HTTP auth and redirects. If needed, capture a short DHCP and
provisioning trace on the LAN. Keep the capture limited to the test phone.
An actual TFTP service is a separate deployment choice. It is not required
just because the DHCP UI calls option 66 a TFTP server field.

## Storage and compatibility

No migration exists in this proposal. No migration ID is reserved.
Before implementation, check all current target and open-PR migration IDs.

Proposed storage changes need forward-only migrations:

- Versioned, tenant-scoped custom template profiles and their activation state.
- A phone profile selection and typed key-group overrides beyond `line_keys`.
- Explicit provisioning endpoint settings and optional profile overrides.

Keep the current `line_keys` API compatible during the transition. Existing
phones use the current Yealink profile unless an administrator selects another.
Preserve old overrides and firmware/media assignments. Do not rewrite SIP
secrets or change endpoint URLs automatically during an upgrade. New saved
profile/settings changes are audited without secrets and enforce permissions.

Template activation must validate all affected profiles before use. A failed
draft cannot replace the active revision. Keep generated secrets out of draft
source, browser previews and diffs. The existing audited raw-config view remains
a separate privileged action.

## Implementation checks after approval

- Validate the complete import manifest against the pinned source. Check
  notices and source byte hashes. Require an explicit source update.
- Add renderer fixtures for every active native profile and file kind. Check
  unresolved variables, filenames, includes, parameter generations and limits.
- Test T41P programmable IDs 1-10, 13, 17 and 18. Test both unused and inherited
  values. Test function-dependent fields and rejected unsupported combinations.
- Test history, directory, phonebook selection, pickup and account switching
  against the exact firmware profile. Check the key-1 source-template defect
  cannot overwrite a different key.
- Test that unrelated phone saves preserve legacy keys. Test migration upgrade
  data, API compatibility, tenant isolation, permissions and audit redaction.
- Test vendor scoping of settings and an unavailable second adapter. No
  Yealink parameter may appear in a non-Yealink rendered result.
- Test Basic auth, rate limiting, HTTP/HTTPS endpoints, exact boot/common/MAC
  filenames, proxy headers, no SPA config responses and asset URL consistency.
- Run browser tests for groups, field visibility, save/resync errors, keyboard
  access, German/English text and responsive layouts.
- Run Rust fmt, full Clippy and workspace tests, web lint/check/test/build,
  Compose checks and the applicable CI/E2E suites. Report service/tool limits.
- Test the actual T41P for DHCP provisioning, idle keys, call-state keys,
  phonebook selection, resync and reboot persistence. Do not infer this from CI.

## Proposed delivery order

1. Review this proposal and the source matrix with the user.
2. Import the reference library, add profiles and compatibility storage.
3. Complete the T41P key groups and shared forms; adapt the native templates.
4. Add explicit endpoints, provisioning diagnostics and the DHCP test guide.
5. Run automated and hardware checks. Submit implementation PRs with migration
   IDs, dependencies and English/German user documentation.

Keep new functions and imported models marked untested until their checks are
complete. The user review can change this order or the template editor scope.

## Suggested release notes after implementation

Add model and firmware profiles for phone provisioning. Extend Yealink T41P
setup with programmable keys and softkey layouts. Add an adaptable Yealink
template library based on FusionPBX. Prepare common forms for vendor adapters.
Allow an explicit device provisioning endpoint and improve DHCP diagnostics.

These are proposed notes. They do not describe the current application.

## Review questions

- Does the proposed separation of key groups match the required workflow?
- Should template cloning and draft editing be included in the first delivery?
- For the transport investigation, confirm whether the same provisioning
  credentials were supplied in both T41P attempts. Test manual HTTPS with
  firmware `36.83.0.120` before changing the transport architecture.

## References

- [Shared release workflow](release-workflow.md).
- [Current key editor proposal](deskphone-key-editor-proposal.md).
- [Current phone guide](en/yealink.md).
- [FusionPBX per-directory inventory](research/yealink-template-inventory.md).
- [FusionPBX function registry](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/app/devices/app_config.php#L1290).
- [FusionPBX T41P source](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t41p).
- [Yealink DSS reference](https://support.yealink.com/docs/sip-t42u/dss-keys/711335ef674b43e39937b51d22f7ae2d).

Language review used the repository clarity rules. The full ASD-STE100
standard was unavailable. Full STE language review is pending.
