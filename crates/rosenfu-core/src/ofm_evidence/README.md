# Offline OFM evidence decoder, checkpoint 1

This module is not connected to WorldDraft, storage, IPC, or construction. It does
not validate or rebuild derived seam paths yet. A decoded result is not proof of
real-world identity, supplier authenticity, road coverage, or access permission.

## Input and resource contract

`decode_request(&[EvidenceInput { binding, raw }])` validates a complete request
before returning any output. No side effects occur on failure. `decode_tile` is
also available; callers assembling a request must not use it to bypass aggregate
limits. Bindings pin the exact HTTPS URL, revision, zoom 14, x/y, lowercase raw
SHA-256, and `ofm_transportation_z14_v1`. Same request: one provider revision,
unique tile coordinates, 1–16 tiles, <= 5 MiB each and <= 8 MiB aggregate (including
reused raw blobs). Identical raw blobs at different tile coordinates remain distinct
bindings and distinct decoded hashes. Storage deduplication is a later layer.

The protobuf reader is independently implemented from the MVT 2.1 wire schema:
https://github.com/mapbox/vector-tile-spec/blob/master/2.1/vector_tile.proto
It borrows byte slices and checks lengths before allocation. No protobuf groups,
gzip, network fetch, last-one-wins singular fields, invalid UTF-8, multi-valued
Value, duplicate attribute keys, or invalid wire types are accepted. Unknown
protobuf fields are skipped within the bounded input. Non-transportation layer
bodies are scanned only for names, never materialized. There is exactly one
transportation layer, version 2, extent 1..2^24 (default 4096).

Per tile: <= 64 layers, <= 10,000 transportation features, <= 20,000 dictionary
keys and values each, <= 256 tags per feature, <= 20,000 emitted road points,
<= 100,000 road geometry words. Dictionary keys <= 1 KiB; value strings <= 4 KiB.
Only recognized road classes and LineStrings become context. Feature indices are
original transportation layer positions, not renumbered after filtering. Each
line retains its original line index. Coordinates use checked accumulation and
must stay within +/- 16 extents and i32. This deliberately rejects unusually large
buffers instead of guessing or clipping them.

Known road attributes (`class`, `subclass`, `access`, `brunnel`, `layer`, `oneway`)
retain typed protobuf values. Strings are nonempty, <= 100 UTF-8 bytes, and reject
Unicode Cc/Cf. Numeric layer/oneway accept finite values; integer values must be
JS-safe. Other known attributes require strings. Unknown brunnel values reject,
never become ground. Raw unknown oneway strings are retained without interpreting
permissions. `layer` numeric strings currently use Rust finite decimal parsing;
unusual JS Number spellings such as hexadecimal are intentionally rejected rather
than normalized. Geometry uses MoveTo(count=1) and LineTo only, at least two points
per line; malformed/short lines reject instead of being silently skipped. These
strict backend limits can reject data the current frontend decoder tolerates.

## Canonical decoded digest v1

SHA-256 over this byte stream, without JSON, geographic projection, normalization,
or floating-point arithmetic:

1. ASCII `rosenfu-ofm-integer-context-v1` then NUL.
2. Strings decoder rule, canonical URL, revision, raw SHA. Every string is a u32
   little-endian UTF-8 byte length followed by exact UTF-8 bytes.
3. u8 zoom; u32 LE x, y, line count.
4. Each line in supplier feature/line order: u32 LE feature index, line index,
   extent, point count; each point as i32 LE x then y; u32 LE attribute count.
5. Attributes ordered by ASCII key. Key string followed by typed value:
   `1` string (length-prefixed); `2` f32 raw bits u32 LE; `3` f64 raw bits u64 LE;
   `4` int64 i64 LE; `5` uint64 u64 LE; `6` sint64 decoded i64 LE;
   `7` boolean u8 (0 or 1). Type tags are u8.

Raw SHA is deliberately part of the digest. Byte-distinct inputs with the same
visible roads are not interchangeable evidence. JS graph JSON SHA is unrelated
and must not be substituted. `DecodedTile` is an output DTO, not a trusted proof:
future storage/session boundaries must re-decode raw bytes rather than accepting
caller-mutated DTOs.

## Tests and real corpus

`cargo test -p rosenfu-core --test ofm_evidence` exercises synthetic golden,
binding/revision/hash substitution, malformed wire and geometry, resource limits,
known attribute rejection, atomic failure, and 2,000 deterministic byte mutations.
It is not a substitute for continuous fuzzing or independent security review.

