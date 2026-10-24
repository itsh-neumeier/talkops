# 0045 - Outbound dialing permissions

- Status: Accepted by the user on 2026-10-23 (development prompt
  "Wahlberechtigungen für ausgehende Anrufe", with a proposed model that the
  task could change with reasons).
- Date: 2026-10-23
- Uses: ADR 0006 (dialplan from the database), ADR 0029 (roles and
  permissions), ADR 0044 (number geolocation). Changes no earlier ADR.

## Context

Before this decision, TalkOps did not limit outbound calls. The dialplan
put the dialed digits into one of four classes (`Dialed::Emergency`,
`Service`, `External`, `Invalid`) and sent all valid numbers to the trunk.
Each extension could call all valid numbers, also foreign numbers and
premium-rate numbers (`0900`, `0137` and others). Toll fraud through
hacked phones or misused call forwarding is a real cost risk for small
businesses.

The existing lists `call_blocks` and `call_allowlist` apply only to inbound
calls and to the full tenant. Roles and permissions (ADR 0029) control
access to the web interface and the API, not telephony.

Calls can go out through more paths than direct dialing:

| Path | Code |
| --- | --- |
| Direct dialing from a device, also the browser softphone | `plan_internal` |
| SIP address dialing (`user@host`) | `plan_internal`, `plan_sip_address` |
| Unconditional call forwarding to an external number | `ring_extension`, `forward_external` |
| Forwarding code `*72<number>` | `feature_code` |
| Blind transfer (SIP REFER) to an external number | `plan_transfer`, `transfer_external` |
| Conference participant from the softphone | `conference::add_participant` (loopback into `plan_internal`) |
| Outbound fax | `fax::dispatch` |
| Smart Attendant direct dialing | `plan_transfer` with `dial:<number>` |

A limit that does not cover all paths is easy to bypass.

## Options

1. Where to store the rules: (a) on each extension; (b) in tenant-wide
   profiles (class of service) that extensions refer to; (c) in the
   existing role model.
2. Rule evaluation: (a) an ordered list, first match wins; (b) the most
   specific rule wins.
3. Default after an update: (a) a restrictive default; (b) no limit.
4. Assignment: (a) extension only; (b) extension and user.

## Decision

### Model

TalkOps uses tenant-wide **dialing profiles** (option 1b). A profile has:

- a name and a description;
- a **base mode**: `allow` (all numbers are allowed) or `deny` (all numbers
  are blocked);
- a switch for **SIP address dialing** (`user@host` over the Internet);
- **rules**. Each rule has a kind, a value, an action (`allow` or `deny`)
  and a label. The kinds are:
  - `country`: an ISO 3166-1 alpha-2 code, for example `DE`. TalkOps finds
    the country of the number with the geolocation data of ADR 0044.
  - `prefix`: an E.164 prefix, for example `+49900`. The API also accepts
    the form `+49900*`.
  - `number`: one complete E.164 number, for example `+4930123456`.

Each tenant has zero or one **default profile**. Each extension can refer
to a profile; `NULL` means the default profile. A tenant without a default
profile has no limit. Profiles are not built in: the administrator can
change, rename and delete each profile. A profile that is the default or
that an extension uses cannot be deleted.

The role model (option 1c) is not used: roles control people in the web
interface, while dialing permissions control phone lines. A door station
or a lobby phone has no user.

### Evaluation

The most specific rule wins (option 2b). The order of the rules has no
effect, so a new rule cannot hide another rule by its position:

1. A `number` rule that is equal to the number.
2. Else, the `prefix` rule with the longest prefix of the number.
3. Else, the `country` rule for the country of the number.
4. Else, the base mode.

If two rules of the same rank match, `deny` wins. The database prevents
two rules with the same kind and value in one profile, so this case
occurs only with other data sources. The function
`talkops_core::dial_permissions::evaluate` makes this decision. It has no
I/O: its input is the E.164 number, the profile and the country of the
number; its output is the decision and the rule that matched.

The dialplan evaluates these classes of `DialPlanSettings::classify`:

- `Emergency`: **always allowed**. TalkOps does not evaluate a profile.
- `External`: the E.164 number.
- `Service` (short numbers such as `115`, `11833`): the national form
  `+<country code><digits>`, for example `+4911833`. Directory assistance
  numbers can be expensive, so profiles must also control them.
- `Invalid`: rejected as before.

A SIP address call is allowed only if the tenant setting and the profile
allow SIP address dialing.

If the geolocation data is not available, a `country` rule does not match.
A profile with base mode `deny` then blocks the number (fail closed). The
test endpoint shows that no country was found.

### Which profile applies on each path

| Path | Profile |
| --- | --- |
| Direct dialing, softphone, SIP address | The profile of the calling extension. |
| Call forwarding to an external number | The profile of the forwarding extension. If the profile blocks the number, the call rings the extension as if no forwarding were set. `*72` rejects a blocked target with `403`. |
| Blind transfer to an external number | The profile of the extension that transfers the call. Without this information: the default profile of the tenant. |
| Conference participant | The profile of the extension of the softphone (the loopback call goes through `plan_internal`). |
| Outbound fax | Each enabled extension of the fax owner must allow the number. An owner without an extension uses the default profile. |
| Smart Attendant direct dialing | Not affected: it reaches internal numbers only. |
| Attended transfer | Not affected: the consultation call already went through `plan_internal`. |

**Blind transfer.** FreeSWITCH transfers the partner leg of the device
that sends the SIP REFER. The partner leg does not contain a reliable
identity of the transferring extension (`Referred-By` is optional). TalkOps
therefore marks the partner leg itself with the channel variable
`talkops_xfer_extension_id`:

