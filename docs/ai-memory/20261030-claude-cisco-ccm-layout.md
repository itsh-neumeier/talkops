# Cisco Enterprise call manager layout fix

## Task state

- Task ID: 20261030-claude-cisco-ccm-layout
- Date: 2026-10-30
- Tool: Claude Code
- Status: In progress
- Branch: claude/cisco-ccm-layout
- Base commit: 2fdf037
- Related commits or PR: Pending

## Goal

Find why the CP-7861 (sip78xx.14-2-1-0001-14) does not register after the
1.8.18 update, and make a permanent fix.

## Result

Live debugging on the PBX host (SSH, read-only except temporary test files on
the cisco-tftp volume) found two causes:

1. The phone had no `cisco_enterprise_tftp_ip`. The phone was deleted and
   created again on 2026-10-29, so the exporter skipped it without a log entry.
   The user set the value in the UI. The exporter then delivered the files.
2. The SIP stack of the phone ignores the call manager when
   `<callManagerGroup><members><member priority="0"><callManager>` is on one
   line. The Java layer logs the CUCM, but the native log has no `ccm0=` line
   and shows `Admin has not configured a valid cucm for cucm index=PRIMARY_CCM=1`.

Device tests with temporary files on the TFTP volume:

- Test 2: `/` instead of `&#x2f;` in `timeZone`: parse completed, no CUCM.
- Test 3: hand-written layout (block on separate lines, empty
  `commonProfile`, no `messagesNumber`): registered.
- Test 4: original export, only the block on separate lines: registered.

## Changes

- `crates/talkops-provisioning/templates/talkops/cisco7800/enterprise.cnf.xml.jinja`:
  one element per line in the call manager block.
- `crates/talkops-provisioning/tests/cisco7800.rs`: assert that these tags do
  not touch.
- `docs/research/cisco-7800.md`, `CHANGELOG.md`: record the device result.

## Decisions

- Keep `&#x2f;` (minijinja HTML escape). Test 4 shows that the phone accepts it.
- Keep the content-hash `versionStamp`. The changed content gives a new stamp,
  so the phone parses the file again.

## Checks

- Passed: device test 4 on the user's CP-7861 (Unified CM1 192.168.140.30 Active).
- Pending: provisioning tests in a rust:1-trixie container; CI.
- STE review: manual review of the new text.

## Dependencies and open work

- Exporter: log Cisco Enterprise phones without a TFTP client IP; show a
  warning on the phone page.
- Phone page: Cisco phones always show "offline" after save or resync.
- Phone clock is about 13 minutes late (pool.ntp.org); suggest a local NTP server.
- The user must remove the `claude-talkops-debug` key from root's
  `authorized_keys` on the PBX host after debugging.

## Next action

Merge the PR, release it, and check that the phone registers with the exported
file without test patches.

## References

- `docs/ai-memory/20261029-claude-cisco-enterprise-features.md`
- `docs/ai-memory/20261029-claude-cisco-registration-agent.md`
