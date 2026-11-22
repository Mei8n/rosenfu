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

## Optional bounded PBF operator pipeline

`from_pbf.py` uses separately installed **pyosmium 4.3.2** to select whole ways from a
fixed local PBF. It reads the snapshot timestamp from the header, selects an outer
`.poly` ring that wholly contains every emitted cell, and refuses any `.poly` hole.
The same cell enumerator drives PBF filtering and JSON emission. Multiple outer rings
are allowed only when one alone certifies all cells; no union coverage is invented.
Original PBF and polygon hashes are recorded. The completed manifest is staged with
all cells and renamed once. Source data is never downloaded by the generator.

```
python -B scripts/road-pack/from_pbf.py kanto-261114.osm.pbf \
  --poly kanto.poly --bounds 139.70 35.62 139.80 35.72 \
  --source-url https://download.geofabrik.de/asia/japan/kanto-261114.osm.pbf \
  --output new-tokyo-pack --max-pbf-bytes 600000000 --max-source-nodes 100000000
```

Default budgets are 256 MiB input / 20 million source nodes. Explicit budgets are
capped at 2 GiB / 200 million nodes. A temporary sparse file index lives beside the
output in the operator workspace; selected nodes and references remain capped at
one million each. The final XML/pack builder's existing limits also apply.
PBF fixture tests run with the pinned optional dependency; stdlib-only tests skip them.
Publishing requires copying the completed pack to public/road-data, setting its exact
manifest SHA in src/roadPackRelease.ts, retaining ODbL notices and running the real-cell
reader tests. Never promote partial output or change a manifest without changing its pin.

## National source extraction (operator-only, not an enabled app release)

`national.py` processes up to eight **same-timestamp** Geofabrik Japan PBF regions
sequentially using `osmium==4.3.2`, `shapely==2.1.2`, a file-backed node-location
index and SQLite (16 MiB page cache). It does not download or copy TrainsWorld code.

```sh
python national.py sources.json /absolute/output/not-yet-existing
```

`sources.json` is a list of `{ "name": "kanto", "pbf": "/.../kanto-261114.osm.pbf",
"poly": "/.../kanto.poly", "sha256": "<verified PBF SHA-256>",
"url": "https://download.geofabrik.de/asia/japan/kanto-261114.osm.pbf" }`.
Retain the provider receipt and matching source clipping polygon. Files are hash
verified before extraction. A current `.poly` alone is **not** evidence of a
historical clip boundary; the operator must establish the polygon applies to the
pinned PBF before distributing coverage claims.

The atomic output contains `index.json` (`rosenfu-road-pack-index`, version 1) and
`packs/<region>/<integer-longitude>_<integer-latitude>/manifest.json`. Each shard
contains at most 2,500 existing v1 `.02°` road cells. The index records source PBF
URL/hash/timestamp, polygon hash, shard path/hash/bytes/bounds and certified-cell
counts. Shard `source.format` is `osm-pbf`; `xml_sha256` is deliberately absent,
because this pipeline does not produce XML. The PBF hash is `file_sha256` and
`parent_pbf_sha256`. `coverage_geojson` has its own canonical JSON hash; original
`.poly` bytes are independently identified by `poly_sha256`.

Only cells wholly covered by the source clipping polygon are emitted, including
valid empty-road cells. Holes are subtracted. Partially intersecting regional
boundary cells are counted as **unknown and omitted**, not claimed empty or land.
A nationwide source inventory is not nationwide complete runtime coverage.
Consumers must verify the pinned index, shard manifests and every cell. A shard's
bounding rectangle is a locator, **not coverage**. Conflicting overlapping source
ways must be rejected by consumers, not silently replaced. All sources must use
exactly the same OSM replication timestamp. The source manifest records selected
OSM-road extraction completeness, not real-world road completeness.

