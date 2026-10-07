# Monitoring

TalkOps exposes its state in the Prometheus text format at `/metrics`.

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
