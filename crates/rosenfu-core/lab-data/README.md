# Debug road gameplay labs

This opt-in development-only data is **not** a real route, a real stop, an
assertion of actual traffic permission, or coverage of the surrounding region.
The default build does not include it. A release build rejects this feature.

Run a debug source build with `npm run tauri -- dev --features dev-road-lab`.
In a temporary new world choose 建設 → 道路交通 → 島田・大津の検証用データ.
Select the direction, then place two game bus stops, create a route and configure
its timetable. Do not use a valuable save: a world using this lab catalog cannot
be opened without the same opt-in digest. The normal release registry is unchanged.

## Data and license — separate from application code

Copyright OpenStreetMap contributors: https://www.openstreetmap.org/copyright .
The machine-readable derived database in this directory,
`shimada-otsu-lab-bus.json`, is available under ODbL 1.0:
https://opendatacommons.org/licenses/odbl/1-0/ .
It may be downloaded separately from the same repository and is not licensed
under the software license. Source distribution: Geofabrik Chūbu.

- Dated source: https://download.geofabrik.de/asia/japan/chubu/chubu-261120.osm.pbf
- Replication timestamp: `2026-11-20T21:21:21Z`; sequence `3494`.
- Parent SHA-256: `5d2d71ee6cc485cfc16324de2527fdec9cdafd60a4cd21369de83d5cbed234b5`.
- Derived asset SHA-256: `20de064b03e7c22eb92579e6d53bde0d0b97872c9f7a2ba4346df1ebbdd05c92`.

Selection and transformation: from the ordered nodes of OSM way `392108440`
version 1, retain zero-based segments 1–15 in their original forward direction.
Use the exact 16 source vertices (no snapping or simplification); each was
checked untagged and incident solely to that way in the supplied parent. Write
one directed bus edge per retained segment and one same-way transition between
each consecutive pair for the game. Do not emit reverse edges, branch turns,
bus stop references, tram authority, or contributor/edit metadata. Canonically
sort the output rows by their identifiers as required by the native registry.
The finite snapshot is not proof of any present-day road restriction or field
condition. The approx. 228 m example uses a game stop 25% into the first edge
and another 75% into the final edge.

## Dejima single-segment tram test
`dejima-se-tram-lab.json`, SHA-256
`810bb68473a4f1b7b55c828ddd94b2c8f030b1395894032bce1ce479d66104c2`,
is one southeast-directed game-only segment clipped from OSM way 128993698 v12:
https://www.openstreetmap.org/way/128993698/history/12 .
Like the Shimada file, it is separately licensed under ODbL 1.0, not the software
license; attribution: © OpenStreetMap contributors. In a fresh temporary world
select the Dejima entry, then select the two endpoints from northwest to
southeast and build a shared-road tram segment.
It does not establish actual vehicle access, railway rights, street coverage, or
grade; `ground` is a simulation category here, not a surveyed gradient. The
catalog grants no bus access, no reviewed or player-created boarding positions,
no turns, and no reverse traversal for this region. Keep the existing lab notice.