Optional real corpus test (no network access or corpus checked into the repo):

```sh
OFM_TEST_TILE=/path/to/east.pbf cargo test -p rosenfu-core \
  --test ofm_evidence real_tile_corpus -- --ignored --nocapture
```

Known fixture: revision `20261101_080013_pt`, tile 14/14553/6451, 761,140 raw bytes;
3,165 lines and 10,461 points, matching frontend decoder counts. Canonical hash:
`4dff9abd3b403072fd23d23bf5f91aa17143b524981f352abec89bd9df9b6886`.
Attribution: © OpenMapTiles © OpenStreetMap contributors, ODbL-1.0.
Only synthetic generated bytes are part of the committed tests.

Dependency: official RustCrypto sha2 pinned at 0.10.9, already present in workspace
lockfile. No parser crate allocation precedes resource validation, and no new
protobuf code generator or runtime dependency is introduced.

## Immutable evidence store, checkpoint 2

`EvidenceStore::from_parts(Vec<RawEvidence>, Vec<TileBinding>)` accepts only raw
bytes and bindings, never a claimed decoded hash/context. `with_additions` produces
a new validated store; the original is unchanged on success or failure. Both use
`decode_request` for the whole nonempty candidate. Missing raw, orphan payloads,
SHA mismatch, same URL with changed binding, mixed revisions, and limits reject.
Equal duplicate raw inputs deduplicate by SHA; unequal bytes cannot replace them.

`EvidenceStore::clone` shares a private Arc<State>. Raw blobs and decoded tiles
also use Arc and are reused when additions leave their content unchanged. The
cache key includes raw SHA, revision, zoom/x/y and decoder rule, not just SHA.
Cache reuse never bypasses validation. No mutator or Deserialize implementation
exists. `descriptors` returns small owned binding/byte-length/canonical-hash DTOs;
mutating a descriptor cannot mutate or attest to the store. `decoded(url)` and
`raw(sha)` provide read-only references and reject missing keys. `bindings()` and
`blobs()` provide read-only iterators for future bounded atomic save encoding.

A calculation still takes at most 16 bindings, 8 MiB counting reused raw per
binding, and 5 MiB per tile. The retained-world archive is separate: sequential
bounded requests can retain up to 256 bindings, 8 MiB unique raw, and 64 MiB
counting repeated binding bytes to bound verification work. All share one pinned
provider revision. Restore derives each digest from validated raw bytes before
publishing; it does not load a world-wide decoded geometry graph. A calculation
materializes the full recorded scope on request, never a silent subset. A small
LRU retains at most 16 decoded tiles with a 16 MiB estimated geometry weight;
eviction does not change raw bytes, validation or save contents. The
archive keeps raw for Undo and offline reconstruction; it does not imply
complete coverage, multiple revisions, unbounded allocation or real-road
vehicle access. Overall save-file and provenance-count limits still apply.

License/attribution remain mandatory external source-binding metadata in the
future save envelope. This low-level decoder/store does not validate legal source
metadata or pretend the provider cryptographically authenticated the bytes.

## Raw-context boundary candidates, checkpoint 3 (not path reconstruction)

`boundary_candidates(store, required_tiles)` independently projects integer MVT
coordinates and recomputes conservative correspondence hypotheses from **all**
candidate-class roads in the complete requested store. Required coordinates must
exactly match the stored bindings; duplicate/missing/extra required coordinates
reject. A caller-supplied line subset or graph hash is never accepted. This does
not establish that unrequested external tiles exist or have been acquired. A
trusted path request must determine the acquisition scope before construction.

Rules follow the frozen PR164 helper: primary/secondary/tertiary/minor/service,
absent brunnel, layer absent or numeric zero, oneway absent or numeric 0/1/-1.
String directions remain ineligible; absent attributes do not equal explicit
values. Layer uses the frontend's numeric interpretation; numeric protobuf value
encodings compare numerically, not by wire tag. Class/subclass/access and raw
oneway strings are never coerced. All original typed attributes remain unchanged
in EvidenceStore. No access permission is inferred.

