# Monitoring

TalkOps exposes its state in the Prometheus text format at `/metrics`.

## Dashboard in the web UI

The start page shows operators and admins key figures (calls, missed calls,
answer rate, average talk time), a chart of calls by direction for the last
hour, day, week or month (days in the time zone from the settings), calls in
progress, recent calls and the system status (services, registered devices,
trunks, last backup). It refreshes every 10 seconds.

## Enable

Set a long random token in `.env` (or the Portainer stack environment) and
restart the stack:

```sh
TALKOPS_METRICS_TOKEN=$(openssl rand -hex 32)
```

Without a token the endpoint answers `404`; a missing or wrong token gets
`401`.

## Prometheus

```yaml
scrape_configs:
  - job_name: talkops
    scheme: https          # http if you do not use the Caddy profile
    authorization:
      credentials: <TALKOPS_METRICS_TOKEN>
    static_configs:
      - targets: ["pbx.example.com"]
```

## Metrics

| Metric | Meaning |
| --- | --- |
| `talkops_build_info{version}` | Running version (always 1) |
| `talkops_database_up` | 1 if the database answered |
| `talkops_freeswitch_connected` | 1 if TalkOps is connected to FreeSWITCH's event socket |
| `talkops_registrations` | Registered SIP devices |
| `talkops_trunk_registered{gateway}` | 1 if the trunk account is registered at the provider |
| `talkops_trunk_up{gateway}` | 1 if the trunk answers OPTIONS pings |
| `talkops_active_calls` | Calls in progress (only while FreeSWITCH is connected) |
| `talkops_calls_total{direction}` | Finished calls (`inbound`, `outbound`, `internal`) |
| `talkops_calls_answered_total{direction}` | Answered calls |
| `talkops_recordings`, `talkops_recordings_bytes` | Stored call recordings and their size |
| `talkops_voicemail_new` | Unheard voicemail messages |
| `talkops_jobs{kind,status}` | Background jobs (TTS, transcription, mail) that are queued, running or failed |
| `talkops_door_station_online{name}` | 1 if a door station's event stream is connected |

Trunk metrics carry the FreeSWITCH gateway name of each trunk account
(`gw-<account id without dashes>`).

## Example alerts

```yaml
groups:
  - name: talkops
    rules:
      - alert: TalkOpsFreeSwitchDown
        expr: talkops_freeswitch_connected == 0
        for: 2m
      - alert: TalkOpsTrunkUnregistered
        expr: talkops_trunk_registered == 0
        for: 10m
      - alert: TalkOpsDoorStationOffline
        expr: talkops_door_station_online == 0
        for: 10m
      - alert: TalkOpsJobsFailing
        expr: delta(talkops_jobs{status="failed"}[1h]) > 0
```

## Troubleshooting in the web UI

**Settings → System** (admin):

- **Status**: version, uptime of TalkOps and FreeSWITCH, active channels.
- **Debug logging**: pick a level (debug/info/notice/warning), a duration in
  minutes (1–60) and optionally **SIP trace**, *Start*, reproduce the
  problem. The log follows live (filterable), can be copied to the clipboard
  with *Copy* (the lines shown, after filtering) or downloaded as a text file
  to attach to a support request. The capture stops by itself after the chosen
  time and switches the SIP trace off again. This is
  `fs_cli -x "sofia global siptrace on"` with `/log debug`, without access to
  the server.
- **Active channels**: like `show channels` – channel, caller, destination,
  state, codec, application.
- **Sessions with the provider**: *End all calls* (a BYE for every call),
  *Sign trunks off and on* or both. Helps when the provider rejects calls with
  "403 Too many simultaneous sessions". Trunk calls use session timers
  (600 s) so the provider ends orphaned calls itself.
- **Test calls**: echo, key test, time announcement and "ring only" by
  [sip5060.net](https://sip5060.net/test-calls/) – check audio and NAT to the
  internet without a trunk provider. The button opens the softphone with the
  address; needs *Allow dialing SIP addresses*.
- **Restart**: *Restart FreeSWITCH* or *Restart all services* (FreeSWITCH,
  media worker, TalkOps). The services exit and Docker starts them again
  thanks to `restart: unless-stopped`; active calls are dropped.
