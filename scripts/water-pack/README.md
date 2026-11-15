# Water evidence pack (incomplete coverage)

This offline builder preserves water-area observations, waterway lines with unknown
surface width, and coastlines as separate records. **It cannot certify dry land or
complete water coverage.** Both manifest and payload always contain
`geographic_completeness: "unknown"` and `coverage: []`. A valid empty pack is still
unknown; missing or corrupted bytes are errors. Never set `water_ready` from an
observed-water count, the requested bounds, or a successful pack read.

## Build

Python standard library supports bounded RFC 7946 GeoJSON. OSM PBF additionally
requires `python -m pip install -r scripts/water-pack/requirements.txt` in an isolated
environment. It does not install into or modify other workers' environments.

```sh
python scripts/water-pack/build.py shikoku-261114.osm.pbf \
  --bounds 134.02 34.32 134.08 34.37 \
  --revision 2026-11-14T21:20:41Z \
  --source-url https://download.geofabrik.de/asia/japan/shikoku-261114.osm.pbf \
  --license ODbL-1.0 \
  --license-url https://opendatacommons.org/licenses/odbl/1-0/ \
  --attribution '© OpenStreetMap contributors; extracted by Rosenfu' \
  --output new-takamatsu-water-evidence
python -B -m unittest discover -s scripts/water-pack -p 'test_*.py' -v
```

`revision`, URL and attribution are operator-supplied provenance claims. The builder
also records the actual input SHA-256 and, for PBF, available source header timestamp
and generator. Check these agree before publication. It checks the file hash before
and after extraction. A hash proves byte identity, not truth or geographic completeness.

GeoJSON input is a FeatureCollection with string feature IDs. Each feature must set
`properties.evidence_kind` to `water-area`, `waterway-line`, or `coastline`, and may
set `properties.source_tags` to a string dictionary. Only Polygon/MultiPolygon are
accepted for areas and LineString for lines. Coordinates are longitude, latitude;
custom CRS declarations are rejected. A GML/Shapefile converter is **not** included.

## Preservation and limits

- PBF selection: `natural=water`, `waterway=riverbank`, or
  `landuse=reservoir|basin` areas; `waterway=*` lines; `natural=coastline` lines.
  This is not an exhaustive real-world water classification. Wetlands, pools and
  untagged water are not guaranteed. Original tags (including intermittent, seasonal,
  tunnel and covered) remain observations; no surface-state inference is made.
- Pyosmium assembles areas, including relation holes. Unassemblable relations can be
  omitted without an area callback. `source_issues: []` therefore does **not** certify
  successful assembly of all water geometry or source completeness.
- Full output geometry is retained without application-level simplification,
  clipping or arbitrary line buffering. Selection uses bounding-box intersection
  and can over-include features. Requested bounds are not a completeness mask.
- Source input <=768 MiB; GeoJSON <=5 MiB; selection <=0.2 degrees per axis;
  <=10,000 observations and <=100,000 coordinates; payload <=5 MiB. PBF reading is
  streaming but the location index can still be large. Use regional inputs rather
  than planet data. No full-country build was performed.
- Existing output directories are refused. Payload is written before the manifest;
  a partial directory without a valid pinned manifest is unusable. No existing files
  are deleted. Temporary directories should use a workspace location via TMPDIR.

## Reader and integration boundary

`readWaterEvidencePack(manifestBytes, expectedManifestSha256, payloadBytes)` validates
pinned manifest identity, payload hash/size, revision, bounds, counts, geometry and
unknown-only coverage. It snapshots caller buffers before asynchronous hashing.
The returned `status` is `incomplete`; `coverage` is always empty. Surface polygons
are available for inspection through `water`; lines and all original tags remain
in `observations`. No network reader, automatic source switch, App/hook integration,
save-format change, or physics modification is included.

Do not substitute this pack for a complete water repository. A future reviewed
coverage contract must establish source scope, ocean/river/lake handling, precision,
missing-data detection and valid region independently. Unknown-width rivers cannot
be converted to dry land or a made-up-width water strip.

## Data licensing

Code licensing does not cover source data. OSM-derived packs are ODbL-1.0 data with
OpenStreetMap contributor attribution; source/modified database distribution must
follow that license. The manifest records the data license separately. The builder
does not approve arbitrary source rights. GSI and MLIT candidates have separate
conditions; see [source comparison](../../docs/water-source-comparison.md).

## Reproducible live-data probe

On 2026-11-15 UTC, an existing, unmodified Geofabrik Shikoku PBF was reused:

- Source bytes: 92,206,331; SHA-256
  `ecf914fd451f911b976ef8c37d672c43f8c0234cd2016e8305c8c107551f7c6d`.
- Header timestamp: `2026-11-14T21:20:41Z`.
- Takamatsu bounds: `[134.02,34.32,134.08,34.37]`.
- 67 area features (70 polygons, 21 inner rings), 285 waterway lines, 21 coastlines.
- Payload bytes: 246,469; SHA-256
  `798518a64cdd9b45fbca37e02b3437e56db810e11be1b93e359f061d1b8c99a6`.