Only adjacent tile pairs are considered. Each source must traverse the full
20 m boundary band monotonically, without reentry, boundary-parallel portions,
disconnected clipping, tile-corner contact or >100 m clipped length. Tangent dot
product >=0.995 and bidirectional continuous distance upper bound <=0.8 m are
required. The upper bound uses distance to one convex target segment, recursively
bisecting at most 8 levels. One shared 1,000,000 segment-evaluation budget covers
the entire invocation. Limits: 2,000 candidate-class lines, 100,000 points,
512 points per line, 10,000 eligible clipped pair comparisons. Exhaustion returns
an error, never partial candidates. Each tile boundary requires mutual uniqueness;
a middle fragment may legitimately have one neighbor at each of two boundaries.

Output is sorted by fragment IDs. `validate_boundary_candidates` checks the full
claimed list against regeneration: identities/order/count exact, numeric metrics
finite and within 1e-6 m. This tolerance is only for cross-language floating-point
comparison, not a relaxation of the 0.8 m geometric candidate gate.

### Actual two-tile comparison

The optional `real_boundary_candidate_corpus` test consumes raw files only:

```sh
OFM_TEST_WEST=/path/to/west.pbf OFM_TEST_TILE=/path/to/east.pbf \
  cargo test -p rosenfu-core --test ofm_evidence \
  real_boundary_candidate_corpus -- --ignored --nocapture
```

Revision `20261101_080013_pt`, y6451, x14552/14553:
- west SHA `235914c31f14d0450a19da8c0f0f9c42ab98422378dae798bd2c0a3fa2932413`
- east SHA `48019e0b917793f6be41ed656de6fead84714945fedb2e69631aa8f1193cb873`
- 1,509,801 combined raw bytes; 5,890 total decoded roads; 1,965 candidate-class lines.
- 15 Rust candidate pairs match all 15 PR164 TypeScript pairs; largest observed
  metric difference 9.313225746154785e-10 m.

The small committed golden JSON contains only the 15 IDs/metrics from PR164
`findTileBoundaryContinuations` using the same two raw inputs and the frontend
MVT decoding/projection, filtered to the five candidate classes. Real PBF files
and large generated geometry are not committed. Attribution: © OpenMapTiles
© OpenStreetMap contributors, ODbL-1.0.

**Not implemented:** graph chain/branch/cycle selection, start/end pick handling,
ordered source spans, overlap blend regeneration, lightweight join validation,
physical curvature/grades, source-record validation, world mutation, or saving.
No candidate list is authorization or proof for construction. This is the bounded
candidate stage of a later independently reconstructed seam path validator.

## Pure derived path reconstruction, checkpoint 4

This supersedes the checkpoint-3 *path* omissions above, not its construction or
storage restrictions. `reconstruct_path(store, required_binding_keys, start, end)`
regenerates all candidates, selects a unique acyclic degree <=2 component, and
reconstructs ordered spans and `ofm_overlap_blend_v1` geometry. No claimed graph,
candidate, road subset, or geometry is input. Same-fragment requests reject in
favor of the unchanged existing single-slice workflow. Components exceeding 32
fragments, disconnected picks, branches, cycles, blend-overlapping picks/joins,
invalid fractions/indices, >256 covered source segments, or >256 output segments
reject. Covered source count is ceil(max(from,to))-floor(min(from,to)), summed
across spans, not only the derived output count.

Blend parameters are both endpoints, every 1/20, and all source vertex parameters
inside the 20 m band. Both polylines must intersect every parameter exactly once.
The geometric candidate gate retains its continuous bound; blend samples also
check <=0.8+1e-7 m separation. At t, w=t*t*(3-2*t), output=A+(B-A)*w, using exact
interpolated A/B endpoints at 0/1. Outside the band source vertices and exact pick
interpolations are retained. Reverse travel reconstructs reverse spans/joins; it
does not grant oneway traffic permission. Source raw geometry remains untouched.

`PathDescriptor` is a camelCase serde DTO matching the PR169 lightweight alignment
proposal: method/algorithm, geometryDerived=true, coverageVerified=false,
requiredBindingKeys, start/end, orderedSpans(bindingKey/fragmentId/from/to), and
joins(rule/aId/bId/axis/fromAxis/toAxis/aRange/bRange). Unknown JSON fields reject.
It has **no trusted status after Deserialize**. `validate_path_descriptor` reruns
reconstruction and compares every span and join. Identities, counts, ordering,
algorithm and flags are exact. Fraction-position tolerance is 1e-8; geographic
axis tolerance is 1e-10 degrees. All claimed numeric values must be finite. Required
binding-key order is irrelevant but duplicates or missing/extra keys reject.
Claimed derived points or floating JSON graph hashes are not accepted. Returned
`DerivedPath` contains newly reconstructed points and maximum sampled correction.

