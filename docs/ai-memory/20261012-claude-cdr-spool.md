# Keep undelivered call records and log active call failures

## Task state

- Task ID: 20261012-claude-cdr-spool
- Date: 2026-10-12
- Tool: Claude Code
- Status: Completed
- Branch: `claude/modest-dirac-fhox1w`
- Base commit: `4503fb3` (`ccr-df1c8083-0ce1he`, TalkOps 1.7.6)
- Related commits or PR: Pending. Related: PR 7 (`codex/active-call-overview`)

## Goal

The user reported three problems:

- A 40-minute outbound call stopped showing in the active call list.
- That call is not in the call log and has no recording.
- Other calls made on 2026-10-12 are not in the call log either.

The user also reported that an outbound call showed the wrong target
(`101 Familie-Zweig → 4995431241 Outbound Call`). Codex worked on the same
active call overview at the same time.

## Result

Analysis:

- Wrong target in the active call list: `parse_calls` in `telephony.rs`
  used the caller ID of the bridged leg (`b_cid_num`). For trunk calls,
  this is the TalkOps caller ID number, not the dialed number. "Outbound
  Call" is the FreeSWITCH default callee name. PR 7 fixes this with
  `talkops_destination` from `uuid_dump`. This task does not change it.
- Possible causes of lost call records:
  - mod_xml_cdr tried 3 times at 5-second intervals. Then it wrote the
    record to `$${log_dir}/xml_cdr` in the FreeSWITCH container. That
    path had no volume, so a new container (update) lost the records.
    Nothing imported them.
  - `/fs/cdr` answered HTTP 200 for a malformed record and dropped it.
  - A recording is registered only through its call record. A lost record
    also hides the recording. The WAV file stays in the `recordings` volume.
- An update or a FreeSWITCH restart ends all active calls. TalkOps 1.7.6
  was published on 2026-10-12. No 1.7.6 change touches the CDR path.
  The cause on the user's host is not confirmed. The user must send the
  logs (see next action).
- When the event socket was not connected, the active call list was empty
  and no log entry showed the problem.

Implementation:

- New volume `cdr-spool` (`/var/lib/talkops/cdr-spool`), shared by the
  `freeswitch` and `talkops` services. mod_xml_cdr uses it as `err-log-dir`.
- mod_xml_cdr now tries 5 times at 10-second intervals. This covers
  a TalkOps restart.
- TalkOps imports the spool every minute (`cdr_spool::spawn`). Files
  younger than 10 seconds stay for the next scan. Stored, duplicate and
  unrouted records are removed. Malformed records move to `rejected/`.
  A database error stops the scan, and the files stay.
- `/fs/cdr` answers 400 for a malformed record. FreeSWITCH then spools
  the record, and it is not lost.
- Logs: `call record stored` (info) for each stored record, `show calls
  failed` (warn), and `lost connection to FreeSWITCH event socket` (warn).

## Changes

- `crates/talkops-api/src/cdr_spool.rs`: shared `ingest`, recording
  registration (moved from `routes/fs.rs`), spool import and scan task.
- `crates/talkops-api/src/routes/fs.rs`: `/fs/cdr` uses `cdr_spool::ingest`.
  It answers 400 for malformed records.
- `crates/talkops-api/src/config.rs`, `main.rs`, `lib.rs`:
  `TALKOPS_CDR_SPOOL_DIR` (default `/var/lib/talkops/cdr-spool`), start
  of the scan task.
- `crates/talkops-api/src/esl.rs`, `telephony.rs`: warnings for lost event
  socket connection and failed `show calls`.
- `crates/talkops-api/tests/freeswitch.rs`: test `cdr_spool_import`.
- `docker-compose.yml`, `docker/freeswitch/Dockerfile`,
  `docker/talkops/Dockerfile`, `docker/freeswitch/docker-entrypoint.sh`,
  `docker/freeswitch/conf/autoload_configs/xml_cdr.conf.xml`: volume,
  directory, `talkops_cdr_spool_dir` variable, retry values.
- `docs/en/installation.md`, `docs/de/installation.md`: update note and
  troubleshooting for missing calls, including recovery from older containers.
- `docs/architecture.md`: volume table row.

## Decisions

- The user approved the proposal. The user told me to wait for the Codex
  PR and then to do the work.
- PR 7 rewrites `routes/dashboard.rs` and the active call component.
  To prevent conflicts, this task does not change them. The proposed
  "FreeSWITCH not available" state (HTTP 503 and a stale-list marker in the
  UI) goes to PR 7 as review feedback.
- The spool import is in TalkOps, not a separate tool. Duplicate records
  are safe because `cdr.call_uuid` is unique (`ON CONFLICT DO NOTHING`).
- The spool importer accepts the plain XML document and the URL-encoded
  `cdr=` form, because both formats are possible.

## Checks

- Passed: `cargo fmt --all --check`.
- Passed: `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `DATABASE_URL=… cargo test --workspace` (local PostgreSQL 16),
  including the new `cdr_spool_import` test and `cdr_ingestion`.
- Passed: `sh -n docker/freeswitch/docker-entrypoint.sh`, `xmllint` on
  `xml_cdr.conf.xml`, YAML parse of `docker-compose.yml`.
- Not run: Docker image build, full stack and SIPp E2E. The FreeSWITCH
  image builds from source. CI runs these checks.
- Not run: web checks. This task does not change `web/`.
- STE review: full standard unavailable; review pending.

## Dependencies and open work

- No migration.
- Upgrade: the new volume is created by `docker compose up -d` or a
  Portainer stack update. Records spooled by older versions are inside the
  old FreeSWITCH container. The user must copy them out before the update
  (see `docs/en/installation.md`).
- Feedback for PR 7:
  - `active_calls::load` returns an empty list when the event socket is
    not connected. It should report the failure (HTTP 503, UI state).
  - `load` drops a call when `uuid_dump` fails or the call has no
    `talkops_tenant_id`. It should show the unresolved row and not hide it.
- Not confirmed: the cause of the lost calls on the user's host.

## Next action

The user runs the diagnostic commands from the PR on the host before the
update. If files exist in `/usr/local/freeswitch/var/log/freeswitch/xml_cdr`,
copy them out and import them after the update.

## References

- PR 7: <https://github.com/itsh-neumeier/talkops/pull/7>
- `docs/adr/0006-steuerung-xml-curl-und-esl.md`
