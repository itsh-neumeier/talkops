# Users and roles

Each user has one **role**. The role gives the permissions. Open
**Settings → Users → Roles** to see and change the roles.

## Built-in roles

| Role | Permissions after the update |
| --- | --- |
| Administrator | All permissions. Nobody can change this role. |
| Operator | Active calls and statistics, all calls in the call log, live transcript of own calls, phone book, call blocking, time condition switch, read access to extensions, users, call routing, trunks, numbers and phones. |
| Default user | No extra permissions. |

These defaults are the same as the fixed roles of earlier versions. An
update does not change any access.

Every user can use their own data without a permission: own extensions,
own calls in the call log, own recordings and transcripts, own voicemail,
own phones and the softphone with own extensions.

## Change permissions

1. Open **Settings → Users** and select the **Roles** tab.
2. Set or clear the check boxes. One column is one role, one row is one
   permission. The box in the area row sets all permissions of the area.
3. A yellow bar shows the number of changed roles. Click **Save**.

Changes apply at once, also in open sessions. The administrator column
is locked. Permissions marked **Critical** give access to secrets, the
system or more rights (for example `Manage users`, `Backups`).

## Own roles

1. Click **New role**.
2. Enter a name and, if necessary, a description.
3. Optional: select a role in **Copy permissions from**.
4. Optional: enter a **directory group** (see below).
5. Click **Save**, then set the permissions in the table.

Select the role in the user dialog (**Users** tab → **Edit** → **Role**).
You can delete an own role when no user has it.

## Rules for user managers

A user with the permission **Manage users** but without the administrator
role:

- cannot give the administrator role,
- can only give roles whose permissions they also have,
- cannot change, delete or set a password for users with more permissions.

Only administrators add, change and delete roles.

## Directory users (LDAP, OIDC)

The group fields of [sign-in](sign-in.md) select the administrator,
operator and user roles. An own role can also have a **directory group**.
At each login and at the hourly LDAP sync, a member of this group gets the
own role. Administrators stay administrators. The user group still controls
who can log in.

## Permissions

| Area | Permission | Key |
| --- | --- | --- |
| Calls | See active calls and statistics | `calls.active` |
| Calls | See all calls in the call log | `calls.view_all` |
| Calls | Live transcript of own calls | `calls.live_transcript` |
| Calls | Live transcript of all calls | `calls.live_transcript_all` |
| Recordings | All recordings and transcripts (audited) | `recordings.access_all` |
| Recordings | Delete recordings | `recordings.delete` |
| Recordings | Transcribe again | `recordings.retranscribe` |
| Recordings | Own transcription jobs | `transcriptions.view` |
| Recordings | All transcription jobs and worker resources | `transcriptions.manage` |
| Voicemail | All voicemail boxes | `voicemail.access_all` |
| Voicemail | Voicemail and e-mail settings | `voicemail.settings` |
| Phone book | Edit the phone book | `phonebook.edit` |
| Extensions | See extensions | `extensions.view` |
| Extensions | Manage extensions | `extensions.manage` |
| Extensions | Softphone with any extension | `softphone.any_extension` |
| Users | See users | `users.view` |
| Users | Manage users (critical) | `users.manage` |
| Call routing | See call routing | `routing.view` |
| Call routing | Manage call routing | `routing.manage` |
| Call routing | Switch time conditions | `routing.override` |
| Call routing | Block numbers | `blocking.manage` |
| Call routing | Spam protection settings | `blocking.settings` |
| Trunks | See trunks and numbers | `trunks.view` |
| Trunks | Manage trunks and numbers (critical) | `trunks.manage` |
| Phones | See phones | `phones.view` |
| Phones | Manage phones | `phones.manage` |
| Doors | Manage door stations | `doors.manage` |
| System | Telephony settings | `settings.manage` |
| System | Transcription settings (critical) | `transcription.settings` |
| System | SIP protection (critical) | `security.manage` |
| System | LDAP and single sign-on (critical) | `identity.manage` |
| System | Backups (critical) | `backups.manage` |
| System | Storage targets (critical) | `storage.manage` |
| System | Diagnostics and restart (critical) | `diagnostics.manage` |
| System | Audit log | `audit.view` |
| System | Audit retention and export (critical) | `audit.manage` |

See [Audit log](audit-log.md) for the filters, the downloads, the retention
and the automatic export.

## API

- `GET /api/v1/roles`: permission catalog and roles (needs `users.view` or
  `users.manage`).
- `POST /api/v1/roles`, `PUT /api/v1/roles/{id}`, `DELETE /api/v1/roles/{id}`:
  administrators only. Every change writes an audit entry.
- `POST /api/v1/users` and `PUT /api/v1/users/{id}` accept `role_id` (any
  role, built-in or own). Without `role_id`, `role` selects a built-in role.
- `GET /api/v1/auth/me` returns `permissions` (all keys for administrators).