The committed real two-tile path golden was generated by frozen PR164 TypeScript
`buildTileRoadPath` from the same raw corpus as checkpoint 3. Pair 262:2 -> 184:1,
outer endpoint picks, yields 23 points. Rust regenerated spans/joins match, and
maximum point difference measured 2.842170943040401e-14 degrees (test threshold
1e-10 degrees). Optional test `real_derived_path_matches_typescript_golden` uses
the same OFM_TEST_WEST/OFM_TEST_TILE variables. Synthetic tests cover reverse path,
three-tile chains, continuity, invalid/inside-blend picks, forged metadata/ranges,
DTO round-trip revalidation, ambiguity and source segment/component budgets.

Still **not construction-ready**: actual track elevation/grade, curve radius,
speed policy, physical endpoint connection, source metadata/license provenance,
WorldDraft mutation, atomic file save, Undo, and frontend/IPC integration remain
separate required stages. The polyline blend is quantized derived reference
geometry, not a surveyed road or a physical-feasibility certificate. Neither the
DTO nor the returned path bypasses later physical validation.

## World and complete save envelope, checkpoint 5

Schema **25** stores the raw table outside the world payload, in the same bounded
atomic JSON document: `road_evidence {raw_blobs:[{sha256,byte_length,base64}],
bindings:[TileBinding]}`. Base64 is canonical and decoded only after count/length
preflight; SHA and all context are revalidated through EvidenceStore::from_parts.
A required envelope is written even for an empty store. Schema 24 and all earlier
versions are rejected before world parsing, without changing files or backups.
No automatic migration, deletion, external blob file, or additional save path is
introduced. The existing 16 MiB pretty-JSON cap includes the entire envelope.
The initial 16-binding / 8 MiB raw context limits remain in force.

WorldDraft contains runtime_road_evidence with serde(skip), plus the lightweight
road_evidence_descriptors manifest. Validation requires exact correspondence of
that manifest to the entire validated runtime store. Plain WorldDraft JSON is an
IPC projection, **not a complete backup**; a nonempty manifest deserialized without
evidence cannot enter World::new. Arc clones retain runtime evidence. PartialEq
has an Arc fast path and compares full contents (including raw bytes) on reload,
so independently decoded equivalent worlds do not become dirty by identity alone.

APIs:
- `draft.with_road_evidence(store)` explicitly constructs a new validated manifest
  and runtime table. It does not create alignment/path provenance or authorize build.
- `draft.restore_road_evidence(store)` injects runtime data without overwriting the
  serialized manifest, validates the entire candidate, and mutates only on success.
- `world.replace_retaining_road_evidence(candidate_draft)` requires an unchanged
  manifest, preserves the current store rather than trusting an IPC DTO, validates,
  and returns a new World. It never mutates the current world on failure.

Storage encode/decode, regular/versioned save/load and save-slot creation all use
this complete envelope. Export/import via encode/decode use the same format.
Existing manual/autosave session paths call the same storage entrypoints; no UI or
session change is part of this checkpoint. Their future replacement/build commands
must call the explicit preservation/attachment API. Registering evidence separately
before a build is not the intended transaction: future preview/build must validate
raw evidence and light descriptors inside the same candidate transaction, regenerate
backend geometry, and commit evidence plus actual alignments only after all checks.

Still absent: alignment-to-PathDescriptor provenance, physical path construction,
source license metadata binding, UI/IPC integration, and seam build/Undo acceptance.
The manifest reference checks here are table integrity, not physical track approval.

## World plan and separate path provenance, checkpoint 6

`ConstructionPath.ofmPath` is optional and mutually exclusive with legacy
`reference`, edited geometry, and redo points. Parked plans are now validated
against a freshly reconstructed descriptor: their points and line must match the
backend result (1e-10 degrees). The manifest, raw evidence, and lightweight plan
survive schema-25 save/load without putting raw blobs in IPC world JSON.

World APIs (all mutate only after complete candidate validation):
- `stage_ofm_construction_plans(plans, raw_additions, tile_bindings)` merges raw
  evidence with the current immutable store, regenerates every OFM plan's points
  and line, validates the whole world and commits once. Frontend preview geometry
  is replaced, never trusted. Plans' level arrays and other physical inputs must
  still satisfy their existing validation; incompatible arrays are not repaired.
