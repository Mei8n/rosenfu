# Offline OSM XML reference-geometry converter

This developer CLI converts a **small regional OSM 0.6 XML snapshot** into the existing ReferenceGraph v1 plus a mandatory provenance sidecar. It does not download anything. It does not import into the game UI, modify saves, provide offline background maps, route trains, enforce access/oneway/turn restrictions, or implement construction snapping. PBF and compressed XML are not inputs.

## Run

Node **22.18 or newer** is required to load the shared TypeScript graph validator without duplicating it. Run `npm ci` with development dependencies present. The SAX parser is a pinned direct development dependency (`saxes` 6.0.0).

```sh
node scripts/convert-osm-graph.mjs \
  --input region.osm --output new-region-bundle \
  --id osaka-small --revision 2026-11-12 \
  --source https://example.org/actual-source \
  --source-date 2026-11-12 \
  --bounds 135.48,34.68,135.50,34.70
```

The URL above is a placeholder: provide the actual source URL and actual snapshot/source date. The CLI does not fetch or verify this URL. Source date is distinct from a locally chosen revision label. A lawful, complete-enough extract is the caller's responsibility. Public distribution of a derivative database needs ODbL compliance, including applicable attribution and database/source-offer requirements; this does not automatically license the game code under ODbL.

If the original data is PBF, preprocessing can be done separately with official Osmium tools:

```sh
osmium extract -b WEST,SOUTH,EAST,NORTH -s complete_ways source.osm.pbf -o region.osm.pbf
osmium cat region.osm.pbf -f osm -o region.osm
```

These commands are guidance, not operations performed by this CLI. `complete_ways` can include nodes beyond the bounds. Use an appropriately buffered extract and retain the original XML. No actual OSM dataset has been downloaded or validated as part of the synthetic test suite.

## Bundle and completion protocol

The output directory must not already exist, including as a symlink. All data is generated and validated before exclusive directory creation. Files use exclusive creation:

1. `graph.json`: existing graph v1, checked with `parseReferenceGraph`.
2. `source.json`: versioned provenance, original contributing-way node order/tags, referenced in-graph node tags, exclusions and limitations.
3. `complete.json`: written **last**, with SHA-256 hashes for both files.

Consumers must parse a valid completion marker and verify both hashes. Checking only whether `complete.json` exists is insufficient: interruption during its own write can leave a partial marker file. An interrupted or failed write can leave a directory without a valid marker; that is an incomplete bundle, not usable output. Existing directories are never reused, merged, recursively deleted or overwritten. This is a logical completion protocol, not a transactional filesystem or power-loss durability guarantee. No game bundle reader is added here.

The source sidecar records original input SHA-256, source date/URL, requested and XML-declared bounds, ODbL-1.0 and © OpenStreetMap contributors. It deliberately does not retain OSM contributor usernames, uid or changesets. Restriction relation contents are not preserved. Keep the original XML alongside your research records.

## Geometry policy

- IDs remain positive integer strings up to 20 digits, with separate node/way/relation namespaces. XML object order does not matter. Duplicate object IDs reject history/multiple versions.
- Original way node order and segment indices are preserved, including `oneway=-1`. The direction tag is retained, not enforced.
- Only original consecutive segments with **both endpoints within requested bounds** are emitted. Original indices are not renumbered. No intersections, boundary nodes, smoothed coordinates, merged nodes or invented edges are created.
- Missing referenced nodes on a selected way reject the conversion, including outside-bound nodes. Non-target ways are counted and excluded.
- Nodes unused by retained edges are absent from graph output. Contributing way sidecars retain their whole original node-reference sequence, which may refer to outside-bound nodes not present in graph output. Original XML is needed to recover those coordinates.
- Both-outside segments crossing the bounds are omitted. Boundary counts and `coverageGuaranteed: false` warn about incompleteness. Neither an extract nor repository `coverage: inside` proves full network coverage.
- Rail allowlist: rail, subway, light_rail, tram, narrow_gauge. Road allowlist: motorway/trunk/primary/secondary/tertiary and corresponding links, unclassified, residential, living_street, service. `area=yes` and explicit construction/proposed/abandoned/disused tags other than `no` exclude a way. Footways and other unsupported classifications are not normal roads.
- A way tagged for both allowed modes yields separate rail and road edges. A common node never creates a cross-mode interchange.
- Missing layer/level/bridge/tunnel remain unknown, not ground. Known bridge/tunnel values are normalized conservatively. Uninterpretable height tags reject conversion rather than silently merging distinct unknown heights. Multi-level strings are currently unsupported.
- Existing graph adjacency remains undirected and requires an explicit shared node, same mode and identical vertical metadata. Bridge transitions may therefore be disconnected. Same coordinates with distinct IDs and geometric crossings do not connect.
- Zero-length and shorter-than-1-cm emitted segments are unsupported and rejected, rather than dropped silently.

## Safety and limits

Input uses one regular-file descriptor, nonblocking open, fstat, and bounded reads even if a file grows. UTF-8 decoding is fatal. Only XML 1.0 OSM 0.6 snapshots are accepted. DTD, declared entities, external resources, namespaces/XInclude, CDATA, non-declaration processing instructions, deleted/change objects and unsupported nesting reject the input. XML predefined and numeric character references remain valid XML. The SAX parser has no configured fetcher/resolver or added entities. XML comments are harmless and ignored; notes are bounded and ignored.

Hard limits reject the whole conversion, not truncate it:

- XML: 50 MiB; graph JSON: 5 MiB; sidecar JSON: 20 MiB, all measured in bytes.
- Graph: 20,000 nodes, 40,000 edges.
- XML: 250,000 objects; 500,000 combined way references/relation members; 500,000 tags; 256 tags/object; 32 attributes/element; 4,096 characters/attribute; depth 3.
- All input coordinates must fit the existing supported latitude range ±85.05112878°. Bounds cannot cross the antimeridian or exceed 180° longitude width.

This is bounded in-memory processing, not a planet-scale streaming importer. SAX attribute callbacks occur after attribute parsing, so input-byte bounds remain the ultimate limit on an individual buffered token. For larger areas, split source regions; do not raise limits without profiling and review.

## Tests

```sh
npx vitest run scripts/convert-osm-graph.test.mjs
npm test
npm run build
```

Fixtures are synthetic. Successful conversion tests do not prove real-data coverage, legal provenance, routing correctness, UI integration or PBF support.