Whole original node IDs, way IDs, vertex order and structure tags are retained.
No smoothing, missing-node inference, generalized-vector-tile completeness claim,
or map-background redistribution is performed. Road filtering matches the Tokyo
pack; excluded classes such as footways are not counted as vehicle roads.

Budgets are explicit: 2 GiB PBF/source, 200M source nodes/source, 10M ways/source,
300k certified cells/source, 10k nodes/way, 10k ways and 100k coordinates/cell,
5 MiB/cell or manifest, 12 GiB total output and 3 GiB free-disk floor. The full
national output is never held in RAM. Node indexes and SQLite spill to disk;
source processing removes their temporary indexes before starting the next source.
Any missing node, hash/version mismatch, invalid polygon, limit breach or exhausted
disk aborts without publishing an index/output directory. Nothing is truncated.
Actual national output size, source polygon provenance, cold-cache load, source
boundary behavior and hosting limits must be measured before enabling a release.

Data attribution: © OpenStreetMap contributors, ODbL 1.0, via Geofabrik.
Keep the original data's attribution/license and derived-database obligations;
this generator does not grant a new license. Hosting/distribution is a separate
step and is not performed by the generator.

Tests: `python -m unittest -v test_national` (optional pinned dependencies required).

**Current provenance gate:** the national index and each shard source explicitly
carry `coverage_verified: false` and `polygon_snapshot_match: "unverified"`.
`certified_cells` counts geometric containment in the supplied polygon only; it is
not permission to certify absence of roads or permit construction. Current `.poly`
downloads do not establish that a historical PBF used the same clipping geometry.
The data may serve reference display/following, but construction coverage must
remain UNKNOWN until that separate provenance gap is resolved and reviewed.

### Boundary revision evidence (2026-11-16 investigation)