- `build_alignment_from_road_path(owner, name, track_type, path_descriptor)` uses
  only the staged store and backend-generated points via existing typed build.
- `extend_alignment_from_road_path(owner, id, endpoint, path_descriptor)` uses
  existing endpoint/owner/tangent/length checks and preserves earlier provenance.
  For prepend, it independently reconstructs reversed provenance orientation.

Session must call stage and build on the **same outer candidate clone**, then run
its existing physical profile/design-speed/station/roads/saveable checks before
publishing that candidate. Parking alone is also a legitimate atomic operation;
there are no built assets yet. Build does not itself clear the parked plan. Preview
uses a clone. This checkpoint does not wire those session/IPC commands.

`AlignmentRoadPathProvenance` is separate from the old single-source type and
contains alignment/index range, PathDescriptor, retained whole-path arc fraction
interval, and explicit OFM attribution/license. Validation reconstructs original
path geometry from evidence and checks the current retained slice with the existing
source-lineage geometric tolerance (0.01 m + original length * 1e-6), including both
current and original interior vertices. Source snapshots are never rewritten.
Insertion/splitting and endpoint growth preserve range indices and retained
fractions. Generic replacement cannot attach/drop/change provenance on a surviving
alignment or change its points. Explicit removal drops that alignment's derived
records, not raw evidence; restoration/Undo can reuse immutable snapshots. Geometry
edits not covered by explicit range-preserving commands fail closed.

Plan plus built path records are capped at 32 per world, bounding repeated full
path validation. Retained raw archive and per-request limits are separate as
described above. `EvidenceStore::scope(keys)`
is a read-only projection of **already validated** full tiles, sharing raw
Arcs and decoding only the selected complete scope when needed. Missing/duplicate keys reject; individual roads cannot be filtered. This is
not a constructor for untrusted input. Each descriptor is reconstructed against its
recorded scope, so adding unrelated world tiles does not invalidate prior paths.
A caller can omit an external tile from its declared scope; this API cannot prove
acquisition completeness. The trusted frozen frontend acquisition set must match
the descriptor, and future source scope must not be presented as physical identity
certainty. A whole-tile scope retains all same-tile ambiguous candidates; omission
of required tiles rejects. Different scope can change the hypothesis set.

Serde JSON `float_roundtrip` is enabled in storage (and core test decoding) because
otherwise a generated coordinate can change one ULP on parsing, producing false
World inequality after reload. This is bit-preserving parsing, not a relaxed
geometric acceptance rule. Tests cover complete parked/built envelope round-trips,
reload equality, failed park/build candidates, generic-edit rejection, insertion /
split / append / prepend, deletion plus snapshot Undo, and unrelated tile additions.

**Still unintegrated:** session transaction/IPC/App, route-aware user deletion
workflows, UI Undo execution, and physical seam-build acceptance. The core entrypoints
preserve existing typed alignment/extension checks; session must still apply actual
height/grade/curvature/speed and endpoint-connection policy before live commit.

## Ordinary Bus graph checkpoint

`find_gameplay_bus_path` derives a simulator-only ground route from one already
validated z14 tile in an `EvidenceStore` scope. It does not install or enlarge a
native road catalog. Supported street classes with no explicit access tag are
usable by gameplay convention; denied and unrecognized values are excluded.
Bridge, tunnel, nonzero layer, buffered portions, guessed geometric intersections,
nearby endpoints and coincident interiors are not connected. The multiple-feature
junction is inferred only at a shared integer vertex with at most four occurrences
and at most one interior occurrence, with separate original lines. This does not
resolve missing structural tags or assert surveyed junctions or real bus access.

By default pass `respect_oneway=false` for bidirectional gameplay (known supplier
oneway values only). Explicit `true` enforces forward/reverse tags. The tile feed
does not carry full turn restrictions, barriers or public-service access semantics.

The result keeps the exact input vertices, ordered original fragment spans, source
revision/URL and raw SHA, plus a reproducible algorithm version. Use
`reconstruct_gameplay_bus_path` against `store.scope` when replaying a request.
This slice has no inter-tile seams, arbitrary interior-edge endpoint picks, route
editing/operation, world declaration or installed road data. Raw bytes in tests
are generated MVT; no imported tile corpus is bundled.

