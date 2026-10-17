# Manufacturer settings and device copies

Select the exact model when you add a phone. The source library contains the
complete pinned FusionPBX provisioning tree. Spectralink and Grandstream Wave
have no usable configuration there. They remain visible as blocked entries.

## Edit manufacturer settings

Open a phone and find **Model settings**. Fields are grouped by function:
language and time, network, SIP, calls, audio, display, keys, contacts,
provisioning, maintenance and security. Search matches parameter names,
comments and source files. Open a category to show its fields. Use **Show more
fields** for the next page. **Only device overrides** filters configured values.

Each field shows its source parameter and file. Source comments describe
options where available. An empty value is an explicit empty value. **Use
template default** removes the override. Leave a saved secret field unchanged
to preserve it. Use its reset button to remove it. List settings accept a JSON
array of strings, for example `["112", "110"]`.

TalkOps manages SIP credentials and identity through their own controls.
Those fields are read-only in the manufacturer form. Source coverage is not
a complete firmware manual. Check the device manual for supported values and
additional firmware features. Hardware verification is shown separately from
source conversion. Successful rendering still requires device testing.

Set common language, IANA time zone and NTP values in **Provisioning transport**.
A phone can override them. Manufacturer fields expose device-specific locale
formats. A manufacturer field override takes precedence. Some phones need a
language dictionary. Use the manufacturer field and model manual where no
common-format mapping exists.

The account count follows source constraints. Without a source bound, the
profile starts with one account. Enter the count documented for the device.
Remove higher account assignments before you reduce this count.

## Deliver files

Native profiles list their provisioning directory and all configuration files.
Use the directory address or primary file as specified by the device manual.
The files require provisioning credentials. Configuration views and QR exports
require phone management permission and create audit entries.

App devices receive a generated ID if their identity field is empty. Assign a
SIP account before exporting an app profile. Cloud Softphone also needs its
cloud code. QR codes contain credentials. Treat them as secrets.

Profiles without remote command adapters use scheduled downloads or the device
menu. Their UI does not offer remote resync or reboot. TalkOps serves HTTP(S).
A phone that requires TFTP needs a separate delivery service. Swissvoice has
no directory artifact in the source. Its directory URL fields start empty.
Use a compatible external directory if required. Firmware files are not bundled.

## Copy a phone

1. Select **Use settings from a phone**.
2. Select the source and review the copy. The model stays fixed.
3. Enter a new name and MAC address. An app can generate a new ID.
4. Select new extension assignments if required.
5. Create the phone.

The copy gets independent SIP credentials. It excludes source credentials,
secret parameters, runtime state, source account labels and device-specific
fields. Keys without an assigned account stay inactive. Later source changes
do not update the copy. The operation sends no resync. Reload a stale preview
before creating a copy.

The source revision and license are in the source library. Native template
copies retain the model artifact manifest. Activation checks file structure,
credential bindings and empty, single and full account fixtures.

See [provisioning profiles](provisioning.md) for transport and template controls.
