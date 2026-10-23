# Third-party provisioning sources

The directory `crates/talkops-provisioning/reference/fusionpbx` contains all
1,037 provisioning source files from FusionPBX commit
`20143aec4366afc2755f336d3058243479d49d44`. This includes the 433 Yealink files
previously imported at `be0d0e3220e6b3666775fa46db6f1accc60c79c6`.
All 433 Yealink blobs have the same bytes at both commits.

Source: https://github.com/fusionpbx/fusionpbx/tree/20143aec4366afc2755f336d3058243479d49d44/resources/templates/provision

The Original Code is FusionPBX. The Initial Developer is Mark J Crane.
The upstream `resources/licenses.php` identifies the Mozilla Public License
Version 1.1. A copy is in `reference/fusionpbx/licenses.php`. The complete
license is in `reference/fusionpbx/MPL-1.1.txt` in the provisioning crate.
These reference files are unchanged. Their Git blob IDs are recorded in
`docs/research/fusionpbx-provisioning-files.json`. The complete import date is
2026-10-17. The earlier Yealink inventory remains available.

The contents of these reference files are subject to the Mozilla Public
License Version 1.1. You may obtain the License at
https://www.mozilla.org/MPL/1.1/. The software is provided on an AS IS basis,
without warranty of any kind. Refer to the included license for its terms.
The reference source remains available in this repository and the server's
authenticated template library. The server image includes this notice and
the upstream license files in `/usr/share/doc/talkops`.

The reference files are data. TalkOps does not execute upstream PHP or Smarty.
The source-derived files under `crates/talkops-provisioning/templates/fusionpbx`
are modified derivatives of these sources. They retain the MPL-1.1 terms. Each file identifies
its source commit, path and blob. The build scripts record and reproduce the
conversion and TalkOps-specific changes. These derivative files are available
in this repository and through authenticated native-template copies.
The independently maintained files under `templates/yealink` and the application
code use the TalkOps project license. The source library records template
content; it does not certify a phone model or firmware.

The directory `reference/fusionpbx-apps` also contains the original
`app/devices/app_config.php` and `app/provision/resources/classes/provision.php`
blobs from the same revision. Their hashes are in
`docs/research/fusionpbx-function-sources.json`. These files retain their source
license notices. The literal function records in `profiles/functions.json` are
derived data under the same MPL-1.1 terms. TalkOps parses those records at build
time and does not execute either PHP file.

The independently authored `groundwire/groundwire/directory.json.jinja` file
implements the public web contacts format. It uses the TalkOps project license.
It is not a derivative of a FusionPBX template.

# Call origin map data

The files in `presets/geo/` and `web/static/map/countries.geojson` contain
data from these sources (ADR 0044). The map in the web UI shows the same
notices.

- Local network areas of Germany (also the area outlines of the map):
  © Bundesnetzagentur. Source: ONB boundaries and the area code list of the
  Bundesnetzagentur. Free use,
  also in software and for third parties, with this copyright notice.
- Local networks of Austria: Source: RTR-GmbH, RTR Open Data `rnOrtsnetze`.
- Place coordinates, place labels of the map and country data: GeoNames
  (https://www.geonames.org/),
  licensed under Creative Commons Attribution 4.0
  (https://creativecommons.org/licenses/by/4.0/). TalkOps uses selected
  coordinates and names.
- Number prefixes of other countries and mobile ranges: libphonenumber
  (https://github.com/google/libphonenumber), Copyright The Libphonenumber
  Authors, licensed under the Apache License, Version 2.0
  (https://www.apache.org/licenses/LICENSE-2.0). TalkOps uses prefixes and
  place names from `resources/geocoding/en` and prefixes from
  `resources/carrier/en`, in a changed format.
- Country outlines, label points, state borders, rivers, lakes and German
  names of large places (map detail layers): Natural Earth
  (https://www.naturalearthdata.com/), public domain.
- Map library: Leaflet 1.9.4 (https://leafletjs.com/), Copyright Volodymyr
  Agafonkin and CloudMade, BSD 2-Clause License. The web UI bundles it and
  loads it only in the map view.
