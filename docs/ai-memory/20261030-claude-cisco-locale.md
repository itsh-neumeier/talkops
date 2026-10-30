# Cisco Enterprise locale upload

## Task state

- Task ID: 20261030-claude-cisco-locale
- Date: 2026-10-30
- Tool: Claude Code
- Status: In progress
- Branch: claude/cisco-locale
- Base commit: main, v1.8.20
- Related commits or PR: PR109, release 1.8.21, locale directory fix

## Goal

A Cisco Enterprise phone (CP-7861, 14.2(1)) with the device language German
must show German menus. TalkOps must not ship Cisco files.

## Result

The phone log (`/FS/main_*.tar.gz`) showed that the phone requests
`/sp-sip.jar` from the TFTP root after a `userLocale` entry in the SEP file.
The user has the Cisco package `po-locale-de_DE-14.3.1.5000-1.cop.sha512`.

Package layout: a signature header (TLV, `PT=ccmPatch_cop3`), then a gzip tar.
The outer tar has `po-locale-de_DE-<version>.tar`, `copstart.sh` and
`action.txt`. The inner tar has `usr/local/cm/tftp/german_germany/*` (about 45
files, with `sp-sip.jar`) and
`usr/local/cm/application_locale/localeinstaller/german_germany/localeversion.txt`
("User Locale Version : 14.3(1.9900)DEU").

- Upload: `POST /api/v1/phone-locales/cisco` (multipart `file`), list and
  delete. Permission `PhonesManage`, audit entries.
- `cisco_locale::parse` reads only the TFTP files of one locale directory and
  never runs the package scripts. Limits: package 32 MiB, file 4 MiB, total
  24 MiB, 200 files, safe file names only.
- Storage: `<provisioning_dir>/cisco-locales/<Name>/{locale.json,files/}`.
- The SEP file gets `<userLocale>` when the phone language matches the
  `lang_code` prefix (`de` → `de_DE`).
- The TFTP exporter reads the `userLocale` name from the rendered SEP file and
  adds the locale files to `active/<ip>/`. A new map rule serves root requests
  such as `/sp-sip.jar` from that directory.
- UI: **Phones → Cisco Enterprise languages**.

## Changes

- `crates/talkops-provisioning/src/vendors.rs`: `CiscoLocale`,
  `select_cisco_locale`, `Common::cisco_locale`.
- `crates/talkops-provisioning/templates/talkops/cisco7800/enterprise.cnf.xml.jinja`:
  `userLocale` block.
- `crates/talkops-api/src/cisco_locale.rs`, `src/routes/cisco_locales.rs`:
  parser, storage, API.
- `crates/talkops-api/src/routes/provisioning.rs`: locale selection.
- `crates/talkops-api/src/cisco_tftp.rs`: locale files in the export.
- `docker/talkops/cisco-tftp.map`, `tests/provisioning/tftp.py`: root rule and
  test.
- `web/src/lib/components/CiscoLocaleManager.svelte`, phones page, i18n.
- `crates/talkops-api/tests/phones.rs`: upload, SEP file, export, delete.
- Docs EN/DE, `CHANGELOG.md`.

## Decisions

- Store files in the provisioning directory, not in the database. The files
  are vendor data and can be large.
- Network locales (call progress tones, `g3-tones.xml`) are not supported. The
  de_DE user locale package does not contain them. This needs the separate
  Combined Network Locale package.

## Checks

- See the PR description for the final check results.

## Dependencies and open work

- Device test on the CP-7861 after a release.
- Network locale support when the user supplies the package.

## Device test on 1.8.21

The phone stayed English. A TFTP capture on the host showed
`RRQ "German_Germany/sp-sip.jar"` and `RRQ "/g3-tones.xml"`. The map rule
accepted only root names, so the phone got an error. The fix accepts one
locale directory level (`[A-Za-z][A-Za-z_]{0,63}/`) and maps it to the flat
per-phone export. The `/sp-sip.jar` root request seen earlier stays allowed.

The public directory URL returned 307 from the reverse proxy (also with a bad
token). TalkOps returns 200 on `http://192.168.140.30:8095`. The user set that
provisioning address on the phone.

## Next action

Open the PR, then release and do the device test when the user asks.

## References

- `docs/ai-memory/20261030-claude-cisco-tftp-refresh.md`
- `docs/ai-memory/20261030-claude-cisco-ccm-layout.md`
