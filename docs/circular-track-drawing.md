# Circular manual track drawing

The TrainsWorld screenshots supplied by the user on 2026-11-12 show a whole-circle guide behind the currently previewed curve. This implementation follows that visible interaction, not an asserted reconstruction of TrainsWorld's private algorithm.

- The first two anchors establish a straight segment and heading.
- Each following anchor defines a circle tangent to the previous segment. Earlier segments do not move when another point is added.
- The same sampled polyline is previewed and passed to construction/storage. Undo and redo reconstruct identical geometry from the anchor prefix.
- A thin full circle is shown for the current curved segment; straight segments have no circle.
- Geometry uses locally scaled Web Mercator, so the on-screen guide is circular. Radius in metres is an approximation at the first anchor's latitude, not engineering-grade geodesic surveying.
- Prototype minimum radius is 25m, matching the supplied warning screenshot. Backward points, duplicate points and overlarge sample counts are rejected. No vehicle-specific speed calculation is claimed.
- Chord deviation is bounded to 0.5m in the local projection, with at most 256 samples per arc and 4096 points overall. Excessive arcs must be split rather than silently simplified.
- Existing saved geometry and the reference-data copy tool are unchanged. This is only manual drafting.

Native Windows rendering and interactive TrainsWorld equivalence require separate visual verification; the automated tests cover circle shape, prefix stability, left/right curves, invalid input, undo/redo, and preview/payload equality.
