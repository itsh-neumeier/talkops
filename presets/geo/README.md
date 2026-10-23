# Call origin data

The files in this directory map phone number prefixes to areas for the call
map (ADR 0044). The server loads all `*.tsv` files of `TALKOPS_GEO_DIR` at
startup.

| File | Content | Maintained |
| --- | --- | --- |
| `de.tsv` | Germany, 5,200 local network areas (BNetzA) | generated |
| `at.tsv` | Austria, 1,022 local networks (RTR) | generated |
| `ch.tsv` | Switzerland, area codes (BAKOM) | by hand |
| `service.tsv` | DE, AT, CH service and non-geographic ranges | by hand |
| `world.tsv` | Other countries, places (libphonenumber, GeoNames) | generated |
| `mobile.tsv` | Mobile ranges of all countries (libphonenumber) | generated |
| `countries.tsv` | Countries and calling codes (GeoNames, Natural Earth) | generated |

Do not edit generated files. Change `tools/geo/generate.py` and run it
again. See `tools/geo/README.md`.

## Format

UTF-8 text, one record on each line, fields separated by a tab.

```text
# key: value
A	<id>	<country>	<kind>	<lat>	<lon>	<cc rest>	<name>
P	<digits>	<id>
```

- A line that starts with `#` is a comment. The comments `title`, `source`,
  `license` and `updated` identify the data. Each file must have them.
- `A` defines an area:
  - `id`: unique in all files, for example `DE-951`.
  - `country`: ISO 3166-1 alpha-2 code in upper case.
  - `kind`: `area` (local network area or place), `mobile`, `service` or
    `country`.
  - `lat`, `lon`: WGS 84 degrees.
  - `cc rest`: the display prefix, the calling code and the rest separated
    by a space, for example `49 951`.
  - `name`: the name of the area in the language of the source.
- `P` maps a prefix (E.164 digits without `+`) to an area. A prefix can
  refer to an area in another file. Each prefix must be unique in all
  files.

A lookup uses the longest prefix that matches the number.

## Licenses

- `de.tsv`: © Bundesnetzagentur. Keep the copyright notice.
- `at.tsv`: Source: RTR-GmbH (RTR Open Data). GeoNames, CC BY 4.0.
- `world.tsv`, `mobile.tsv`: libphonenumber, Apache License 2.0. GeoNames,
  CC BY 4.0.
- `countries.tsv`: GeoNames, CC BY 4.0. Natural Earth, public domain.
- `ch.tsv`, `service.tsv`: official numbering plans. Coordinates: GeoNames,
  CC BY 4.0.
