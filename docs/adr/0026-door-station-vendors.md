# 0026 – Door station vendors and SIP password formats

- Status: Accepted
- Date: 2026-10-13

## Context

ADR 0013 connects Dahua VTO door stations. TalkOps treats the station as a
SIP device of an extension and controls it through the Dahua HTTP API.
Dahua was implicit: the database, the API and the documentation had no
vendor. Other vendors can come later.

Dahua VTH indoor monitors also register as SIP devices. Two VTH5422H with
firmware V4.800.0.2 got `403 Forbidden` after the `401` challenge. The same
account registered with a softphone. TalkOps generated SIP passwords with
20 characters. The Yeastar guide for Dahua VTH devices sets a length of
8–15 characters. This limit is not confirmed for this model and firmware.
FreeSWITCH also answers `403` for an unknown user name and for a user name
that is different from the authentication name. A VTH sends its Room No.
as the SIP user name.

## Options

Vendor:

1. Structure only the UI and the documentation by vendor.
2. Add a vendor column to `door_stations` and select the integration by
   vendor. Add a common interface for vendor integrations when a second
   vendor comes.
3. Option 2 with a common interface (trait) for all integrations now.

Password:

1. Generate 15 characters for all devices.
2. Generate 15 characters for the device type `door`.
3. A password length as a parameter of the reset, not stored.
4. A password that the admin types.
5. A stored password format per device.

## Decision

Vendor: option 2.

- `door_stations.vendor` (enum `door_vendor`, value `dahua`, default
  `dahua`). Existing stations and API clients without the field stay Dahua.
- `DoorCtx` selects the HTTP client by vendor. SIP, ring routing, video and
  DTMF detection stay vendor neutral.
- The web UI has a setup guide per vendor and device type. A guide is data
  (`web/src/lib/doors/`); its texts are i18n keys `doorGuide.<vendor>.*`.
- User documentation: a vendor-neutral page and one page per vendor.
- To add a vendor: add the enum value (migration), add the HTTP client
  behind a common interface, add the guide data and the user documentation
  page.

A common interface now (option 3) would be designed for one example only.

Password: option 5.

- `devices.sip_password_format`: `standard` (20 characters, default) or
  `compact15` (15 characters). The alphabet stays the same (57 letters and
  digits, about 87 bits for 15 characters).
- The format applies when TalkOps generates a password: on create and on
  reset. A change of the format does not change the stored password. An
  admin can change the format in the reset request. Other users reset with
  the stored format.
- The web UI preselects `compact15` for new devices of the type `door`. The
  API default stays `standard`.
- Browser devices always use `standard`.

Options 1 and 2 change the default for devices that work. Option 3 loses
the choice at the next reset. Option 4 allows weak passwords.

Diagnosis: TalkOps logs failed SIP authentications and directory lookups for
unknown users with the user name and the address (never the password).

## Consequences

- Migrations `20261030000001_device_sip_password_format` and
  `20261030000002_door_station_vendor`.
- The guide marks information that is not confirmed with real devices.
- A hardware test with the VTH and a 15-character password is open.
