# Route candidate preview API

Current note (2026-11-13): schema 4 below records the introduction of RoutePath, not the current save version (7). Alignment-based ServicePath creation now exists; see [service path design](service-path-design.md) for its original design and [operating-service model](operating-services.md) for the operating-service boundary. This API is a bounded construction helper, not passenger routing; see [research integration](trainsworld-findings-integration-audit.md).

Candidate search and preview remain read-only. Explicit saving now persists physical paths and station membership in schema 4, without creating tracks or services. Existing Route and OperatingService data remain independent.

## Desktop commands

- `preview_route_candidates({ start, end })` returns `{ status, candidates, truncated }`.
- `preview_route_membership({ start, segments, excludedStations })` returns `{ path, member_stations, excluded_stations }` after validating the chosen path against the current world.

IDs are the existing canonical decimal strings. Output field names are snake_case. Candidate fields are `stations`, `segments`, and `track_only`. `stations` includes all stations in traversal order. Segment objects are either `{ kind: "track", track_id }` or `{ kind: "alignment", alignment_id, from, to }`.

`status` is `no_path`, `unique`, `ambiguous`, or `incomplete`. Never silently select a candidate when ambiguous. An incomplete result may contain zero or one candidates but does not prove disconnection or uniqueness. Search is deterministic by infrastructure ID, not a shortest or fastest recommendation. Limits are 32 candidates, 100,000 traversal steps, and 256 segments. Only station-simple paths are enumerated; circular routes and repeated station visits require explicit editing, not this helper.

Track connections are undirected, matching the current prototype. On one Alignment, adjacent placed stations form virtual candidate segments. Geometric crossings, nearby coordinates, or overlapping alignments do not create connections. Coincident station placements have no inferred ordering or connection. These are infrastructure path candidates, not a simulation of switches, directional tracks, contracts, access permission, or capacity.

`save_planned_route({ routeId, name, start, segments, excludedStations })` validates and commits an explicitly selected path. A new route allocates one RouteId; editing retains that ID. A `RoutePath` sidecar (`route_id`, `segments`, `member_stations`) stores the full physical path separately from its member stations. The corresponding Route has an empty `tracks` list. Legacy routes keep their Track lists without a sidecar. Dual physical sources, missing routes, duplicate sidecars and invalid membership are rejected atomically. The old `save_route` command refuses to overwrite sidecar routes.

Exclusion is route-membership selection, not station demolition or train stopping policy. Only distinct intermediate stations may be excluded; endpoints remain members in this initial API. The full physical path is preserved. Unknown, disconnected, cyclic and excessive paths are rejected before returning a plan. Alignment spans are resolved by placed-station positions rather than adjacency. Adding a station inside a saved span does not invalidate it and does not silently add the new station to its persisted membership. Removing a member or span endpoint is refused; an excluded interior station can be removed without changing the physical span.

## Tests

`cargo test -p rosenfu-core --test route_planning` covers unique/reversed traversal, intermediate membership exclusions, branch/parallel ambiguity, bounded search, invalid/stale inputs, virtual Alignment traversal, and non-connection at geometric crossings. Core/storage tests and clippy verify data compatibility. The desktop session logic is also tested in a standalone harness; full native desktop compilation requires the platform dependencies. The route tab includes candidate/membership preview and explicit new-route saving; see below.

## UI integration boundary

The candidate panel must require explicit candidate selection for ambiguous/incomplete results, explicit route naming and save intent. Browsing must not overwrite an existing route draft. The persistence API does not choose a preferred path. UI integration is maintained separately from the backend; native WebView rendering is not verified in the cloud environment.

## Save compatibility

Schema 4 requires `world.route_paths` (an array, empty for legacy-only worlds). Versions 1–3 migrate in memory to an empty sidecar list; loading does not rewrite files. Version 1 retains its frozen legacy decoder. Versions 2–3 use a strict legacy top-level world shape, rejecting injected newer fields. Old versions cannot express exclusions or Alignment route paths. Save backup/atomic replacement behavior is unchanged. Older applications reject schema 4 rather than silently dropping route metadata.

New persisted paths are station-simple and limited to 256 segments, just like preview paths. Existing legacy routes may retain repeated visits. At schema 4 introduction, services were Track-based only. The current model also supports independent ServicePath data; persisting an Alignment route still does not create a service or operate trains automatically.

Additional tests: `route_paths` core/storage suites cover ordered membership, invalid references, reverse/mixed paths, insertion stability, migrations, backups and atomic rejection. Session tests cover legacy-overwrite refusal and deletion guards.
