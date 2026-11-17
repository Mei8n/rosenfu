# Map rendering proportions (alpha 0.2)

Reference inspected: circularsquare/trainsworld `6b8ef91729e1e5812f3dcbaedd6c0a49fed6a84b`, `trainworld/app/src/map/renderer.ts`, `lineBands.ts`, `stationGeometry.ts`. No source copied.

TrainsWorld renders zoom-dependent CSS-pixel widths: ordinary track 0.4/1.8/3 px at zoom 9/12/15; service line 0.85/3.9/6.6 multiplied by 1.3. Station dot radii also vary with zoom; platforms replace dots at zoom 14, retaining physical platform geometry. Device-pixel ratio is applied separately. Its values are references, not Rosenfu specifications.

Rosenfu previously used screen-space physical track strokes but fixed SVG-space 8-unit service bands and 10-unit casing. Resizing the viewport changed their relative apparent widths. Station dots and labels likewise inherited SVG scaling.

The independent Rosenfu policy keeps service color at 1.15 times the ordinary track width, shares a continuous bounded zoom response, and converts color-band offsets and station dot/label dimensions to viewport coordinates. Overlapping services share the same total band width rather than multiplying it. Physical platform footprints, connection topology, placement checks and saved data are unchanged. Platform rims retain screen-space thickness. The actual rendered appearance still needs desktop visual acceptance.
