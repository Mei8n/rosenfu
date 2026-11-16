# OpenFreeMap road runtime integration

The road runtime now selects bounded OpenFreeMap z14 transportation tiles on demand, not a self-hosted national pack. Existing national generator, packaging and native cache branches/artifacts are retained as research checkpoints; publication work is stopped. The existing OSM/GSI background is unchanged.

- Tile-derived identifiers are explicitly distinguished from OSM node/way identifiers.
- `tileSnapshots` retain each tile URL, content hash, provider revision and geometry separately. Viewport-dependent aggregate hashes are not persisted as one mutable road source.
- The source OSM timestamp is unknown and stays null. The provider revision identifies the delivered dataset, not an invented OSM update time.
- Coverage is unverified. Known surface crossings can be derived, but missing tiles, omitted roads and unsupported structure tags never prove road absence.
- Failed acquisition leaves ordinary construction available with explicit unverified-crossing feedback. Water remains an independent subsystem.
- OpenFreeMap, OpenMapTiles and OSM credits remain visible for derived road data, including after reference-layer hiding and save reload.
- Save schema 24 introduces explicit tile-derived provenance. Schemas 1–23 are rejected with `このセーブ形式は非対応です`; original files and backups are never deleted or converted silently.

## Incomplete

Cross-tile road-following continuity needs separately reviewed buffer-overlap matching. Matching coordinates or numerical feature IDs alone do not establish a connection; ambiguous cases must stop with a reason. Live WebView/CSP/usage-policy and graphical background overlay acceptance, current-head full CI and final independent review remain required. This checkpoint is not a claim that every physical road or crossing is represented.