The official [Geofabrik polygon history](https://github.com/geofabrik/polygons_download.geofabrik.de)
permits inspecting changes, but its README explains that download-server polygons
are intersections of regional and parent polygons, not the raw regional files.
At inspected tree `c26b05e77065591be18f83cbb38622468afd8af5`, the most recent
Kanto polygon author date was 2017-11-09, Japan 2022-03-16, Asia 2022-09-14.
The repository imported that history in 2025. These dates are evidence of stability,
not proof of the exact extraction configuration used for a particular PBF.

`kanto-261114.poly` returned HTTP 404. The official Kanto listing associated
`kanto-261114.osm.pbf` and the undated `kanto.poly` with the same displayed
2026-11-15 01:49 time, but does not publish a PBF-to-polygon hash binding.
Intersecting the pinned repository region/Japan/Asia polygons did **not** produce
geometrically identical boundaries to the downloaded polygons: Kanto symmetric
difference 0.0005398823 square degrees, Shikoku 0.0001223265. Rounding is a
possible explanation, not an established fact. No tolerance is used to turn this
into a coverage guarantee. Actual generated packs remain `coverage_verified:false`.
Even an exact clipping match would only certify extraction of selected OSM road
classes from that snapshot, not completeness of roads in the real world.

`generate_contract_fixture.py` creates an invented four-cell PBF and polygon
pair together, then explicitly marks **only that synthetic fixture** verified.
`src/nationalRoadRepository.test.ts` loads its real generator output through the
production index/shard/cell reader and checks source identity, revision and hashes.
It must not be used as a certification tool for real Geofabrik data.

### Independent disk budgets

The generator reserves at least 3 GiB globally (`min_free_bytes`) and caps its own
allocated filesystem blocks at 4 GiB (`work_bytes`) by default. Do not implement
the latter by subtracting current free space from startup free space: unrelated
builds would be charged to this generator. Accounting includes output files,
output directories and the active SQLite/node-index working directory, but not
input PBFs or other jobs. Immutable output files are statted once; only their
small directory chain and the few mutable index files are refreshed. Thus emitting
400,000 cells does not repeatedly scan 400,000 prior files. Sparse-file accounting
uses Linux `st_blocks * 512`, not logical size. Platforms without `st_blocks` use
logical bytes as a fallback; exact allocated-block accounting is supported only
where `st_blocks` is available. The guard runs at bounded extraction
checkpoints and before/after output writes; it detects an overage and aborts
without publication rather than providing an OS-enforced instantaneous quota.
Node indexes are released after extraction and before cell emission.

PBF extraction runs in a short-lived `spawn` process. In osmium 4.3.2 a named
`sparse_file_array` file descriptor can remain open even after reader closure,
cache clear, reference deletion and Python garbage collection. Waiting for the
child to exit makes descriptor release explicit before removing `nodes.idx`,
including on Windows. This is not a cleanup-error suppression. SQLite and the
node cache remain inside the same own-work budget; no source data is sent elsewhere.
Extraction/shutdown has a two-hour default deadline; error, timeout or parent
interruption terminates and joins the child before temporary-directory cleanup.
No index is published on failure.

## Native road region projection (offline candidate only)

One source can serve different consumers without treating a map line as
permission to drive. Run the existing v1 pack producer for crossing/reference
geometry and `build_native_region.py` for the directed bus/tram candidate from
the **same exact raw OSM XML**, coverage polygon, and revision:

```sh
cd scripts/road-pack
f=fixtures/shared-road-fixture
python3 build_native_region.py "$f/survey.osm" --coverage "$f/coverage.json" \
  --policy "$f/policy.json" --output /tmp/rosenfu-native-shared-road.json
python3 -m unittest test_build_native_region
```

The example is synthetic, never real-road authority. The build checks hashes
from the supplied region policy, not the truth or completeness of that policy.
Use a separately verified full regional source (including nodes, ways and
restriction relations, not a v1 crossing pack or generalized OpenFreeMap
tiles). Review origin/revision/bounds against its source, rights and updates
before any real artifact can be approved. This tool makes no network requests
and does not change the crossing pack format or production provider.

For this initial narrow profile, only complete interior residential and
unclassified ways with explicit `oneway=yes|no|-1` are considered. A region mode
and explicit `bus` or `tram=yes|designated` are both necessary; missing
permission remains unknown, and general denials, unsupported tags or structures,
tagged nodes, and affected restriction members exclude movement. Geometry comes
from ordered source node IDs, not proximity or a guessed junction at tile
seams. Unshared same-way continuation is derived; an actual shared junction
needs an explicit reviewed directed movement. Boarding positions are reviewed
per region once, not registered per bus route. Nothing here performs a legal
assessment of bus service or tram construction on the source roads.

The output uses the existing native candidate format 1. It cannot activate
real routes by itself: a release still needs an independent data review and
a digest pin in the native registry, which this tool never touches. The initial
fixture's three boarding positions permit different routes from one region;
the same graph cannot be widened by a save or frontend request. Source refresh
or loading more than one region in a world is outside this increment.

### Opt-in simulated simple junctions

The default stays strict, including byte-identical existing output. A separate
gameplay policy can generate movements once per **region**, not per play route.
It does not make a legal-turn determination or establish actual operator access.

First independently verify that the exact extraction has **every incident raw
way** (including ways this projection would otherwise discard) and **every
potentially applicable mapped restriction relation with all its members** across
the entire candidate bounds. A polygon, a clipped or filtered tile, a local
coordinate match, and a lack of restriction rows do not establish this. Keep
the extraction method and evidence with the review. If completeness is not
affirmative, omit the opt-in and keep the junction closed to implicit turns.

Record the independently reviewed regional attestation in a separate JSON file:

- format 1; `source_sha256`, `coverage_sha256`, `revision`, and `bounds` must bind
  the exact existing policy values;
- `incident_ways_complete` and `restriction_relations_complete` must both be
  literal `true`.

Add the attestation's own byte digest as `simulated_junctions_sha256` to the
region policy and pass `--junction-coverage PATH` along with the existing
inputs. Neither input alone opts in. The generator checks the binding, not the
truth of the independently reviewed attestation. Never use a fictional or
unverifiable attestation to register a real asset.

For this policy, only genuine, nondegenerate three-arm raw nodes on wholly
interior eligible at-grade ways can emit automatic turns. Degree-two and more
complex multiway joins remain strict, including coincident IDs that are not
actually the same node. An affected restriction relation, barrier, unsupported
or conditional tag/structure, duplicate or overlapping arm, or unknown mode
on any arm blocks automatic turns for the applicable junction/mode. No
U-turn or travel against the directed way order is added. The unchanged native
gate still checks the explicit per-mode ordered movements, including on
restore. Ordinary tram vertical limits and actual street rules are not
certified by this topology policy.

The candidate asset marks its origin `[Rosenfu simulated turns]`; the build
receipt identifies `simulated-simple-at-grade-v1` and the attestation digest.
No file is added to the native pin registry automatically.

### Independent opt-in for in-game bus stops

A regional policy may independently set `"allows_player_bus_stops": true`.
The generator then emits the same field in the native digest-pinned asset and
in the build receipt. A missing or explicit false value leaves the field absent
and does not change the candidate bytes. Non-boolean values are rejected.

This is a gameplay option to choose stops on already approved, directed bus
edges. It creates no real stops, does not grant mode access, and never adds
routing transitions. The simulated-junction policy does not enable player stop
placement, and player stop placement does not enable simulated junctions.

### Opt-in Japanese local-street bus simulation

`--bus-profile-coverage <certificate.json>` is a separate, offline option.
The region policy must list its exact SHA-256 as `simulated_bus_profile_sha256`
and enable the bus mode. The certificate is a format-1 JSON document with
`profile: "jp-local-simulated-bus-v1"`, `country: "JP"`, and the exact same
`source_sha256`, `coverage_sha256`, `revision`, and `bounds` as the region policy.
It must affirm `raw_way_access_and_direction_tags_complete`,
`raw_node_tags_complete`, `incident_ways_complete`, and
`restriction_relations_complete`. The raw XML must carry matching snapshot
revision metadata. This is an operator attestation, not evidence created by the
command; a clipped map response or a v1 crossing pack alone does not qualify.

The simulator accepts only supported ground `residential` and `unclassified`
ways. Explicit `oneway=yes|-1|no` is respected. On these simple street types
only, an omitted oneway tag yields two simulated travel directions, and an
omitted bus tag can allow **simulated bus use**. Explicit denials, variants such
as conditional/directional restrictions, unrecognized tags or structures,
unsupported surfaces, tagged nodes and applicable restriction relations stay
excluded. Exact `source` is accepted as attribution metadata only. The data
does not establish legal access, public ownership, road clearance, or actual
bus service; native candidate origin explicitly labels the simulation.
Tram eligibility and direction keep the original strict rules.

The profile can continue an eligible bus along the **same raw way** through a
fully covered, unambiguous three-arm junction with supported bus arms. It can
also continue straight through a simple four-arm junction when exactly two
supported original ways both pass through the same raw node. All four arms
must be distinct and separated by at least 30°, and each same-way pair
must deviate from straight by at most 30°. Each local arm must stay strictly
inside the source certificate's coverage polygon. An arm beyond the playable
rectangle but inside that polygon may count only as evidence: no road edges
are emitted because of it. Split, ambiguous, and tagged or restricted
approaches remain excluded. The profile adds no U-turns or
turns between different ways. The existing, separately opted-in
simple-junction policy still needs explicit mode tags on every arm for its
separate turn derivation. Existing policies and candidate bytes are unchanged
when the bus profile is not selected. No production pin is created.

Tag sources: [OSM Japan road conventions](https://wiki.openstreetmap.org/wiki/Japan_tagging),
[OSM oneway conventions](https://wiki.openstreetmap.org/wiki/Key:oneway),
and [OSM access fallback caveats](https://wiki.openstreetmap.org/wiki/Default_access_restrictions).