The next native checkpoint can commit one of these routes in an otherwise empty
road network from already retained evidence, run it using the existing bus runtime,
and restore it through the ordinary world save. It projects only traversed
raw fragments, not a second network, and distinguishes its fictional stop labels
and simulator-only eligibility from released native pilot data. Restoring a
current save regenerates the route against exact archived raw SHA and fixed-point
tile routing; only the latest validated derivation is memoized in the existing
archive cache. Old unmarked saves keep their bytes; older binaries do not read the
new marked world, and schema 26 explicitly refuses it. A fresh player-facing
cache admission and map flow, arbitrary stop positions/vias, broader coverage
and inter-tile continuity are not included in this native slice.

The cache-admission checkpoint accepts one paired, already fetched raw tile and
the matching ordinary road binding (or neither for a tile already in the
world). The same input/size/SHA/canonical URL checks as park-plan admission run
before publication. It makes neither a rail plan nor a catalog selection.
Native preview and commit share candidate evaluation; preview changes neither
the world nor its retained raw archive. The expected world token and atomic
save-size guard still apply. There is no separate loader or source UI.

### Bounded rectangular context, v2

`ofm_ground_bus_rectangle_z14_v2` adds the explicit end tile and up to 15
additional exact URL/SHA bindings. It verifies the complete rectangle of
adjacent same-revision z14 cells, admitting freshly fetched raw cache entries
atomically and checking retained tiles for missing/changed evidence. V1 still
works with neither field. The resolved geometry and fictional stops keep the
same preview/commit/save path and operate through the existing Bus runtime.

A road may cross an internal tile edge only between unique ground fragments
accepted by the existing reciprocal geometrical overlap verifier. The original
raw segments must additionally cross that boundary at the **same exact rational
grid coordinate**, which allows exact cuts without requiring a stored border
vertex. There is no nearest-neighbor gap bridge or corner jump. The boundary
verifier sees every possible crossing fragment, while lines lying wholly to
one side of every internal boundary do not consume its work budget. Restriction
and work limits fail closed; incomplete, ambiguous, unsupported and even small
mismatched seams can produce no path, not invented connections. Fixed-point
search and deterministic tie-breaking are independent of projected latitude.

This milestone's route remains limited to two selected raw vertices and a
single persisted ordinary route. It does not yet implement vias, arbitrary
interior stops, cross-tile geographic coverage guarantees, or shared-road Tram.

### Independent ordinary Bus routes

The native world stores ordinary route proofs indexed by route ID. Reconstruct
each proof against the same immutable tile archive, then materialize the union of
only traversed directed raw atoms. Exact tile URL/SHA, original line/segment and
owned interval determine identity; geometry proximity does not coalesce distinct
atoms. A shared directed atom has one runtime road resource. Different routes
retain independent fictional endpoint IDs even at the same point. Removing a
route prunes only unreferenced directed atoms and retains raw archives and
unrelated tracks/services for Undo and future reconstruction. Route preview
displays only the chosen ordered path, not the whole materialized union.

The marker is absent from previously published worlds and from existing pinned,
lab and QA routes. Those save formats are unchanged. The unpublished single-route
experimental world marker is intentionally not migrated; opening such a
prototype is non-destructively rejected. V1 one-tile queries and V2 multi-tile
queries are both supported as proof values in the indexed format.

Bounds: up to 64 proof rows, 256 total bound tile occurrences, 4,096 atoms per
route and 32,768 ordered atom occurrences across the derived union, subject also
to the existing tile and world-save byte budgets. A bounded proof memo retains
the hot cohort plus a candidate (up to 36,864 atom occurrences); first restore
rebuilds verified paths as required and warm world validations do not rescan the
whole geography. The existing provider cache remains the only raw source.

### Ordered authored via vertices

Optional `via` holds up to 30 stable, unique control IDs with exact raw
tile/feature/line/vertex pointers in the same source rectangle as the
endpoints. Via points constrain the chosen geometry; they are not passenger
stops. Search builds or reuses a compiled graph keyed only by complete immutable
scope and direction policy, then runs ordered directed searches with shared
scratch space. A multi-anchor route has a cumulative 1,000,000 relaxation and
4,096 atom limit. A failed leg prevents the full edit, and native preview can
return a zero-based failure index without any partial route geometry.

The default bidirectional open mode solves return via points in reverse order,
rather than reversing rendered points. With explicit strict one-way enabled,
outbound shape controls stay outbound-only and the return independently finds
a legal route between endpoints, avoiding a detour merely to revisit an
outbound control. Intermediate passenger stop editing and fractional picks
are follow-on, not part of this waypoint feature.
