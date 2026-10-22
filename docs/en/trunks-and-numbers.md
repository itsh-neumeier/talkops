# Trunks and numbers

[Handbook](index.md) · [Deutsch](../de/trunks-und-rufnummern.md)

A trunk connects TalkOps to a SIP provider. An account contains registration
credentials. A phone number belongs to an account and selects an incoming
destination. These objects are separate from users and internal extensions.

## Before you start

Use an administrator or a role with `trunks.view` and `trunks.manage`.
Telephony defaults also require `settings.manage`. You need the provider's SIP
server, username, authentication name if different, password and assigned numbers.
Use SIP credentials, which can differ from the provider portal login.
See [LEONET](leonet.md) for that provider's procedure.

## Add the provider and account

1. Open **Settings → Trunks → New trunk**.
2. Select the provider preset and read its notes and verification state.
3. For an unknown provider, select **Generic SIP provider** and enter its registrar.
4. Save the trunk and open its detail page.
5. Select **Add account** and enter the provider credentials.
6. Save and check the registration status.

Expected result: an account that requires registration shows **registered**.
An IP-authenticated provider can behave differently; follow its preset notes.
For errors, check the registrar, transport and authentication name before
changing advanced settings. Do not publish credentials in support requests.

## Add numbers and incoming destinations

1. Add a number to the correct account in international format, such as `+49891234567`.
2. Open **Settings → Phone numbers** or the number on the trunk detail page.
3. Select its incoming destination and save.
4. Call the number from a separate external phone.

Choose an extension, group, time condition, attendant, queue, voicemail or
configured fax destination as offered by the destination selector.
For an extension destination, **Ring more extensions** adds simultaneous targets.
The main extension supplies unanswered-call handling. See [call routing](call-routing.md).
Changing a number to a fax destination replaces its voice route.

Expected result: the selected destination receives the call. The completed call
appears in the permitted [call log](call-log.md). A successful registration alone
does not verify incoming number matching or two-way audio.

## Outgoing calls and emergency routing

1. Open **Settings → Telephony** and set country and area codes.
2. Select a default number and save.
3. If needed, assign another outgoing number on an extension.
4. Make a normal external call and check caller ID and audio in both directions.

Emergency calls use the default number and do not hide caller ID. Confirm the
provider's emergency service support and address assignment with the provider.
Do not call an emergency number as an ordinary installation test.
Dialing formats and hidden caller ID are listed in [first steps](first-steps.md).

## SIP accounts without numbers

An account can have a SIP address without a phone number. Set **Inbound calls
to this account** when calls should reach a destination without number matching.
For `name@domain` dialing, see [first steps](first-steps.md).

## Troubleshooting

- Not registered: check credentials, registrar, transport and network reachability.
- Registered but no incoming ring: check the account, number and destination.
- Rejected incoming call: check the stored block reason and routing rules.
- No external call: check the default/outgoing number and provider permissions.
- One-way audio: use the network checks in [troubleshooting](troubleshooting.md).

The [preset catalog guide](../trunk-presets.md) explains provider sources and
how to contribute a tested preset.
