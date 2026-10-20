# Overview

[Handbook](index.md) · [Deutsch](../de/ueberblick.md)

TalkOps is a self-hosted phone system for homelabs and small businesses.
It targets installations with about 50 extensions. Capacity depends on calls,
recording, transcription and the host. This is not a measured capacity limit.

## Components

| Component | Responsibility |
| --- | --- |
| FreeSWITCH | SIP registrations, calls, audio, video and fax media. |
| TalkOps server | Web interface, API, routing, provisioning and access checks. |
| PostgreSQL | Settings, users, call records and background job queue. |
| Media worker | Local speech synthesis and transcription jobs. |
| Optional Caddy and coturn | HTTPS and browser media relay. |

The [architecture](../architecture.md) and [ADRs](../adr/README.md) describe
implementation decisions. The handbook describes operation. Old phase labels in
design records describe development history, not the current feature status.

## Start as an administrator

You need access to a Linux Docker host and the initial administrator account.

1. Read [installation](installation.md), or [Portainer](portainer.md).
2. Complete [first steps](first-steps.md) and confirm an internal call.
3. Configure [trunks and numbers](trunks-and-numbers.md) and confirm both call directions.
4. Connect [phones](provisioning.md) or the [browser softphone](softphone.md).
5. Set [roles](users-and-roles.md), [backups](backup.md) and [monitoring](monitoring.md).

Expected result: users can sign in, call through their assigned identity and
reach their messages. A checked recovery backup exists outside the host.

## Start as a user

Ask an administrator to create your user and assign an extension.
Use **Settings → My phones** for devices, DND and forwarding.
Use the [softphone](softphone.md), [phone book](phonebook.md),
[call log](call-log.md), [voicemail](voicemail.md) and [fax](fax.md) as permitted.
Features outside your role can be hidden. An empty personal page can mean that
no extension or destination is assigned to you.

## Support and verification limits

Provider presets and converted provisioning templates are configuration sources.
They do not prove that every provider, device model or firmware has been tested.
Read the displayed validation state and the model or provider guide.
Spectralink and Grandstream Wave profiles remain blocked where their source
templates are empty. Firmware files are not included with TalkOps.

Automated tests cover application and simulated SIP/browser behavior. Verify
your real phones, provider, fax delivery and recovery on your deployment.
Start with [troubleshooting](troubleshooting.md) when a check fails.

The [handbook index](index.md) lists every topic. The [glossary](reference.md)
defines the terms used in the procedures.