- Manifest SHA-256:
  `b3940b5e2032a24c565bb9fbb3d7cc4cdcca428aa3a529cbe1c6cea0b16e7b1a`.
- Reader verification succeeded with `status=incomplete`, `coverage=[]`.

The full source and generated probe remain in the cloud workspace, not the code
repository. Rebuild using the identical source hash and command above. Synthetic
GeoJSON and PBF tests run without downloading geographic data in CI.

## Multi-cell batch (one source pass)

`grid.py` reads one regional PBF/GeoJSON once and distributes full observations to
explicit bounds in a JSON array. It emits the same pinned leaf packs plus a final
`catalog.json`. Pin the catalog SHA-256 externally, then use each entry's manifest
pin with the existing reader. There is no application catalog loader yet. Missing
cells/files, source issues, and valid empty cells cannot grant dry-land coverage.

```sh
python scripts/water-pack/grid.py shikoku-261114.osm.pbf \
  --cells cells.json --revision 2026-11-14T21:20:41Z \
  --source-url https://download.geofabrik.de/asia/japan/shikoku-261114.osm.pbf \
  --license ODbL-1.0 --license-url https://opendatacommons.org/licenses/odbl/1-0/ \
  --attribution '© OpenStreetMap contributors; extracted by Rosenfu' \
  --output new-water-grid
```

Each batch permits <=256 explicit cells (each <=0.2 degrees per axis), <=768 MiB
input, <=100,000 emitted observation copies, <=1,000,000 emitted coordinates and
<=128 MiB serialized output. Each leaf also retains the stricter original limits.
The PBF location index still consumes memory. This is a bounded regional building
block, **not completed nationwide generation**. National source partitions, source
revision consistency, extract borders, job scheduling, resumability and a trusted
catalog loader remain unimplemented. Split large jobs; do not raise limits blindly.
All source issues are copied into every cell because their location may be unknown.
Boundary-touching observations may occur in multiple cells: deduplicate with source
snapshot + kind + ID; do not sum lengths/areas from cell copies.

Live Shikoku validation: four Takamatsu cells, 395 emitted observation copies and
9,395 coordinates; all four read successfully as incomplete, coverage empty. Bounds:
`[[134.02,34.32,134.05,34.345],[134.05,34.32,134.08,34.345],
[134.02,34.345,134.05,34.37],[134.05,34.345,134.08,34.37]]`.
Catalog SHA-256: `289c22ae85dc6bd56250c688e98d15698f1ed2024512b4d44a19dac7857f5a15`.
Cloud artifact: `/workspace/shared/rosenfu-water-pack-artifacts/takamatsu-grid-v1`.

## Marine polygon adapter (synthetic validation only)

`marine.py` is **only for an operator-verified OSM-derived water-polygons dataset**
with verified ODbL rights and source date. It fixes the OSM license/attribution and
must NOT be used with GSI, MLIT or arbitrary Shapefiles. Source URL and revision are
operator claims; the tool cannot authenticate the actual source dataset. Install
`requirements-marine.txt` in your own environment. Provide local, already extracted
`.shp/.shx/.dbf/.prj` files from the unsimplified WGS84 split water-polygons dataset:

```sh
python scripts/water-pack/marine.py water_polygons.shp \
  --bounds 134.02 34.32 134.08 34.37 --revision VERIFIED_SOURCE_REVISION \
  --source-url https://osmdata.openstreetmap.de/download/water-polygons-split-4326.zip \
  --output new-marine-evidence
```

The adapter accepts 2D EPSG:4326 polygons, validates rings before conversion,
rejects pyshp's orphan-hole promotion, invalid shapes, open rings and antimeridian
segments, then intersects with the small requested rectangle. It preserves interior
islands and source polygon overlaps. It does not simplify, buffer, repair, union,
construct oceans from lines, or claim the dataset includes inland water. Clip edges
are artificial selection boundaries, not observed coasts. The geometry engine and
clipping operation are recorded. Exact bytes may vary across geometry-engine
versions; use the newly reported hash, not a presumed cross-platform hash.

Source bytes are limited to 8 GiB across four components, PRJ to 1 MiB, selected
source records to 10,000, points per source record to 100,000, and output to the
original pack limits. `source.file_sha256` here hashes the canonical component
inventory; the inventory also contains each actual file's hash. It is **not** the
ZIP's hash or the SHP's hash. Inputs are hashed before and after processing.

Synthetic tests cover island holes, clip boundaries, projection mismatch, orphan
holes, invalid polygons and empty results, including Python-to-TypeScript readback.
The real worldwide marine ZIP connection timed out on 2026-11-15; no real marine
archive was downloaded or transformed. Global processing and marine/inland fusion
remain unverified. The resulting pack still has `coverage=[]`, even for an apparently
all-ocean rectangle. Absence of a marine polygon is never proof of dry land.
