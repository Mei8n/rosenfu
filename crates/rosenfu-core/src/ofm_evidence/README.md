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

The **initial whole-store capacity is deliberately the same as one request**:
16 bindings, 8 MiB counting reused raw once per binding, 5 MiB per tile. Repeated
requests cannot silently accumulate past the limit. This is a conservative initial
API, not a finished nationwide-world storage policy. A larger immutable table
requires a separately reviewed distinction between store budgets and per-path
context budgets. Empty stores are valid but attest to no data. No garbage
collection, WorldDraft mutation, or save-envelope encoding is implemented here.

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
