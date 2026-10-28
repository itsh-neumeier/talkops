# Fix Cisco TFTP sidecar privileges for PR 89

## Task state

- Task ID: 20261028-claude-cisco-tftp-privileges
- Date: 2026-10-28
- Tool: Claude Code
- Status: Completed
- Branch: codex/cisco-7800-profiles
- Base commit: 00fe4e5
- Related commits or PR: https://github.com/itsh-neumeier/talkops/pull/89

## Goal

Complete draft PR 89 (Cisco 7800 profiles) for release 1.8.15. Codex reached
its usage limit. The user asked Claude Code to finish the PR and the release.

## Result

The CI job "Rust (fmt, clippy, test)" failed in the new TFTP protocol test with
"TFTP listener did not become ready". A CI rerun failed the same way. The
`slapd.service` error in the same log is expected noise; CI configures LDAP later.

Root cause: tftpd-hpa forks one process for each request. That process calls
`setgroups()` before it drops to `--user`. Without `CAP_SETGID` the call fails
with EPERM and the process exits with status 71 and sends no reply. The test
demoted the daemon to a non-root user, and the optional Compose file started
the sidecar as UID 10001 with all capabilities dropped. Thus the production
sidecar could not serve any file either. This was reproduced in Ubuntu 24.04
and Debian trixie containers with strace.

The user selected the capability fix. The sidecar now starts as root with only
`SETUID`, `SETGID` and `DAC_READ_SEARCH`, `no-new-privileges` and a read-only
filesystem. tftpd-hpa serves each request as `talkops` (UID 10001).
`DAC_READ_SEARCH` lets the lease guard read the 0700 export root.

## Changes

- `docker-compose.cisco-tftp.yml`: run the sidecar as `0:0` with the three
  capabilities and a comment that gives the reason.
- `docker/talkops/cisco-tftp.py`: exit with a clear message if not started as
  root, instead of accepting requests that never get a reply.
- `tests/provisioning/tftp.py`: require root, start in.tftpd as root and let it
  drop to `nobody`, as the sidecar does. Remove the pre-exec demotion.
- `.github/workflows/ci.yml`: run the TFTP protocol test with `sudo`.
- `docs/adr/0049-cisco-enterprise-tftp-bootstrap.md`, `docs/en/manufacturer-provisioning.md`,
  `docs/de/hersteller-provisionierung.md`, `docs/research/cisco-7800.md`:
  replace "UID 10001 with no capabilities" with the actual privilege model.

## Decisions

- Keep tftpd-hpa (ADR 0049: no TFTP server in Rust). Grant the smallest
  capability set that lets it drop privileges. File capabilities are not an
  option because `no-new-privileges` ignores them.
- ADR 0049 is not released yet, so it is amended in place instead of superseded.

## Checks

- Passed: `tests/provisioning/tftp.py` as root in Ubuntu 24.04 with tftpd-hpa,
  five tests. As non-root it stops with the new explicit message.
- Passed: sidecar simulation on Debian trixie with the real guard and map file,
  read-only root, `--cap-drop ALL` plus the three capabilities and a fresh
  lease: `dialplan.xml` is served, an unknown SEP file gets "File not found".
  Without `DAC_READ_SEARCH` the guard cannot read the lease and nothing starts.
  As UID 10001 the guard now exits with the explicit message.
- Observed: the first request after daemon start gets no reply in containers.
  This also occurs as full root and with in.tftpd started directly, so it is
  independent of this change. TFTP clients retransmit requests.
- Passed: `docker compose ... -f docker-compose.cisco-tftp.yml config` renders
  the new user and capabilities; the file contains no `build:`.
- Not run locally: full Rust workspace tests and image builds; CI verifies them.
- STE review: repository writing rules applied; full standard unavailable.

## Dependencies and open work

- No phone has used the TFTP bootstrap yet. A hardware test remains open.
- Docker leaves a small test volume `tftp-test` on the developer host.

## Next action

Push to PR 89, verify CI, mark the PR ready and integrate it into 1.8.15.

## References

- docs/ai-memory/20261028-codex-cisco-7800-profiles.md
- docs/ai-memory/20261028-codex-cisco-7800-enterprise.md
- https://manpages.debian.org/trixie/tftpd-hpa/in.tftpd.8.en.html
