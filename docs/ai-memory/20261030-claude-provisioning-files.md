# Provisioning tabs and provisioning files

## Task state

- Task ID: 20261030-claude-provisioning-files
- Date: 2026-10-30
- Tool: Claude Code
- Status: In progress
- Branch: claude/provisioning-files
- Base commit: 5e13f5d (main, v1.8.22)
- Related commits or PR: to be added

## Goal

The user wants a TFTP and provisioning file explorer with an editor, for Cisco
and for the other phones, and a better structure of the provisioning area:
General, Cisco Enterprise, Debug / advanced.

## Result

- The phones page has the tabs Phones, General, Cisco Enterprise and
  Debug / advanced (`?tab=`). Only users with phone management see the tabs.
- ADR 0052: table `provisioning_files` (transport `http` or `tftp`, optional
  phone for TFTP, path, encrypted content). A file with the name of a
  generated file replaces it; another name adds a file.
- HTTP: `/provisioning/{path}` serves a stored file after the credential
  check. TFTP: the exporter adds the stored files (all phones first, then the
  phone's own files).
- API: `GET/PUT /api/v1/provisioning-files`, `GET/DELETE
  /api/v1/provisioning-files/{id}`. `PhonesManage`, audit without content.
- UI: `ProvisioningFileExplorer.svelte` shows the files of a phone (from
  `provisioning_files` in the phone detail and the config preview), edits a
  generated file as a new stored file, and lists and edits stored files.

## Decisions

- The user chose "both": overrides of generated files and extra files.
- Contents are encrypted because a copy of a SEP file contains the SIP
  password.
- TFTP names must match the TFTP map (root names `.xml`, `.jar`, `.dat`,
  `SEP<MAC>.cnf.xml`). Ringtone `.rwb` files are not editable.

## Checks

- Passed (rust:1-trixie + Postgres 17): `cargo fmt`, `cargo clippy -p
  talkops-core -p talkops-api --all-targets -D warnings`,
  `cargo test -p talkops-core --lib provisioning_files`,
  `cargo test -p talkops-api --test provisioning_files`.
- Passed (node:22): `npm run check`, `npm test`, `npm run build`, Prettier on
  changed files.
- Not done: a browser test of the new tabs and dialogs.

## Dependencies and open work

- Other user requests from the same chat: Cisco ringtones, Cisco key layout,
  phone language list from uploaded Cisco locales.
- PR113 (Cisco directory menu) is separate and not merged.

## Next action

Open the PR and wait for CI.

## References

- docs/adr/0052-admin-provisioning-files.md
- docs/ai-memory/20261030-claude-cisco-locale.md
