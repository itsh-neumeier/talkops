# Setting up Yealink phones

TalkOps configures Yealink desk phones (T3x, T4x, T5x, VP59, CP9x0) and DECT
bases (W60B, W70B, W80B) automatically: SIP accounts, line keys with BLF,
phonebooks, time zone, language, admin password and firmware.

> **Status:** All parameters come from the official *Yealink Auto
> Provisioning Guide*. Tests with real devices are still pending – feedback
> (including "works with T46U, firmware …") is very welcome.

## 1. Add the phone

1. **Phones → Add phone**: enter a name, the MAC address (label on the back)
   and the model.
2. **Extensions → extension → Add device**: choose the phone under *Phone*.
   The *account* (1, 2, …) is assigned automatically; on DECT bases it is the
   handset number.

One phone can carry several extensions (e.g. account 1 = own extension,
account 2 = reception).

## 2. Point the phone to TalkOps

**Phones → Auto-provisioning → Show provisioning URL** shows the URL including
credentials, e.g.

```
http://provision:Ab3dE…@192.168.1.10:8080/provisioning
```

**Option A – DHCP option 66 (recommended):** add the URL to your DHCP server
as option 66 (type *text/string*). New or factory-reset phones then configure
themselves.

| DHCP server | Setting |
|---|---|
| UniFi | *Settings → Networks → network → DHCP → Custom DHCP Option*: code 66, type text |
| OPNsense / pfSense | *Services → DHCPv4 → LAN → Additional options*: number 66, type text |
| dnsmasq / Pi-hole | `dhcp-option=66,"http://provision:…@192.168.1.10:8080/provisioning"` |
| FRITZ!Box | no option 66 – use option B |

**Option B – manually:** in the phone's web interface (default login
`admin`/`admin`) under *Settings → Auto Provision*: *Server URL* =
`http://192.168.1.10:8080/provisioning`, *username* `provision`, *password*
from TalkOps, then **Autoprovision Now**.

The phone reboots and registers. Afterwards the **phone admin password** shown
in TalkOps is required for the phone's web interface. The phone list shows the
last contact, IP address and firmware version.

## 3. Line keys and BLF

On the phone's page you can assign keys:

- **Line** – key for one of the accounts (default for the first keys).
- **BLF (extension)** – shows whether an extension is idle, ringing or busy.
  Pressing a flashing BLF key picks up the call.
- **Speed dial** – dials a number.

After saving press **Resync**: the phone reboots and applies the changes.
Phones that are not registered fetch the configuration on their next start or
at night between 2 and 4 am.

## 4. Feature codes

| Code | Function |
|---|---|
| `*78` / `*79` | do not disturb on / off |
| `*72<number>` / `*73` | forward all calls (extension or external number) / off |
| `**<extension>` | pick up a ringing call |
| `*97` | voicemail (phase 3) |

The phone's DND key toggles *do not disturb* of the extension on account 1.
Both settings are also available in the web UI on the extension page and
under *My phones*.

## 5. Phonebook

Phones show two phonebooks: **Internal** (all enabled extensions) and
**Contacts** (the shared phonebook under *Phonebook* in the web UI). Numbers
are shown in national format and can be dialed directly.

## 6. Firmware

Download firmware from the Yealink support site (`.rom` or `.bin`), upload it
under **Phones → Firmware** for the matching model and **activate** it. Phones
update on their next resync. At most one image per model is active.

## Troubleshooting

- **Phone fetches nothing:** is port 8080 reachable from the phone network?
  The `talkops` container log shows `provisioning request from unknown phone`
  when the MAC address has not been added.
- **401 in the phone log:** wrong or regenerated credentials – update DHCP
  option 66 or the auto-provision settings. After 10 failed attempts TalkOps
  blocks the IP for 5 minutes.
- **BLF key stays dark:** the monitored extension needs at least one enabled
  device; run a resync after changing keys.
- **Check the configuration:** *Show configuration* on the phone page shows the
  generated file (it contains SIP passwords, every view is audited).

## Security

Provisioning uses HTTP on the LAN. The files contain SIP passwords, so run
TalkOps in a trusted network only, or behind the optional Caddy with HTTPS
(then use `https://…/provisioning`). **Generate new passwords** replaces the
provisioning and phone admin passwords.
