# Inspection diagnostics, not construction assessment

Call `diagnoseWaterEvidence(readerValidatedComposite)` after a successful strict
reader or delivery-loader result. It returns source-part counts and source claims
for display. It does not fetch data, produce polygons, estimate width/elevation,
intersect a route, deduplicate features, change coverage or assess construction.
The result is always selected-query-window / incomplete / coverageCertified=false
/ constructionAssessment=unavailable. It has no EnvironmentSnapshot output.

Transport/hash/schema failure must remain an unavailable request in the caller.
Do not fabricate an empty composite and show it as a successful zero observation
result. An actually verified empty part still cannot certify dry land.

Counts are per-source observation records. A MultiPolygon is one water-area
record; separate rendered polygons can be more numerous. Regions can repeat the
same OSM object, so adding part counts does not count unique rivers or crossings.
Waterway-line records are line geometry selected by the source rules; they do not
establish a surface footprint. Absence or whitespace-only width tags are counted
as missing. Every other width string, including numbers or 'unknown', remains an
unverified source claim. No parsing, width buffer or six-metre height assumption
occurs. Tunnel/covered/intermittent/layer strings (including 'no') are retained
as tag claims and must not be rendered as verified physical classifications.

Source issues are copied per part. Their locations are unspecified: a regional
relation error retained with a query does not mean the missing geometry lies
within that query window. Duplicate issues across parts are not a unique missing
feature count. Zero reported issues still leaves unassembled relations without
callbacks and unmapped water unresolved. Width tags and issue records are
untrusted text; render as text rather than HTML.

Version differences are recomputed from per-part revision strings. Equal strings
are not a proof of common observation date or completeness. The app must retain
source attribution/conditions from the original parts, identify the selected
trial windows, and avoid presenting diagnostics as nationwide coverage.

The currently public trial is Fuji, Kanmon and Takamatsu only. The national query
builder can read all eight regional stages for selected windows; that capability
is distinct from nationwide hosted cells or complete real-world water coverage.
App/hook/CSP wiring remains owned by the integration task. This diagnostic module
does not change useAutomaticEnvironment, ready status, water_coverage or build
permission.
