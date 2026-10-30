# Cisco TFTP export before resync

## Task state

- Task ID: 20261030-claude-cisco-tftp-refresh
- Date: 2026-10-30
- Tool: Claude Code
- Status: In progress
- Branch: claude/cisco-tftp-refresh
- Base commit: 3bbdb31 (main, v1.8.19)
- Related commits or PR: PR107, release 1.8.20

## Goal

A change to a Cisco Enterprise phone must reach the phone on the first resync.

## Result

On 1.8.19 the user changed the line label, saved and started a resync. The
phone got the old label; the second resync delivered the new one. Cause: the
TFTP exporter ran only on a 30-second timer, and the resync NOTIFY went out at
once.

- `AppState::cisco_tftp` holds an `Exporter` (root and a mutex). The timer,
  `refresh_now` and `schedule` share the mutex, because each refresh swaps the
  `next` and `active` directories.
- `resync_phone` awaits `refresh_now` before the Cisco service-control NOTIFY.
- Phone create, update, delete, account, settings and ringtone changes call
  `schedule` (background refresh).

## Changes

- `crates/talkops-api/src/cisco_tftp.rs`: `Exporter`, `refresh_now`, `schedule`;
  `spawn` takes the state only.
- `crates/talkops-api/src/lib.rs`, `src/main.rs`: state field and wiring.
- `crates/talkops-api/src/routes/phones.rs`: refresh before resync, schedule
  after changes.
- `crates/talkops-api/tests/phones.rs`: resync after an extension rename
  exports the new name.
- `CHANGELOG.md`: Unreleased entry.

## Decisions

- Extension and contact changes do not call `schedule`. The resync refresh and
  the timer cover them.

## Checks

- Passed in a rust:1-trixie container with Postgres 17: `cargo fmt --all
  --check`, `cargo clippy -p talkops-api --all-targets -- -D warnings`,
  `cargo test -p talkops-api --test phones --test multi_vendor`.
- Pending: CI.
- STE review: manual review of the new text.

## Dependencies and open work

- Phone book: the user's phone has no provisioning address, so the SEP file has
  no `directoryURL`. TalkOps listens on port 8095 on the host. The user must set
  `http://192.168.140.30:8095/provisioning` on the phone page.
- German language needs Cisco locale files. TalkOps does not ship them. A
  feature to upload a locale package is possible as separate work.

## Next action

Merge the PR and include it in the next release.

## References

- `docs/ai-memory/20261030-claude-cisco-ccm-layout.md`
