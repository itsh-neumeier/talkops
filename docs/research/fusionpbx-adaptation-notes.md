# FusionPBX adaptation and validation notes

Source revision: `20143aec4366afc2755f336d3058243479d49d44`.
Research date: 2026-10-17.

The library has 386 source profiles. Of these, 385 contain usable templates.
The GXP1630 model adds one selector variant of the GXP16xx source profile.
Spectralink is a separate empty source entry. Grandstream Wave is the empty
profile in the profile count. No source entry is marked hardware-verified.

## Source corrections

The originals remain unchanged. `scripts/provisioning/adapt-source.mjs` records
the corrections to native derivatives:

- Remove stray upstream stash-marker lines from Flyingvoice output.
- Correct copied Yealink password bindings and the Cisco `assword` typo.
- Correct Htek account ID suffixes.
- Bind static ATCOM sample SIP fields to the assigned TalkOps account.
- Fill empty Grandstream GXP110x and GXP2100 credential blocks.
- Correct malformed Polycom and Grandstream template structure.
- Normalize account access through `account.N`, `user.N`, flat `_N` aliases,
  loops and indexed line maps. Bound account inference to credential blocks
  and account loops. Do not use unrelated key loops as hardware evidence.
- Give GXP1630 its explicit selector and third account. Keep generic GXP16xx
  at two source account slots.
- Route generated provisioning and contact URLs to the device directory.
  Keep TLS, port, path and authentication from the selected endpoint.
- Correct Algo's reversed HTTP authentication fields.
- Convert old PHP file queries to declared device files. Decode XML values
  when checking references. Exclude comments from those checks.
- Clear Snom firmware-status URLs without a source artifact. Use the selected
  firmware URL for Grandstream instead of the old local firmware folder.
- Clear the three Swissvoice directory defaults because the source has no
  directory file. The fields accept a separately hosted compatible directory.
- Supply CP965 directory files from the pinned Yealink T4x profile. Supply the
  missing Sangoma directory through the Htek P4401 source-format alias.
  These aliases do not certify either device.
- Bind Panasonic and Poly D230 configuration URLs to the actual primary file.
- Bind the older Snom language fields to the common language setting.
- Map contacts and extensions to their source categories and number records.
  Fixtures check that profiles with contact exports retain contact numbers.
- Correct Fanvil X7C function selection and its one-based account numbering.
- Resolve the two T4x boot-name spellings to one model-aware artifact. The
  older duplicate refers to a generic CFG file absent from that source folder.
- Expand static empty XML leaves to explicit empty values. Do not change
  dynamic tag names. This makes empty source settings editable.

The compiler supports a restricted Smarty subset at build time. MiniJinja
does not load files from the host or network. The renderer bounds source size,
fuel, recursion and output. XML values are escaped. Native text overrides must
not contain line breaks or quote delimiters. Source contact operations preserve
multiple numbers and bound Unicode truncation. PIN fields use the same secret
storage and copy exclusions as password fields.

## Function and field evidence

`app/devices/app_config.php` supplies literal function UUIDs, types and codes.
The importer does not execute PHP. Escene programmable functions have a
separate group constraint. Flyingvoice uses the source's Yealink-style key
parameters. Telekonnectors uses the source's Fanvil-style key format. These
format aliases are source adaptations, not device certification.

The field schema includes scalar values from each configuration fixture and
manufacturer variables from the source. Fields have categories, source names,
comments, defaults, secret flags and copy rules. The schema is not a complete
catalog of settings absent from the source or added by later firmware.
Do not describe the field count as a count of distinct hardware features.

Fixtures cover no accounts, one account and the profile account limit. They
check all declared artifacts, structure and each account's username and
distinct password. Linphone uses the expected HA1 digest instead of a plain
password. App fixtures require one account. The fixture report keeps source
adaptation and hardware validation separate.

## External protocol references

- [Fanvil auto-provisioning description](https://www.fanvil.com/Uploads/Temp/download/20191218/5df9dd9ce42f0.pdf):
  device filenames, HTTP authentication and configuration download behavior.
  The quarter-hour time-zone mapping is inferred from the source defaults
  and the manufacturer example with value 32. Hardware verification is pending.
- [Grandstream provisioning guide](https://documentation.grandstream.com/knowledge-base/sip-device-provisioning-guide/):
  `cfgMAC.xml` and shared fallback filenames. TalkOps resolves aliases only
  inside the authenticated phone directory.
- [Acrobits Account XML](https://doc.acrobits.net/cloudsoftphone/account/):
  one account root, SIP credentials and external provisioning properties.
- [Acrobits web contacts](https://doc.acrobits.net/api/client/web_contacts/):
  JSON contact records. TalkOps adds a declared `directory.json` export for
  the Groundwire source URL.

## Remaining evidence and integration work

Real device and firmware checks remain pending. Source rendering alone does
not prove registration, key LEDs, reboot, firmware upgrades or language packs.
Common locale mappings do not cover every manufacturer's format. The model
form exposes source-specific fields for those formats. Firmware and media
references need device-specific verification. Some old Cisco firmware needs
TFTP delivery outside TalkOps. Cloud Softphone needs the user's Cloud ID and
portal configuration.

The user explicitly accepted Spectralink as a missing-template entry on
2026-10-17. The pinned upstream tree supplies only an empty file. Grandstream
Wave also has no usable template. Both remain blocked in the library. A later
implementation needs an actual model and source. These entries do not receive
an invented configuration.
