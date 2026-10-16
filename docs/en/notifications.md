# E-mail notifications

TalkOps sends an e-mail when something important happens on the phone
system: a trunk loses its registration, FreeSWITCH is not reachable, a backup
fails, the disk is almost full or somebody changes the configuration.

You set up the notifications in **Settings → Notifications**. You need the
permission "Notifications" (administrators have it). The mail server must be
set up in **Settings → E-mail**.

## Default settings

Without a change, the users of the built-in administrator role with an
e-mail address get the mails. Most events are on. These events are off by
default because they can occur often: device registrations, configuration
changes, successful backups and secrets that were viewed.

## Events

| Area | Events |
| --- | --- |
| Trunks | Registration lost, registration rejected (wrong credentials), trunk configuration changed |
| FreeSWITCH | Connection lost, FreeSWITCH restarted |
| Devices | Device no longer registered |
| Security | Address banned by the SIP guard, repeated failed sign-ins, administrator signed in from a new address, two-factor sign-in changed, password changed, API token created or revoked, secret viewed |
| Backup and storage | Backup failed or written, backup target failed, no backup for two days, low disk space (less than 10 % or 2 GB) |
| Services | Media jobs failing, media worker inactive, voicemail mail failed |
| Configuration changes | Users and roles, numbers and routing, extensions and devices, system settings, backup restored, restart triggered |

Events marked "Planned" are in the list, but TalkOps does not detect them
yet.

## Rules

Click **Edit** next to an event:

- **Minimum severity**: only events with this severity or higher send a
  mail.
- **Recipients**: roles, single users and more e-mail addresses. Users
  without an e-mail address get no mail.
- **Wait before alert** (problems only): TalkOps waits this time. If the
  problem ends in this time, no mail is sent. Example: a trunk that is
  offline for one minute during a provider restart does not send a mail
  (default 2 minutes).
- **Send a mail when the problem ends** (problems only): you get a second
  mail when the trunk is registered again, the connection is back or there
  is enough disk space again.
- **Collect into a digest**: TalkOps collects the events of this time and
  sends one mail with a list. Configuration changes use 5 minutes by
  default.
- **Mails per hour**: more mails for this event are not sent; the log
  shows "Rate limit reached".

**Send test** sends a sample mail to the recipients of the saved rule.
**Reset to default** restores the default values.

## Log

The card **Notification log** shows the last 100 events and the last 100
mails with their status. Entries older than 30 days are deleted.

## Mail text

The mail type "Notification" is in **Settings → E-mail → Mail texts**. You
can change subject and text like the voicemail mail. Placeholders:
`event.title`, `event.category`, `event.severity`, `event.status`,
`event.time`, `event.details`, `event.count`, `event.recovery` (set for a
recovery mail) and `event.digest` (list of a digest). The mail uses the
default language from the telephony settings.
