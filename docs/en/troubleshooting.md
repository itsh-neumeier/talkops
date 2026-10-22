# Troubleshooting

[Handbook](index.md) · [Deutsch](../de/fehlerbehebung.md)

Start with the symptom and the time it occurred. Keep the current configuration
and logs until the cause is known. Do not delete database or media volumes to
repair a registration or storage error.

## Before you start

Own users can inspect their calls and devices. Other users' information requires
the corresponding role. **Settings → System** needs `diagnostics.manage`.
Host commands need Docker access. Run commands in the deployment directory with
all active Compose overrides, environment files and the existing project name.

## First service checks

```sh
docker compose ps
docker compose logs --since 15m talkops freeswitch media-worker
```

Check **Settings → System → Status** for component failures. Use **Sessions**
for device registrations and active calls. Logs can contain phone numbers and
other private data. Redact them before sharing. The default web/API port is 8080;
use the actual configured port and host.

## Symptom map

| Symptom | Check first | Detailed procedure |
| --- | --- | --- |
| UI unavailable | Container health, HTTP port, proxy and HTTPS certificate. | [Installation](installation.md), [monitoring](monitoring.md). |
| User cannot sign in | Local password versus SIP password, TOTP, OIDC/LDAP group mapping. | [Sign-in](sign-in.md), [roles](users-and-roles.md). |
| Phone offline | Registrar, SIP username/password, device address and login ban. | [Extensions](extensions-and-devices.md), [provisioning](provisioning.md). |
| Trunk not registered | Provider credentials, auth name, registrar and transport. | [Trunks](trunks-and-numbers.md), [LEONET](leonet.md). |
| Phone registered, no ring | Number destination, DND, forwarding, schedule and ring time. | [Call routing](call-routing.md). |
| Incoming call rejected | Stored block reason, anonymous rule, local list or PhoneBlock. | [Call log](call-log.md), [call routing](call-routing.md). |
| No or one-way audio | RTP reachability, public IP/NAT and negotiated codec. | [Codec profiles](codec-profiles.md), [softphone](softphone.md). |
| Browser cannot call | HTTPS, microphone permission, chosen call identity and WebSocket proxy. | [Softphone](softphone.md). |
| Provisioning download fails | Device-visible URL, credentials, model files and supported transport. | [Provisioning](provisioning.md), [manufacturer settings](manufacturer-provisioning.md). |
| Prompt or transcript waits | Worker health, queued job, model availability and resource use. | [Prompts](prompts.md), [transcriptions](transcriptions.md). |
| Fax preparation fails | Administrator storage test, shared mounts, free space and UID 10001 permissions. | [Fax](fax.md). |
| Mail delivery fails | SMTP host, TLS, credentials, recipient and delivery log. | [Mail](mail.md), [notifications](notifications.md). |
| Backup copy fails | Encryption/password, target test, mount roots and job result. | [Backup](backup.md). |
| Door video or opener fails | Device-specific credentials, codec policy and opener configuration. | [Door stations](door-station.md), [Dahua](door-station-dahua.md). |

## Isolate call failures

1. Make an internal call between two devices to separate provider issues.
2. Check ringing, audio in both directions and normal hangup.
3. Make an ordinary outgoing call and a separate incoming call.
4. Compare each completed call with its call-log entry and timestamps.

Use a normal test number, not an emergency number. Registration success verifies
SIP credentials, not the RTP media path. Check configured RTP ports on the host
and network. Browser calls also need the WebRTC/WSS path and sometimes TURN.
Do not expose PostgreSQL or the FreeSWITCH event socket to fix media problems.

If the call log entry is delayed, FreeSWITCH can spool a call record for retry.
The [installation diagnostics](installation.md#calls-are-missing-from-the-call-log)
explain spool logs and imports. Preserve spool data during diagnosis.

## Storage and recovery failures

Run the fax storage check before resubmitting a failed preparation. The check
tests server access; verify worker and FreeSWITCH mounts separately when needed.
Retain prepared attachments and failed delivery results. Do not resend an
uncertain fax until the delivery outcome has been checked.

If an update fails after migrations, use the matching backup and offline
[restore procedure](backup.md), rather than an unverified downgrade.
Test restored calls, data access and new backups before normal use.

## Prepare a support report

Record the TalkOps version, image tags, installation method, architecture,
device model/firmware, provider, exact error, timestamps and reproduction steps.
Include expected and actual results, redacted logs and relevant request/call IDs.
For media problems, note whether internal, incoming and outgoing tests differ.
Use the limited diagnostic capture under **Settings → System → Diagnostics**
when needed. Inspect captured files before sharing them.

Submit the report through [GitHub issues](https://github.com/itsh-neumeier/talkops/issues).
Do not include `.env`, passwords, tokens, provisioning QR codes, private audio
or complete backup archives.
