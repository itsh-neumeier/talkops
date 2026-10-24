# System settings and storage

[Handbook](index.md) · [Deutsch](../de/systemeinstellungen.md)

Open **Settings**. The navigation shows the sections allowed by your role.
Administrators have all sections. A hidden section does not mean that a service
is missing. See [users and roles](users-and-roles.md) before granting access.

## Settings map

| Section | Permission | Purpose and detailed guide |
| --- | --- | --- |
| Telephony | `settings.manage` | Dialing rules, emergency/default number, public IP, SIP addresses and phone time zone; [trunks and numbers](trunks-and-numbers.md). |
| Call handling | `settings.manage` | Recording by call direction, retention, transcription, hold music and the tile server of the call map; [recording](recording.md), [prompts](prompts.md), [call log](call-log.md#map). |
| Transcription connections | `transcription.settings` in Call handling | Engines, purposes, cloud connections and API keys; [recording](recording.md). |
| Voicemail | `voicemail.settings` | Default language, voices, speed and system texts; [voicemail](voicemail.md). |
| E-mail | `voicemail.settings` | SMTP, design and mail text; [mail](mail.md). |
| Fax | `fax.manage` | Enablement, sender numbers, transport, destinations and mail input; [fax](fax.md). |
| Notifications | `notifications.manage` | Event rules and delivery log; [notifications](notifications.md). |
| Sign-in & security | `identity.manage` or `security.manage` | OIDC/LDAP controls and SIP login protection; [sign-in](sign-in.md), [installation](installation.md). Each control checks its own permission. |
| System | `diagnostics.manage` | Status, diagnostics, sessions and maintenance; [monitoring](monitoring.md). |
| Backup | `backups.manage` | Capture, encryption, jobs, archives and recovery; [backup](backup.md). |
| Storage | `storage.manage` | Reusable storage connections and test tools; procedure below. |
| My account | Own signed-in account | Password and personal account controls; [sign-in](sign-in.md). |

Setup pages also live under Settings: extensions, users, call routing, trunks,
phone numbers, phones, door stations and audit log. **My phones** shows your
own assigned extensions. Their handbook chapters are in the [index](index.md).

## Search the settings

The field **Search settings** is above the settings navigation. Type a word,
for example `hold music`, `LDAP` or `area code`. The search finds sections,
cards inside them and setup pages. It also finds field labels and common
synonyms in English and German. Umlauts and their spelling with `e` compare
equal, for example `Ubersicht` finds `Übersicht`.

- Select a result, or use the arrow keys and **Enter**. TalkOps opens the
  page and marks the card for a short time.
- Press **/** or **Ctrl+K** to go to the search field. **Esc** clears it.
- The search shows only the settings that your role allows.

## Change a setting

1. Open the appropriate section and record the current value.
2. Read the field hint and detailed guide before changing a shared default.
3. Change one related group of fields and select its **Save** button.
4. Reload the section to confirm persistence.
5. Check the affected workflow with a test account or test call.

Settings use separate forms. Saving recording settings does not save another
open form. A saved default can affect many extensions. Phone time-zone changes
can require a provisioning download; browser time display is a separate concern.
Recording and cloud transcription settings can change where call data is kept.
See the media guides for per-purpose controls and access.

## Call map tile server

The call map (ADR 0044) works offline by default. It shows country outlines
that come with TalkOps and sends no request to another server. An
administrator can set a tile server for a detailed background map:

1. Open **Settings → Call handling → Call map**.
2. Enter the **Tile server URL**, for example
   `https://tile.openstreetmap.org/{z}/{x}/{y}.png`. The URL must start
   with `https://` and contain `{z}`, `{x}` and `{y}`. `{s}` as the first
   part of the host selects the subdomains `a`, `b` and `c`. `{r}` adds the
   suffix for high-resolution screens.
3. Enter the **Attribution** that the tile provider requires, for example
   `© OpenStreetMap contributors`.
4. Select **Save**.

TalkOps then adds the tile host to the Content Security Policy of the web
UI. The change applies after a reload of the page. The audit log records
each change. To return to the offline map, clear the URL and save.

External tiles send the IP address of each browser and the shown map
section to the tile server. Obey the usage policy of the tile server. The
OpenStreetMap tile servers permit only light use; use an own tile server or
a commercial provider for regular use.

`GET /api/v1/settings/call-map` returns the setting (all signed-in users).
`PUT /api/v1/settings/call-map` changes it (`settings.manage`).

## Create a storage connection

Storage targets describe a connection. Backup jobs and audit export refer to
these targets. Creating a target does not itself create a backup schedule or
move running call media to remote storage.

1. Open **Settings → Storage** and add a target.
2. Choose the supported connection type and enter server, path and credentials.
3. Use the browse and connection test controls to verify the intended directory.
4. Save the target and assign it to a backup job or audit export.
5. Run that job and verify the resulting archive or export.

The supported target types and local mount rules are documented under
[backup targets](backup.md#backup-targets). Keep saved secrets unchanged when
you only change a non-secret field. Check the displayed usages before removing
a connection. A connection test alone does not verify archive recovery.

## Diagnose and maintain

Under **Settings → System**, use **Status** for component health,
**Diagnostics** for a limited capture, **Sessions** for registered devices and
calls, and **Maintenance** for the offered service actions.
Plan actions that restart services when no calls are active.
See [monitoring](monitoring.md), [updates](updates.md) and
[troubleshooting](troubleshooting.md). Use the [audit log](audit-log.md) to
inspect recorded changes. Remove secrets and private call data from support evidence.
