#!/usr/bin/env python3
"""Generates the detail layers of the offline call map (ADR 0044).

Usage (after tools/geo/fetch.sh):
    /tmp/geo-venv/bin/python -I tools/geo/basemap.py /tmp/talkops-geo-raw

Writes to web/static/map/:
    admin1.geojson      state and province borders (lines)
    rivers.geojson      rivers, property `z`: first zoom level
    lakes.geojson       lakes (polygons), property `z`
    places-<z>.json     place labels, one file for each first zoom level:
                        [name, lat, lon, population, German name if different]
    onb.geojson         German local network areas (polygons, property `id`)

The web UI loads each file when the map reaches its zoom level.
"""

import argparse
import csv
import json
import os
from collections import defaultdict

import shapefile
from pyproj import Transformer
from shapely.geometry import mapping, shape
from shapely.ops import linemerge, unary_union

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "web", "static", "map")

# Countries with fine detail: places down to 1,000 inhabitants.
FINE = {"DE", "AT", "CH", "LI"}

# First zoom level of a place by population (ADR 0044, PR 4 specification).
PLACE_ZOOMS = [(500_000, 5), (50_000, 7), (10_000, 8)]
FINE_ZOOM = 10

# First zoom level of all layers that are not places.
ADMIN1_ZOOM = 5
MAJOR_WATER_ZOOM = 5
WATER_ZOOM = 7
ONB_ZOOM = 8


def rounded(geo, digits):
    def r(c):
        if isinstance(c, (list, tuple)) and c and isinstance(c[0], (int, float)):
            return [round(c[0], digits), round(c[1], digits)]
        return [r(x) for x in c]

    return {"type": geo["type"], "coordinates": r(geo["coordinates"])}


def write_geojson(name, features, attribution):
    path = os.path.join(OUT, name)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(
            {"type": "FeatureCollection", "attribution": attribution, "features": features},
            f,
            separators=(",", ":"),
            ensure_ascii=False,
        )
    print(f"{os.path.relpath(path, ROOT)}: {len(features)} features, {os.path.getsize(path)} bytes")


def records(raw, name):
    reader = shapefile.Reader(os.path.join(raw, "ne10", name), encoding="utf-8")
    fields = [f[0].lower() for f in reader.fields[1:]]
    for sr in reader.iterShapeRecords():
        if sr.shape.shapeType == shapefile.NULL:
            continue
        yield dict(zip(fields, sr.record)), sr.shape


# Europe has fine detail; other regions are simplified more.
EUROPE = (-25.0, 34.0, 45.0, 72.0)


def in_europe(geom):
    x0, y0, x1, y1 = geom.bounds
    return x1 >= EUROPE[0] and x0 <= EUROPE[2] and y1 >= EUROPE[1] and y0 <= EUROPE[3]


def water_feature(geom, props):
    if in_europe(geom):
        return feature(geom, props, 0.005, 3)
    return feature(geom, props, 0.03, 2)


def feature(geom, props, tolerance, digits):
    geom = geom.simplify(tolerance, preserve_topology=True)
    if geom.is_empty:
        return None
    return {"type": "Feature", "properties": props, "geometry": rounded(mapping(geom), digits)}


def merged(lines, props, tolerance, digits):
    """One feature for many lines: joins touching segments first."""
    if not lines:
        return []
    geom = linemerge(unary_union(lines))
    f = feature(geom, props, tolerance, digits)
    return [f] if f else []


def admin1(raw):
    fine, europe, world = [], [], []
    for rec, shp in records(raw, "ne_10m_admin_1_states_provinces_lines"):
        geom = shape(shp.__geo_interface__)
        if rec.get("adm0_a3") in ("DEU", "AUT", "CHE", "LIE"):
            fine.append(geom)
        elif in_europe(geom):
            europe.append(geom)
        else:
            world.append(geom)
    out = (
        merged(fine, {}, 0.005, 3)
        + merged(europe, {}, 0.02, 2)
        + merged(world, {}, 0.08, 2)
    )
    write_geojson(
        "admin1.geojson",
        out,
        "Natural Earth 1:10m admin 1 boundary lines (public domain), simplified",
    )


def water_zoom(rec):
    # Natural Earth gives each feature the first zoom level of its own web
    # maps; large rivers and lakes have a small value.
    try:
        z = float(rec.get("min_zoom") or 99)
    except ValueError:
        z = 99
    return MAJOR_WATER_ZOOM if z <= 5 else WATER_ZOOM


def name_of(rec):
    return (rec.get("name_de") or rec.get("name") or "").strip()


def rivers(raw):
    groups = defaultdict(list)
    for layer in ("ne_10m_rivers_lake_centerlines", "ne_10m_rivers_europe"):
        for rec, shp in records(raw, layer):
            if rec.get("featurecla", "").lower().startswith("lake"):
                continue
            geom = shape(shp.__geo_interface__)
            groups[(water_zoom(rec), in_europe(geom))].append(geom)
    out = []
    for (z, europe), lines in sorted(groups.items()):
        out += merged(lines, {"z": z}, 0.005 if europe else 0.03, 3 if europe else 2)
    write_geojson(
        "rivers.geojson",
        out,
        "Natural Earth 1:10m rivers and lake centerlines, Europe supplement (public domain)",
    )


