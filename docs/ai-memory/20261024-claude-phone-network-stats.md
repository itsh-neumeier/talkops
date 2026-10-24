# Network data of provisioned phones from the Ubiquiti Networks API

## Task state

- Task ID: 20261024-claude-phone-network-stats
- Date: 2026-10-24
- Tool: Claude Code
- Status: Completed
- Branch: `claude/phone-network-stats` (worktree `../talkops-netstats`)
- Base commit: `986a2f6` (head of `claude/ubiquiti-api-naming`, PR #73)
- Related commits or PR: Pending

## Goal

PR 2 of 6 from one user request. After an administrator selects a setting,
the phone list shows the Wi-Fi or network values of each provisioned phone.
The user accepted the proposal: the switch is in the Ubiquiti Networks API
settings.

## Result

- Switch "Show network data on phones" in Settings → Wi-Fi survey →
  Ubiquiti Networks API → Phones. Off by default.
- Phone list: column "Network" (Wi-Fi: signal bars, dBm, AP, band, channel,
  SSID; cable: switch, port, link speed; else "not connected").
- Phone page: card "Network" with IP, network/VLAN, connection time,
  traffic, data rate and experience value. Refresh each 30 s.

## Changes

- `migrations/20261111000001_unifi_phone_network.sql`: column
  `unifi_connections.phone_network BOOLEAN NOT NULL DEFAULT false`.
- `crates/talkops-unifi/src/lib.rs`: `NetClient`, `Client::clients`
  (classic `stat/sta`, GET only), `parse_clients` with unit test.
- `crates/talkops-core/src/unifi.rs`: field `phone_network`,
  `set_phone_network`.
- `crates/talkops-api/src/routes/phone_network.rs`: `GET
  /api/v1/phones/network` (`phones.view`), `PUT /api/v1/unifi/phone-network`
  (`unifi.manage`, audit `unifi.phone_network`), 15 s cache per tenant.
- `crates/talkops-api/tests/phone_network.rs`: mock console, roles,
  matching, names, cache, unreachable console.
- Web: `phoneNetwork.ts`, `PhoneNetworkCell.svelte`,
  `PhoneNetworkCard.svelte`, `UnifiSettings.svelte`, phone list and phone
  page, i18n EN/DE, `tests/phoneNetwork.test.ts`.
- `tests/browser/phone-network.mjs` and a CI step in `ci.yml`.
- Docs: `docs/en/provisioning.md`, `docs/de/provisionierung.md`.

## Decisions

- One `stat/sta` request for all clients instead of one request per phone.
  Matching by MAC in TalkOps.
- Cache key includes console URL and site, so a changed connection reads
  again at once.
- Without the switch, the API sends no console request.
- Device names come from `ap_names` (`stat/device-basic`), which lists
  switches too. Without names, the UI shows the MAC.
- Wi-Fi fields of a wired client are dropped; `satisfaction` outside
  0 to 100 is dropped.
- The column uses `col-opt` and is hidden on narrow screens like the other
  optional columns; the phone page shows the card on all widths.

## Checks

- Passed: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `cargo test --workspace --no-fail-fast` with PostgreSQL 16:
  510 passed, 1 ignored, 1 failed
  (`read_only_fax_directory_rejects_upload_without_queueing`; known, fails
  only as root, unrelated).
- Passed: web `npm run lint`, `npm run check`, `npm test` (71), `npm run build`.
- Passed: `tests/browser/phone-network.mjs` against `vite preview`
  (390 px and 1280 px), screenshots checked.
- Passed: `node tests/docs/wiki.mjs`, `node scripts/docs/build-wiki.mjs --check`.
- Not run: real UniFi console, Docker images, SIPp/E2E.
- STE review: manual review against `docs/documentation-style.md`; the full
  ASD-STE100 standard was not available.

## Dependencies and open work

- Depends on PR #73 (`claude/ubiquiti-api-naming`).
- Migration ID: `20261111000001`.
- Local note: the session disk filled with a 29 GB debug target. Build with
  `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0` in this environment.

## Next action

None.

## References

- `docs/ai-memory/20261024-claude-ubiquiti-api-naming.md`
- `docs/ai-memory/20261022-claude-wifi-voice-survey.md`
