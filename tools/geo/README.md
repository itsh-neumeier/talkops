# Geo data generator

These scripts generate the call origin data in `presets/geo/` and the base
map `web/static/map/countries.geojson` (ADR 0044).

## Procedure

1. Download the raw data (about 60 MB) to a directory outside the
   repository:

   ```sh
   tools/geo/fetch.sh /tmp/talkops-geo-raw
   ```

2. Install the Python packages in a virtual environment:

   ```sh
   python3 -m venv /tmp/talkops-geo-venv
   /tmp/talkops-geo-venv/bin/pip install -r tools/geo/requirements.txt
   ```

3. Generate the files:

   ```sh
   /tmp/talkops-geo-venv/bin/python -I tools/geo/generate.py /tmp/talkops-geo-raw
   ```

4. Run the data test:

   ```sh
   cargo test -p talkops-core --lib geo
   ```

5. Examine the diff of `presets/geo/` and the warnings of the script.

## Sources

`fetch.sh` uses fixed URLs. Set these variables for a newer version:

| Variable | Default |
| --- | --- |
| `BNETZA_ONB_ZIP` | BNetzA ONB boundaries, status 2026-08-10 |
| `BNETZA_NAMES_ZIP` | BNetzA area code list (`Vorwahlverzeichnis_ONB.zip`) |
| `RTR_ORTSNETZE` | `https://data.rtr.at/api/v1/tables/rnOrtsnetze.json` |
| `LIBPHONENUMBER_TAG` | `v9.0.42` |

GeoNames (`cities15000.zip`, `AT.zip`, `countryInfo.txt`,
`admin1CodesASCII.txt`) and Natural Earth (`ne_50m_admin_0_countries.zip`)
have no version in the URL. The `updated` comment of each generated file
records the generation date.

The BNetzA publishes a new boundary file after each change of the local
network areas. Find the current link on the BNetzA page "Ortsnetzbereiche,
-kennzahlen und -bereichsgrenzen".

## Rules of the generator

- Germany: the representative point of the largest polygon of each area
  (the point is always inside the area). The names come from the area code
  list.
- Austria: the GeoNames place with the name of the local network
  (administrative seats first, then the largest population). Areas without
  a match get the mean point of the areas with the same leading digits.
- Other countries: the GeoNames place with the name of the libphonenumber
  entry. A state or province name gives the largest city of that region.
  Entries without a place are left out (the number then resolves to the
  country). Entries inside mobile ranges are left out.
- Prefixes are made shorter where possible: ten sibling prefixes with the
  same area become their parent prefix, and a prefix with the same area as
  its nearest shorter prefix is left out.
