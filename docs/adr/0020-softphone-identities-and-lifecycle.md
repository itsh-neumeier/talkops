# 0020 - Softphone identities and connection lifetime

- Status: Accepted
- Date: 2026-10-12
- Supersedes: The owner-only credential rule in [ADR 0015](0015-webrtc-softphone.md)

## Context

Administrators need to use the browser softphone without a fixed extension
assignment. Regular users can have more than one assigned extension.
The page previously selected the first assigned extension. Navigation closed
the SIP connection. The account endpoint reused the first browser device on
the extension. Admin access to that device would share another user's secret.

## Decision

An administrator can select an enabled extension in the same tenant.
Other roles can select only their enabled assigned extensions.
An administrator must select an extension before registration starts.
Users with one extension get a fixed identity. Users with several extensions
get a selector. The server applies these rules to accounts, choices, recent
calls and conferences.

The selected extension determines incoming calls, internal identity, public
caller number and call settings. The public caller number comes from the
existing outbound route. This change does not add an arbitrary caller number
override. Emergency routing continues to use the existing dialplan rules.

Each user gets a separate browser device on each extension. The device stores
`browser_user_id`. The server locks the extension during account creation.
The unique index prevents duplicate browser accounts for the same user and
extension. Creation and credential access write an audit entry in the same
transaction. SIP directory lookup and ring targets check the current user's
enabled state and extension permission for these devices.
The device credential and password-reset endpoints also check browser ownership.
An extension owner cannot retrieve or reset another user's browser secret.

Migration `20261026000002` assigns the first existing browser device to the
current extension owner. It keeps the SIP username, encrypted password and
enabled state. It leaves other devices unchanged. User deletion removes the
user's browser devices. Admin devices do not change the extension assignment.

The root layout owns the softphone component after the user first opens it.
The component keeps its media elements when the route changes. A compact panel
shows incoming and active calls on other TalkOps pages. Logout destroys the
component and stops the SIP user. Closing or reloading the tab disconnects it.
Identity changes are blocked during calls. The old identity unregisters before
the new identity connects. SIP.js handles bounded transport reconnection.

Audio preferences and contact favorites are stored locally per user.
SIP credentials are kept only in memory. Browser notifications require an
explicit user action. Sound playback needs a browser user gesture. Output
device selection uses the browser's supported API. Other browsers use the
system output device. Microphone tests stop when a call starts or the user
leaves the full softphone view.

## Consequences

Admin registration can receive calls to another user's extension. The UI must
state this effect. No extension ownership or existing dialplan setting changes.
An old admin API client must send `extension_id` to `POST /api/v1/me/webrtc`.
A regular user's empty account request retains its previous default behavior.

The first scope keeps one active call per browser softphone. Transfer,
consultation, call waiting and multiple calls need a separate session design.
The app does not promise incoming calls after the tab closes or the browser
suspends it. Existing calls are not forcibly ended when permissions change.

## Validation

HTTP tests cover roles, tenant isolation, disabled devices, account reuse,
concurrent requests, migration compatibility, call history and caller numbers.
Browser fixtures cover identity changes, audio tests, contacts, notification
permissions, navigation and logout. The FreeSWITCH E2E test checks live audio
after navigation during an established call.