def lakes(raw):
    out = []
    for layer in ("ne_10m_lakes", "ne_10m_lakes_europe"):
        for rec, shp in records(raw, layer):
            geom = shape(shp.__geo_interface__)
            # Small lakes outside Europe are not visible at the offline zoom levels.
            if not in_europe(geom) and geom.area < 0.05:
                continue
            f = water_feature(geom, {"z": water_zoom(rec)})
            if f:
                out.append(f)
    write_geojson("lakes.geojson", out, "Natural Earth 1:10m lakes, Europe supplement (public domain)")


def place_zoom(pop, country):
    for limit, z in PLACE_ZOOMS:
        if pop >= limit:
            return z
    if country in FINE and pop >= 1_000:
        return FINE_ZOOM
    return None


def german_names(raw):
    """German names of large places from Natural Earth (by GeoNames ID, else
    by English name and position)."""
    by_id, by_name = {}, defaultdict(list)
    for rec, shp in records(raw, "ne_10m_populated_places"):
        de = (rec.get("name_de") or "").strip()
        en = (rec.get("name_en") or rec.get("name") or "").strip()
        if not de or de == en:
            continue
        lon, lat = shp.points[0]
        try:
            gid = str(int(rec.get("geonamesid") or 0))
        except ValueError:
            gid = "0"
        if gid != "0":
            by_id[gid] = de
        by_name[en.lower()].append((lat, lon, de))
    return by_id, by_name


def places(raw):
    by_zoom = defaultdict(list)
    de_by_id, de_by_name = german_names(raw)

    def german(gid, name, lat, lon):
        if gid in de_by_id:
            return de_by_id[gid]
        for la, lo, de in de_by_name.get(name.lower(), ()):
            if abs(la - lat) < 0.3 and abs(lo - lon) < 0.3:
                return de
        return None

    seen = set()
    # cities1000 covers all places >= 1,000; other countries keep >= 15,000
    # (the GeoNames cities15000 limit) and the zoom rules above.
    with open(os.path.join(raw, "cities1000.txt"), encoding="utf-8") as f:
        for row in csv.reader(f, delimiter="\t", quoting=csv.QUOTE_NONE):
            gid, name, lat, lon, cls, code, cc, pop = (
                row[0], row[1], row[4], row[5], row[6], row[7], row[8], int(row[14] or 0)
            )
            if cls != "P" or code in ("PPLX", "PPLH", "PPLQ", "PPLW"):
                continue
            if cc not in FINE and pop < 15_000:
                continue
            z = place_zoom(pop, cc)
            if z is None or gid in seen:
                continue
            seen.add(gid)
            item = [name, round(float(lat), 3), round(float(lon), 3), pop]
            de = german(gid, name, float(lat), float(lon))
            if de and de != name:
                item.append(de)
            by_zoom[z].append(item)
    for z, items in sorted(by_zoom.items()):
        items.sort(key=lambda p: -p[3])
        path = os.path.join(OUT, f"places-{z}.json")
        with open(path, "w", encoding="utf-8") as f:
            json.dump(
                {
                    "attribution": "GeoNames (CC BY 4.0), https://www.geonames.org/",
                    "zoom": z,
                    "places": items,
                },
                f,
                separators=(",", ":"),
                ensure_ascii=False,
            )
        print(f"{os.path.relpath(path, ROOT)}: {len(items)} places, {os.path.getsize(path)} bytes")


def onb(raw):
    shp_dir = os.path.join(raw, "onb")
    base = next(f[:-4] for f in os.listdir(shp_dir) if f.endswith(".shp"))
    reader = shapefile.Reader(os.path.join(shp_dir, base), encoding="utf-8")
    to_wgs84 = Transformer.from_crs("EPSG:31467", "EPSG:4326", always_xy=True).transform
    parts = defaultdict(list)
    for sr in reader.iterShapeRecords():
        geom = shape(sr.shape.__geo_interface__)
        if not geom.is_valid:
            geom = geom.buffer(0)
        parts[str(sr.record[0]).strip()].append(geom)
    out = []
    for code in sorted(parts, key=lambda c: (len(c), c)):
        # Simplify in metres (Gauss-Krüger), then project.
        geom = unary_union(parts[code]).simplify(250, preserve_topology=True)
        if geom.is_empty:
            continue
        geo = mapping(geom)

        def project(c):
            if isinstance(c, (list, tuple)) and c and isinstance(c[0], (int, float)):
                lon, lat = to_wgs84(c[0], c[1])
                return [round(lon, 3), round(lat, 3)]
            return [project(x) for x in c]

        out.append(
            {
                "type": "Feature",
                "properties": {"id": f"DE-{code}"},
                "geometry": {"type": geo["type"], "coordinates": project(geo["coordinates"])},
            }
        )
    write_geojson(
        "onb.geojson",
        out,
        "© Bundesnetzagentur, local network area boundaries (ONB), simplified",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("raw", help="directory from tools/geo/fetch.sh")
    args = parser.parse_args()
    os.makedirs(OUT, exist_ok=True)
    admin1(args.raw)
    rivers(args.raw)
    lakes(args.raw)
    places(args.raw)
    onb(args.raw)


if __name__ == "__main__":
    main()
