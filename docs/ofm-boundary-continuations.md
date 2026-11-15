# OFM boundary continuity candidates (not connected topology)

`findTileBoundaryContinuations` is an independent, bounded helper. It returns
candidate pairs only; it neither edits geometry nor creates physical connections,
copy provenance, roads, or crossings. Production adoption requires independent
review and an explicit multi-fragment provenance/geometry contract.

Only adjacent z14 tiles of the same revision are considered. Each individual
LineString must traverse the complete 20 m strip centered on their shared edge.
MultiLine components have separate IDs. Matching requires equal road class,
subclass, structure, layer, level and direction metadata; bridges, tunnels and
nonzero/unknown structures are excluded. Both clipped lines must have compatible
tangents and <=0.8 m sampled bidirectional separation and be mutually unique.
Corners, loops, boundary-parallel segments, point-only contact and ambiguous
parallel candidates are rejected. Nonzero one-way lines cannot reverse their
source digitization direction. All quantities are conservative game thresholds,
not claims of surveyed topology. Missing height tags remain missing; this does
not prove real-world surface level, legal access, completeness or physical identity.

Caps: 2,000 input lines, 100,000 input points, 512 points per line, 10,000
qualified comparisons, <=100 m clipped arc. Capacity exhaustion returns no
partial candidates and `truncated: true`. Invalid input also returns no pairs.
Line/edge clipping is cached. Geometry is never snapped, averaged or smoothed.
Fixed tolerances can reject valid roads. Boundary support is not complete.

## Read-only real-data study

OFM revision `20261101_080013_pt`, z14 tiles 14552/6451 and 14553/6451.
East tile SHA256 `48019e0b917793f6be41ed656de6fead84714945fedb2e69631aa8f1193cb873`.
Independent Python geometry analysis found 20 unique attribute-compatible pairs
across a +/-10 m strip, with maximum tile-to-tile separation 0.643 m. All 20 had
one nearby raw OSM geometry match within 2 m in the existing Tokyo source pack.
The source dates differ: this is empirical corroboration, NOT a stable-ID or
same-snapshot guarantee. Two pairs had different feature IDs between tiles.
The actual helper retained 15 surface-road pairs from 1,965 relevant input lines,
without truncation, after applying stricter class/direction/geometry conditions.
Bridge/motorway/trunk cases are not accepted by this helper.

Reference: https://mapbox.github.io/vector-tile-spec/ (buffer geometry may extend
past extent; no portable inter-tile topology guarantee). No third-party code is
copied. Raw tile and OSM study artifacts are not committed to this repository.
