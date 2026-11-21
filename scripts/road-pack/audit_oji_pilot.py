#!/usr/bin/env python3
"""Audit pinned Oji source geometry offline; do not emit routable assets."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import xml.etree.ElementTree as ET

LIMIT = 8 * 1024 * 1024
PIN = "7b18c0cc76de0b75015d25defb1e274a56e5d65d7d32ffca9fa7a363c8c32bda"
BOUNDS = (139.7325, 35.7479, 139.7405, 35.755)
SLICES = (
    ("road", "209826275", "676419612", "1309049788", "highway", "trunk"),
    ("tram", "343234687", "1923675893", "1923675895", "railway", "tram"),
)


def tags(item):
    return {t.attrib["k"]: t.attrib["v"] for t in item.findall("tag")}


def index(root, kind):
    items = root.findall(kind)
    result = {item.attrib["id"]: item for item in items}
    if len(result) != len(items):
        raise ValueError("duplicate OSM " + kind + " ID")
    return result


def audit(root):
    nodes, ways = index(root, "node"), index(root, "way")
    ways_refs = {wid: [n.attrib["ref"] for n in way.findall("nd")] for wid, way in ways.items()}
    restrictions = []
    for relation in root.findall("relation"):
        rt = tags(relation)
        if any(k.split(":")[0] == "restriction" for k in rt) or (
            rt.get("type", "").split(":")[0] == "restriction"
        ):
            restrictions.extend((m.attrib["type"], m.attrib["ref"]) for m in relation.findall("member"))

    observed = {}
    for mode, wid, first, last, key, value in SLICES:
        refs, wt = ways_refs[wid], tags(ways[wid])
        if refs.count(first) != 1 or refs.count(last) != 1 or refs.index(first) >= refs.index(last):
            raise ValueError("missing, duplicate or reversed endpoint")
        ids = refs[refs.index(first):refs.index(last) + 1]
        if len(ids) != len(set(ids)):
            raise ValueError("repeated node in slice")
        if wt.get(key) != value or wt.get("oneway") != "yes":
            raise ValueError("unknown direction or unexpected mode")
        if any(k.endswith(":conditional") for k in wt):
            raise ValueError("conditional way tag needs review")
        if any(wt.get(k, d) != d for k, d in (("layer", "0"), ("bridge", "no"), ("tunnel", "no"))):
            raise ValueError("non-default layer or structure needs review")
        ids_set = set(ids)
        if any((kind == "way" and ref == wid) or (kind == "node" and ref in ids_set)
               for kind, ref in restrictions):
            raise ValueError("restriction relation affects slice")
        if any(other != wid and ids_set.intersection(refs) for other, refs in ways_refs.items()):
            raise ValueError("shared node with another way")
        coords = []
        for node in (nodes[nid] for nid in ids):
            if tags(node):
                raise ValueError("tagged node needs review")
            x, y = float(node.attrib["lon"]), float(node.attrib["lat"])
            w, s, e, n = BOUNDS
            if not (w + .0001 < x < e - .0001 and s + .0001 < y < n - .0001):
                raise ValueError("slice approaches/exceeds snapshot boundary")
            coords.append((x, y))
        metres = sum(math.hypot((b - a) * math.cos(math.radians((c + d) / 2)), d - c) * 111195
                     for (a, c), (b, d) in zip(coords, coords[1:]))
        if not 0 < metres < 100:
            raise ValueError("unexpected pilot length")
        observed[mode] = {"way": wid, "node_count": len(ids), "approx_metres": round(metres, 1)}
    return {"bounds": BOUNDS, "source_sha256": PIN, "slices": observed,
            "attribution": "© OpenStreetMap contributors, ODbL-1.0",
            "attribution_url": "https://www.openstreetmap.org/copyright",
            "result": "snapshot_topology_only", "routing_authorized": False}


def check(raw):
    if len(raw) > LIMIT or hashlib.sha256(raw).hexdigest() != PIN:
        raise ValueError("not the pinned bounded Oji XML snapshot")
    root = ET.fromstring(raw)
    bounds = root.findall("bounds")
    if root.tag != "osm" or root.get("version") != "0.6" or len(bounds) != 1 or tuple(
        float(bounds[0].attrib[k]) for k in ("minlon", "minlat", "maxlon", "maxlat")
    ) != BOUNDS:
        raise ValueError("unexpected schema or bounds")
    return audit(root)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    args = parser.parse_args()
    try:
        with args.source.open("rb") as file:
            print(json.dumps(check(file.read(LIMIT + 1)), ensure_ascii=False, sort_keys=True))
    except (ValueError, OSError, KeyError, ET.ParseError) as exc:
        parser.exit(1, f"Oji evidence rejected: {exc}\n")


if __name__ == "__main__":
    main()