- A call from an extension exports `nolocal:talkops_xfer_extension_id` with
  the caller's extension to the called legs.
- Each device leg that the dialplan rings (`bridge` to an extension, a
  ring group or a number with more extensions) has the per-leg variable
  `[api_on_answer_talkops_xfer=uuid_setvar ${uuid} talkops_xfer_extension_id
  <extension>]`. The bridge expands `${uuid}` on the calling leg when it
  starts. When the device answers, FreeSWITCH runs the API command and the
  calling leg gets the extension of the answering device.
- A queue agent leg comes from `mod_callcenter`. `mod_callcenter` sets
  `cc_agent` (`a-<extension>`) on the waiting caller when an agent answers.
  The queue plan clears `talkops_xfer_extension_id` before the caller
  waits, so an earlier mark does not apply.

`plan_transfer` uses `talkops_xfer_extension_id`, else `cc_agent`. If both
are missing, the tenant default profile applies.

TalkOps does not put the mark into the directory (`<variables>` or
`dial-var-*`): FreeSWITCH does not set a variable value that contains
`${` on a new channel (`var_check`), and the directory cannot know the
calling leg.

**User assignment** (option 4a). TalkOps does not add a profile to users.
A user calls through the devices of an extension, so the extension
profile applies. Only fax uses the user as the sender. For fax, the strict
rule above applies. A second assignment on the user would give two places
for one decision and could conflict with the extension profile.

### Rejection

A blocked direct call gets `403 Forbidden`. If the system prompt
`dialing_blocked` is available, the caller first hears it as early media.
The call record has `direction = 'outbound'` and the reason in
`blocked_reason`:

- `dialing:base` - the base mode blocked the number;
- `dialing:sip` - SIP address dialing is not allowed;
- `dialing:country:<ISO>`, `dialing:prefix:+<digits>`,
  `dialing:number:+<digits>` - the rule that blocked the number.

The check constraint `cdr_blocked_reason_valid` accepts these values for
outbound calls. The log has the extension, the number and the reason.

### Upgrade and new installations

The migration creates five profiles for each tenant:

| Profile | Base mode | SIP addresses | Rules |
| --- | --- | --- | --- |
| Unrestricted | allow | yes | none |
| Internal only | deny | no | none |
| National | deny | yes | allow `+<country code>`, premium-rate blocks |
| National + Europe | deny | yes | allow `+<country code>`, EU, EEA and CH, premium-rate blocks |
| International | allow | yes | premium-rate blocks |

The default profile depends on the installation (option 3b with a
safe default for new systems):

- An existing installation (a tenant with an extension or a trunk) gets
  **Unrestricted** as its default. All calls work as before the update.
- A new installation gets **International** as its default. Premium-rate
  numbers and international networks are blocked.

### Premium-rate blocks

The migration and `talkops_core::dial_permissions::PREMIUM_RULES` contain
the same list. The administrator can change or remove each rule.

| Prefix | Meaning | Source |
| --- | --- | --- |
| `+49900` | DE premium-rate services (0900) | Bundesnetzagentur, numbering plan for premium-rate services |
| `+49137` | DE mass-traffic services (0137) | Bundesnetzagentur, numbering plan for mass-traffic services |
| `+49118` | DE directory enquiry services (118xy) | Bundesnetzagentur, numbering plan for directory enquiry services |
| `+4912` | DE innovative services (012) | Bundesnetzagentur, numbering plan for innovative services |
| `+4919` | DE online services and network-internal routing (019) | Bundesnetzagentur, numbering plan, range 019 |
| `+43900`, `+43901`, `+43930`, `+43931`, `+43939` | AT value-added services | RTR, KEM-V 2009 (Kommunikationsparameter-, Entgelt- und Mehrwertdiensteverordnung) |
| `+43118` | AT directory enquiry services | RTR, KEM-V 2009 |
| `+41900`, `+41901`, `+41906` | CH value-added services | BAKOM/OFCOM, E.164 numbering plan of Switzerland |
| `+870`, `+881`, `+882`, `+883` | Inmarsat, satellite and international networks | ITU-T, list of E.164 assigned country codes |

The list does not block shared-cost numbers (`+49180`): many companies use
them for their hotlines. The user documentation tells how to add them.

### Permission and API

The new permission `dialing.manage` (area `routing`, sensitive) controls
the profiles, the default profile, the assignment to extensions and the
test endpoint. Only administrators have it after the migration. A user
with `extensions.view` can read the profiles, so the extension list can
show the profile names. Each change writes an audit entry.

The API is under `/api/v1/dialing-profiles`. The assignment to an extension
uses its own endpoint, `PUT /api/v1/extensions/{id}/dialing-profile`. The
extension endpoint does not change the profile, so a user with
`extensions.manage` but without `dialing.manage` cannot change it.

## Consequences

- Positive: one place for outbound limits; all external paths use the same
  pure function. Emergency calls are never blocked.
- Positive: an update does not block calls that worked before.
- Negative: a `country` rule needs the geolocation data. Without it, a
  `deny` profile is stricter than configured.
- Negative: two extra queries for each outbound call (profile and rules).
  For up to about 50 extensions, this load is small.
- Negative: the blind-transfer marking depends on the FreeSWITCH variables
  `api_on_answer*`, `uuid_setvar` and `cc_agent`. The SIPp end-to-end test
  does not contain a REFER case yet. A transfer without a mark uses the
  default profile.
- Follow-up: time-dependent profiles (for example night mode) and cost
  limits are not part of this decision.
