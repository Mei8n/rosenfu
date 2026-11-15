# Road pack builder (offline, Python standard library)

Build immutable, unsimplified road cells from an **operator-certified OSM extract**.
This tool has no network client and does not fetch or publish data. It is not yet
connected to Rosenfu's automatic road repository. No game height is inferred from
`layer`, `level`, `bridge`, or `tunnel`.

```
python3 scripts/road-pack/build.py source.osm.bz2 \
  --coverage source-extract-coverage.geojson \
  --bounds 139.70 35.66 139.74 35.70 \
  --revision source-timestamp-or-immutable-id \
  --source-url https://example.invalid/source.osm.pbf \
  --output new-pack-directory
python3 -B -m unittest discover -s scripts/road-pack -p 'test_*.py' -v
```

PBF input is not implemented. An operator may convert it with separately installed
`osmium cat input.osm.pbf -o source.osm`; that executable is **not** a dependency
installed by this tool. Keep the original PBF hash and conversion provenance in the
operator's publication records. The manifest's `file_sha256` identifies the actual
XML/bz2 input, **not** a PBF at `source.url`. `xml_sha256` identifies decompressed XML.
`revision` and source URL are operator-supplied claims, not remotely verified.

## Coverage contract

`--coverage` is a GeoJSON Polygon or Feature containing a Polygon, supplied by the
operator from the exact extract's documented clipping polygon. XML `<bounds>` and
`--bounds` alone never certify coverage. The tool records the original GeoJSON and
its hash in the manifest, checks every full 0.02-degree cell is inside that polygon,
and rejects concave intrusions. It cannot prove that the operator's polygon matches
the input extract: that provenance check is a publication prerequisite.

Only simple single-ring polygons with 3–512 vertices are accepted. Holes,
MultiPolygons, antimeridian crossing, and coordinates outside ±85 latitude are not
supported. Split these into separately certified regions first. Requested bounds
are rounded outward to the fixed grid; certification is required for the full
resulting cells. Border numerical uncertainty may reject a valid pack; never enlarge
the source polygon merely to silence rejection. A road's full source geometry may
extend beyond coverage, but that geometry does not extend certified coverage.

Coverage means all selected roads **present in this source snapshot** were extracted.
It does not establish that OSM contains every real road. Empty cells have normal,
hashed JSON files with `ways: []`; missing/corrupt files are never empty coverage.
Clients must verify manifest revision and each cell hash before granting coverage.

## Selection and preservation

Matches `src/automaticRoadData.ts`: motorway, motorway_link, trunk, trunk_link,
primary, primary_link, secondary, secondary_link, tertiary, tertiary_link,
unclassified, residential, living_street, service. `area=yes` is excluded.
Footway/path/cycleway/track and other classes are deliberately outside this contract.
No access filtering is applied. This is a versioned game-road selection, not a claim
that other classes are nonexistent.

Each intersected cell receives the **whole way**, including every source node ID and
coordinate in original order. Boundary-touching ways are included in both cells.
IDs are decimal strings, preserving values above JavaScript's safe integer limit.
Ways retain version and original bridge/tunnel/layer/level/highway/access/motor_vehicle
strings (missing tags are null). No simplification, line merging, inferred heights,
or synthesized OSM identifiers occur. Cross-cell clients deduplicate by revision +
way ID, preserving segment indices from original node order.

## Limits and failure behavior

Defaults: 128 MiB compressed **and** expanded input, 4096 cells, 100000 way elements
and emitted way copies, 1000000 node coordinates, road references and emitted
coordinates; coverage JSON is limited to 1 MiB. Limits can be lowered or explicitly
raised with `--max-input-bytes`, `--max-cells`, `--max-features`, `--max-coords`.
Whole XML parsing is bounded by the expanded byte cap; this is an offline regional
builder, not a streaming planet importer. Distribution work is proportional to
road segments × selected cells; choose small regions. Missing referenced road nodes,
duplicate IDs/tags, malformed geometry, DTD/entities and limit excess fail the build.
An existing output directory is refused; validated output is staged then renamed.
No partial manifest is published. No current-directory files are deleted.

## Publication obligations

Generated packs retain ODbL-1.0 and © OpenStreetMap contributors attribution with
license/source links. Publish derived database data under the applicable ODbL terms;
keep attribution visible in the application. See https://www.openstreetmap.org/copyright
and https://opendatacommons.org/licenses/odbl/1-0/ . Source provider delivery terms
also apply. This tool does not turn another provider's display tiles into certified
OSM extracts and does not grant permission to fetch or redistribute third-party packs.
