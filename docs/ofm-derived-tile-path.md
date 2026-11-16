# Derived OFM multi-tile path, version 1 (not wired to construction)

Pure entrypoint: `buildTileRoadPath(lines, start, end)`. Input lines are decoded
immutable source line strings; picks are fragment ID, segment index and fraction.
No caller-supplied seam evidence is trusted: boundary candidates are recomputed.
The helper returns derived longitude/latitude vertices, ordered raw-source spans,
and explicit join records. It does not mutate inputs, create provenance records,
assign source hashes, save worlds, or construct rail. A future backend must bind
all spans to authenticated per-tile source records and independently revalidate.

## Selection

Only one connected acyclic degree <=2 component is usable. Candidate uniqueness
is evaluated per neighboring tile pair (a middle fragment can meet two distinct
tile boundaries). No branch choice or shortest-road search occurs. All input
revisions must match. Duplicate IDs, disconnected endpoints, ambiguous seams,
cycles and repeated boundary crossings fail. Maximum 64 tiles, 32 component
fragments, 256 total covered source segments and 256 output segments. Single
fragment copies retain the existing unmodified single-slice implementation.

## Reproducible join algorithm: `ofm_overlap_blend_v1`

For an adjacent tile edge, use longitude (vertical edge) or latitude (horizontal
edge) as the parameter. Convert 10 metres to degrees using 111320 m/degree
latitude and longitude scale 111320*cos(midlatitude); midlatitude is the mean
north/south bound of the tile pair. Travel direction determines which end of
the 20 metre band comes first. Each source must cross each band parameter once.

Sampling parameters are 0, 1, every 1/20, and all source vertex parameters inside
the band, sorted. At each parameter interpolate independently along each source
polyline. Their separation must stay <=0.8 m. With t from 0 to 1, let
w=t*t*(3-2*t); emitted point = A*(1-w)+B*w. At t=0 use the exact interpolated A
point; at t=1 use the exact interpolated B point. Outside the band retain original
vertices and exact pick interpolations. Joins cannot overlap or consume a pick.

This is deliberate derived geometry, NOT an exact source copy. It avoids a
sub-metre perpendicular connector. The continuous blend has zero weight derivative
at both ends, preserving source tangent where the original polyline is smooth;
the returned finite polyline remains an approximation and its curvature must be
checked by the construction backend. Source bends outside the band are unchanged.
The 0.8 m bound is checked at the source vertices and regular samples, not a proof
of true geographic identity or sub-sample curvature. Do not label it surveyed or
physically certain. The function emits `geometryDerived: true` and raw coordinate,
source attribute, fraction and algorithm records so this correction is visible.

## Height, provenance and downstream requirements

Only conservative surface candidates are eligible; bridge/tunnel/nonzero level
pairs are rejected by the candidate helper. Original class/access/layer/level and
structure metadata is retained without inferring missing values. This helper does
not assign track elevation or alter heights. Actual anchor heights, longitudinal
grade, curve radius/speed and reference/route consistency must be recomputed on the
resulting path before construction. Failure must commit no track or partial source.

Proposed future provenance method `ofm_tile_path_v1` must retain each source_key,
fragment and ordered span, plus join records and generation version. Existing
`ofm_tile_slice_v1` cannot represent this correction. Source hashes and source keys
are caller/backend responsibilities, never invented here. Reverse travel reverses
span order/fractions and regenerates the joins in reverse. Save/load, Undo/Redo,
physical connection checks and UI correction disclosure remain unimplemented.
