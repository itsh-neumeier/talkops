# 0044 - Call origin map and number geolocation

- Status: Accepted by the user on 2026-10-23 (development prompt "Anrufkarte",
  with fixed decisions).
- Date: 2026-10-23
- Uses: ADR 0027 (phone book numbers in call lists). Changes no earlier ADR.

## Context

The call log (`/calls`) gets a map view. The map shows where external
inbound calls come from and where outbound calls go. The dashboard gets a
small version of the map. The map shows bubbles for local network areas
(Ortsnetzbereiche). The required precision:

- Germany, Austria and Switzerland (DACH): the local network area of the
  area code.
- Other countries: as precise as the available data, at minimum the country.

TalkOps runs in family homelabs and small businesses. Many installations
have no permanent Internet access for the web UI, or the administrator does
not want to send data to third parties. The default must therefore work
offline.

A phone number does not identify the location of the caller:

- Number portability: a number keeps its area code when the owner moves
  inside the area or, for some services, outside it.
- VoIP: a provider can assign a number with an area code to a customer in
  another place. Nomadic use is permitted for many VoIP numbers.
- Mobile numbers have no location. Some countries (for example China)
  publish the home region of a mobile range. This is not the location of
  the caller.

## Options

1. Data source for German areas: (a) the BNetzA GIS data of the local
   network areas; (b) the libphonenumber geocoding data; (c) a commercial
   data set.
2. Storage: (a) static data files in `presets/geo/`, loaded at startup;
   (b) database tables with a migration.
3. Resolution: (a) in the backend, before aggregation; (b) in the browser.
4. Map library: (a) Leaflet; (b) MapLibre GL JS; (c) own SVG drawing.
5. Base map: (a) a shipped country outline file, external tiles optional;
   (b) external tiles (OpenStreetMap) as default.

## Decision

Use options 1a, 2a, 3a, 4a and 5a.

### Data sources and licenses

| File | Content | Source | License |
| --- | --- | --- | --- |
| `de.tsv` | 5,200 local network areas of Germany, a representative point inside each area (`representative_point` of the largest polygon) | Bundesnetzagentur: ONB boundaries (GIS, status 2026-08-10) and the area code list `NVONB.INTERNET.20220727.ONB.csv` | © Bundesnetzagentur. The Bundesnetzagentur permits free use, also in free or commercial software and for third parties. Keep the copyright notice. |
| `at.tsv` | 1,022 local networks of Austria (KEM-V 2009) | RTR Open Data `rnOrtsnetze` (`https://data.rtr.at/api/v1/tables/rnOrtsnetze.json`); coordinates of the main place from GeoNames `AT.zip` | RTR Open Data: free use, the only condition is to name the source (RTR-GmbH). GeoNames: CC BY 4.0. |
| `ch.tsv` | 20 area codes of Switzerland, maintained by hand | BAKOM/OFCOM numbering plan; coordinates of the main place from GeoNames `cities15000` | Numbering plan: facts. GeoNames: CC BY 4.0. |
| `service.tsv` | Service and other non-geographic ranges of DE, AT and CH, maintained by hand | BNetzA, RTR and BAKOM numbering plans | Facts. |
| `world.tsv` | Places by prefix for other countries | libphonenumber `resources/geocoding/en` (tag `v9.0.42`, without +41, +43 and +49); coordinates from GeoNames `cities15000` | libphonenumber: Apache License 2.0 (checked in the official repository; the data files carry the Apache 2.0 header). GeoNames: CC BY 4.0. |
| `mobile.tsv` | Mobile ranges of all countries | libphonenumber `resources/carrier/en` (prefixes only, no carrier names) | Apache License 2.0. |
| `countries.tsv` | Countries with their calling codes and a label point | GeoNames `countryInfo.txt`; Natural Earth 1:50m label points | GeoNames: CC BY 4.0. Natural Earth: public domain. |
| `web/static/map/countries.geojson` | Base map (country outlines, simplified, 0.05°) | Natural Earth 1:50m admin 0 countries | Public domain. |

The map shows the attribution of all sources. `THIRD_PARTY_NOTICES.md` and
the user documentation contain the same notices.

The script `tools/geo/generate.py` generates all files except `ch.tsv` and
`service.tsv` from the raw data. `tools/geo/fetch.sh` downloads the raw data
from fixed URLs. The raw data (about 60 MB) is not in Git. Each data file
records its source, license and date in its header.

### File format and lookup

The files are tab-separated text (format in `presets/geo/README.md`). An
`A` line defines an area: ID, ISO country, kind (`area`, `mobile`,
`service`, `country`), latitude, longitude, display prefix and name. A `P`
line maps a prefix to an area. Each prefix is unique in all files.

`talkops_core::geo::GeoIndex` loads all `*.tsv` files of
`TALKOPS_GEO_DIR` (default `/usr/share/talkops/presets/geo`) at startup. It
stores the prefixes in a hash map (digits and length packed into one
`u64`). A lookup is a longest-prefix match on the digits of an E.164
number. The number must have 6 to 15 digits and at least one digit after
the prefix. Before the lookup, the API normalizes the number to E.164 with
the tenant dial plan (`Directory::external`), the same function that
resolves phone book names. National (`0951…`) and international (`0049…`)
formats therefore resolve.

The data is static and the same for all tenants. A database table gives no
advantage. A new data version comes with a new release.

If the directory does not exist, the server starts and logs a warning. The
map then shows all calls without a location. Invalid data stops the start,
like invalid trunk presets.

### Measured size and load time

- Data files: 2.7 MB in total (`world.tsv` 1.7 MB, `mobile.tsv` 0.5 MB,
  `de.tsv` 0.4 MB, `at.tsv` 0.1 MB). 19,584 areas and 81,687 prefixes.
- Load time: 78 ms in a release build, 260 ms in a debug build (test
  `geo::tests::resolves_numbers_with_the_shipped_data`).

### Precision rules

- DACH: the area of the area code. AT areas without a GeoNames place
  (11 of 1,022) get the mean point of the areas with the same leading
  digits.
- Other countries: a place from libphonenumber if GeoNames knows the place
  name. For a state or province name, the largest city of that region.
  Else the country.
- libphonenumber has no English geocoding data for some countries (for
  example France). Numbers of these countries resolve to the country.
- Geocoding prefixes inside mobile ranges are ignored. Such numbers count
  as mobile numbers.
- Mobile, service and unknown numbers of a country form one map bubble
  "Mobile / no location" at the label point of the country.

### Map library and base map

Leaflet (BSD-2-Clause) draws the map. It is small, shows GeoJSON without a
tile server and supports an optional tile layer. The bubbles are HTML
markers: a keyboard can focus them, and Enter opens the details. The map
has one bubble for each area with calls, so the number of markers stays
small. MapLibre GL JS needs WebGL and vector tiles; the map does not
need these functions. The web UI loads Leaflet with a dynamic import only
when the map view opens. The main bundle does not grow.

The default base map is the shipped country outline file. The page then
sends no request to another host. An administrator can set a tile URL and
an attribution text in the system settings (third PR of the feature). A
tile server receives the IP address of the browser and the map section.

## Consequences

- Positive: the map works offline, the data is versioned with the code, and
  a lookup costs microseconds.
- Positive: the data has a reproducible generator. A data update is a
  generator run and a review of the diff.
- Negative: the repository and the server image grow by about 3.2 MB
  (2.7 MB data, 0.5 MB base map).
- Negative: the map shows the area of a number, not the location of the
  caller (number portability, VoIP). The user documentation states this.
- Follow-up work: the API endpoint and the map view (second PR), the
  dashboard widget and the optional tile URL (third PR).
