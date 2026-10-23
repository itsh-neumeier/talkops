#!/bin/sh
# Downloads the raw data for tools/geo/generate.py into a directory.
# The raw data is large and is not stored in Git. See tools/geo/README.md.
set -eu

RAW=${1:?usage: fetch.sh <raw-dir>}
LIBPHONENUMBER_TAG=${LIBPHONENUMBER_TAG:-v9.0.42}
BNETZA_ONB_ZIP=${BNETZA_ONB_ZIP:-https://data.bundesnetzagentur.de/Bundesnetzagentur/SharedDocs/Downloads/DE/Sachgebiete/Telekommunikation/Unternehmen_Institutionen/Nummerierung/Rufnummern/ONRufnr/ONBGrenzen/Aktuelle_Grenzen_10082026.zip}
BNETZA_NAMES_ZIP=${BNETZA_NAMES_ZIP:-https://www.bundesnetzagentur.de/DE/Fachthemen/Telekommunikation/Nummerierung/ONRufnr/_DL/Vorwahlverzeichnis_ONB.zip.zip?__blob=publicationFile}
RTR_ORTSNETZE=${RTR_ORTSNETZE:-https://data.rtr.at/api/v1/tables/rnOrtsnetze.json}
GEONAMES=https://download.geonames.org/export/dump
NATURAL_EARTH=https://naciscdn.org/naturalearth/50m/cultural/ne_50m_admin_0_countries.zip

mkdir -p "$RAW"
cd "$RAW"

get() {
    curl -fsSL --retry 3 -o "$1" "$2"
}

get onb.zip "$BNETZA_ONB_ZIP"
get onb-names.zip "$BNETZA_NAMES_ZIP"
get rtr-ortsnetze.json "$RTR_ORTSNETZE"
get cities15000.zip "$GEONAMES/cities15000.zip"
get AT.zip "$GEONAMES/AT.zip"
get countryInfo.txt "$GEONAMES/countryInfo.txt"
get admin1CodesASCII.txt "$GEONAMES/admin1CodesASCII.txt"
get ne_50m_admin_0_countries.zip "$NATURAL_EARTH"

rm -rf onb onb-names at ne50 libphonenumber
unzip -oq onb.zip -d onb
unzip -oq onb-names.zip -d onb-names
unzip -oq AT.zip -d at
unzip -oq cities15000.zip
unzip -oq ne_50m_admin_0_countries.zip -d ne50

git clone -q --depth 1 --branch "$LIBPHONENUMBER_TAG" --filter=blob:none --sparse \
    https://github.com/google/libphonenumber.git libphonenumber
git -C libphonenumber sparse-checkout set --no-cone \
    /resources/geocoding/en/ /resources/carrier/en/ /LICENSE
echo "$LIBPHONENUMBER_TAG" > libphonenumber/TAG

echo "raw data is in $RAW"
