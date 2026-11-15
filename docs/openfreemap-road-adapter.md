# OpenFreeMap road tile adapter

Independent implementation using BSD-licensed @mapbox/vector-tile and pbf. No TrainsWorld implementation copied. TrainsWorld roadTiles.ts was consulted for behavior: its crossingWeight intentionally excludes service/track/path; this is not evidence those classes are absent from OpenFreeMap.

## Evidence (2026-11-16)

Existing real z14 tile 14552/6451, revision 20261101_080013_pt: 815 transportation LineString/MultiLineString features including service117, minor77, path225. Native source PBF road samples (2026-11-14) compared with tile geometry at <=5m intervals: service way144970076 (driveway) <=0.181m, residential41818367 <=0.086m. Bridge23297417 retains brunnel=bridge/layer4; tunnel23704461 retains brunnel=tunnel/layer-1. These examples support use, not nationwide completeness or same-date validation. Nearest-geometry comparisons at road intersections do not uniquely identify OSM ways. Tile IDs are NOT interpreted as OSM IDs.

This module retains observed motor-road classes including service and track. Paths/steps/rail are excluded by explicit application policy, not reported missing from the provider. Minor class is mapped to residential for existing road-follow classification; the original geometry/provenance is tile-scoped. Bridge/tunnel/layer are not interpreted as metres. Unsupported structural values stay unsupported. Default absent brunnel uses the OpenMapTiles surface convention, not an OSM ground survey guarantee.

## Contracts and limits

load(bounds) returns combined reference geometry plus tileSnapshots for stable per-tile save identity. sourceDate=null because source OSM timestamp is unknown; dataset revision is the actual tile URL revision. Tile geometry buffers are retained; graph envelope is distinct from coverage bounds. MultiLine components have distinct fragment IDs. No geometric join is inferred at coincident crossings or tile seams.

IDs: ofm:<revision>:14:<x>:<y>:<featureIndex>:<lineIndex> with optional :<pointIndex> for nodes. Existing numeric OSM IDs remain valid only under their original contracts. Copy/save provenance and cross-tile follow need dedicated support in the integrating change.

Fixed HTTPS TileJSON/provider, max16 tiles/request, 5MiB/response, 50MiB/3000 tiles/session, sequential tile fetch, bounded64tile cache, coalesced requests, 20s timeout,60s failure cooldown. Non200 and missing transportation layer remain errors, not empty road coverage. Successful tiles always coverageVerified=false. Attribution includes OpenMapTiles and OSM.

## Terms / integration limits

https://openfreemap.org/ states public map display free, commercial usage allowed, no SLA. https://openfreemap.org/tos/ (2026-10-14) limits offline downloads to 3000tiles or50MB, <=4parallel and app-identifying User-Agent, while prohibiting other automated data collection. Runtime quotas alone are not legal assurance that a particular extraction use is permitted. Browser CORS headers on measured tile and TileJSON were Access-Control-Allow-Origin: *. WebView app User-Agent and permitted usage must be confirmed by integrating owner before production enablement. No paid service or release was created. Existing OSM basemap is unchanged; overlay alignment still requires real GUI inspection.
