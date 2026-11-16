# Isolated OFM reference draft adapter

This is NOT connected to App, referenceCopy, construction, IPC, world storage or
backend validation. `prepareOfmReferenceDraft` produces a self-contained JSON
candidate payload for a future `ofm_tile_path_v1` method, not an exact copy.
Current provider snapshots do not retain raw PBF bytes: a future acquisition
adapter must retain them; a source SHA alone is insufficient.

Inputs: all full tile snapshots in an explicitly required tile-coordinate set,
the immutable raw PBF bytes for each tile, and two fragment/segment/fraction
picks. Required tile set and received set must agree exactly. Caller must compute
the required selection/search scope; the list is not proof of complete global
road topology. Source URLs are canonical fixed-revision z14 OpenFreeMap URLs.
Mixed revisions, duplicates, missing evidence, invalid metadata/geometry,
hash mismatch and oversized inputs fail. Inputs are detached before async hashes.
Raw access/oneway/classification/geometry remains in the full candidate context,
including roads that are ineligible for following. Only the documented follow
classes are passed to the path candidate helper; omitted classes remain evidence.

Output includes:
- `format=rosenfu-ofm-reference-draft`, version 1, method `ofm_tile_path_v1`
- `geometryDerived=true`, `coverageVerified=false`, decoder rule identifier
- exact required tile set and original picks
- per-tile SourceRecords (proposed conversion version 4), fixed source URL,
  revision, raw PBF SHA, graph SHA, ODbL and supplier attribution
- candidateContext: raw PBF base64/byte length, exact graph JSON, full decoded
  tile lines and source binding (not just the chosen fragments)
- derived path, raw ordered spans with source keys, and explicit join metadata

## Backend validation contract (not implemented)

Before accepting a build, decode each raw PBF using the declared, independently
implemented rule after verifying byte limits and source SHA. Recompute the full
graph and all tileLines, verify graph SHA and exact decoded-context equivalence.
Recompute all candidates, route spans, joins and output geometry from the complete
context and original picks. Verify source-key binding and fixed revision. Hashes
prove content identity, not authenticity. Source-hash checking without redecoding
cannot detect a maliciously omitted competing fragment. This frontend checks
full graph-node/edge membership against tileLines, but that is not a substitute
for raw PBF redecoding. It deliberately does not claim otherwise.

Then check actual construction elevation, grade, curvature/speed, route/station
references, resource limits and atomic save/undo semantics. Attach no partial
road or provenance on failure. This payload is not yet a supported World schema.

## Resource experiment

Caps: 16 tiles, 5 MiB per raw tile, 8 MiB total raw bytes, 100,000 full-context
points, 16 MiB snapshot JSON and final JSON. Derived helper limits also apply.
These are refusal limits, not nationwide coverage guarantees.

Actual adjacent Tokyo z14 14552/6451 + 14553/6451, revision
20261101_080013_pt: raw bytes 1,509,801; decoded snapshot JSON 12,496,049;
5,890 complete line fragments / 20,335 points. The adapter successfully emitted
11,269,265 JSON bytes for one cross-boundary route, retaining all tile context.
Thus two dense real tiles fit, with limited room for more; 16 tiles is not a
promise of success. Larger selections must explain the limit, not truncate.
The real measurement is local-only and not a backend/GUI validation result.

Unit fixtures use synthetic opaque bytes to test hashing and contract guards;
they do not pretend to validate PBF decoding. Actual decode was tested separately
with the current provider and retained study artifacts outside the repository.

## Content-addressed reuse (mandatory before World integration)

Raw and decoded evidence must be stored once per source-key in a shared evidence
table, not copied into each alignment. Output separates requiredSourceKeys and
candidateContextKeys from newSources/newEvidence. Optional reusableEvidenceKeys
are reuse hints; the backend MUST resolve every key in its validated evidence
table or reject, never accept a caller's claim that an absent key exists. The
frontend still verifies full inputs and computes geometry before omitting reused
evidence. After load, source-key resolution must retain full candidate context.

The existing 16 MiB World limit means even the measured first 11.27 MB evidence
set is expensive. Reuse avoids multiplicative growth but does not solve initial
capacity or nationwide persistence. Storage compression/external content-addressed
blobs need a separate atomicity and lifecycle design. No World API is connected.
