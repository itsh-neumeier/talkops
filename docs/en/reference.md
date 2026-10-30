# Reference and glossary

[Handbook](index.md) · [Deutsch](../de/referenz.md)

Use this page to find configuration references. An administrator should change
deployment variables; a normal user does not need Docker or database access.

## Network defaults

| Port | Protocol | Use |
| --- | --- | --- |
| 5060 | UDP/TCP | Phone SIP registrations. |
| 5080 | UDP/TCP | Provider SIP profile. |
| 16384–16999 | UDP | FreeSWITCH RTP audio/video media. |
| 8080 | TCP | Web interface and HTTP API. |
| 8021 | TCP, localhost | FreeSWITCH Event Socket. |
| 5432 | TCP, localhost | PostgreSQL. |
| 80, 443 | TCP, optional | Caddy HTTPS and certificate setup. |
| 5066 | TCP, localhost | FreeSWITCH WebSocket behind the authenticated TalkOps proxy. |
| 3478 | UDP/TCP, optional | coturn listener; see the softphone guide for relay setup. |

These defaults are not a firewall-opening checklist. Host networking, NAT and
proxy settings depend on the deployment. Keep the database and Event Socket
local. See [installation](installation.md) and [softphone network settings](softphone.md).

## Environment and deployment files

[.env.example](../../.env.example) and [docker-compose.yml](../../docker-compose.yml)
are the deployment references. Backup and remote mount options are in
[docker-compose.backup.yml](../../docker-compose.backup.yml) and
[docker-compose.targets.yml](../../docker-compose.targets.yml).
Use the files from the same release as the images.

| Variable group | Important values |
| --- | --- |
| Image version | `TALKOPS_VERSION`; image tag such as `1.8.22`, not Git tag `v1.8.22`. |
| Database | `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB`, `POSTGRES_PORT`. |
| Required secrets | `TALKOPS_SECRET_KEY` (64 hexadecimal characters), `TALKOPS_ESL_PASSWORD`, `TALKOPS_XMLCURL_PASSWORD`. |
| Web and SIP ports | `TALKOPS_LISTEN`, `TALKOPS_HTTP_PORT`, `TALKOPS_ESL_PORT`, `TALKOPS_SIP_PORT`, `TALKOPS_SIP_TRUNK_PORT`, `TALKOPS_RTP_START_PORT`, `TALKOPS_RTP_END_PORT`. |
| HTTPS | `COMPOSE_PROFILES=caddy`, `TALKOPS_DOMAIN`. Combine active profiles when using TURN too. |
| Browser relay | `TALKOPS_TURN_SECRET`, `TALKOPS_TURN_URLS`, `TALKOPS_TURN_PEER_IP`, `TALKOPS_TURN_RELAY_ONLY`; optional `turn` profile. |
| Transcription | `TALKOPS_WHISPER_MODEL`, `TALKOPS_WHISPER_VAD`; per-purpose settings are in the UI. |
| Operations | `TZ`, `TALKOPS_LOG_FORMAT`, `RUST_LOG`, `TALKOPS_FS_LOG_LEVEL`, `TALKOPS_METRICS_TOKEN`. |
| Recovery | `TALKOPS_SETUP_TOKEN`, `TALKOPS_DEPLOYMENT_FILES`, deployment/password source paths and backup target roots; see [backup](backup.md). |

Keep `TALKOPS_HTTP_PORT` equal to the port in `TALKOPS_LISTEN`. Keep credentials
out of issue reports. Never store the backup password in captured `.env` files.
The application also has direct server/worker variables; inspect
[server configuration](../../crates/talkops-api/src/config.rs) for that interface.

## Dialing, roles and API

- [First steps](first-steps.md#dialing) lists internal, external and SIP dialing.
- [Feature codes](yealink.md#4-feature-codes) lists DND, forwarding, pickup and parking.
- [Users and roles](users-and-roles.md#permissions) lists permission keys.
- The running server serves OpenAPI at `/api/v1/openapi.json` and status at
  `/api/v1/status`. Use your configured host and port. API requests still require
  authentication and the applicable permissions; an API description grants no access.

## Glossary

| Term | Meaning |
| --- | --- |
| Extension | Internal number, owner and call settings. |
| Device | SIP identity assigned to an extension. |
| Phone | Provisioned hardware or app profile that can carry several accounts. |
| Trunk / account | Provider connection / registration credentials within it. |
| DID / E.164 | Incoming provider number / international number format such as `+49891234567`. |
| SIP / RTP | Call signaling / media transport. |
| Registrar | Server that accepts SIP registrations. |
| BLF / MWI | Busy lamp field / message waiting indicator. |
| IVR / Smart Attendant | Menu or call flow that processes caller choices. |
| DND / CLIR | Do not disturb / hide caller ID. |
| WebRTC / WSS / TURN | Browser calling / secure WebSocket signaling / media relay. |
| TTS / transcription | Text-to-speech / speech-to-text. |
| CDR / spool | Completed-call record / durable queue for records not yet stored. |
| Migration | Versioned database schema change. |

For a failed procedure, use [troubleshooting](troubleshooting.md) and its support
report checklist. Expected configuration is not proof of a live provider or
hardware test.
