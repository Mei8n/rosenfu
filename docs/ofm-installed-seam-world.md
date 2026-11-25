# Installed OFM seam routes: World admission and rollback

**Status:** Schema 36 now has a typed-only normal storage reader/writer
for the open, one-way route with both fragment locks. One ordinary
single-tile route may coexist if it uses the exact same certified archive.
The old route keeps its existing edge IDs, length and wire bindings and gains
only matching fragment reservations while this capability is active. The reader
reattaches the bounded archive, rebuilds the projected graph through the
release-baked catalog, and recompiles saved operation for comparison. The
writer performs the same admission before touching the destination. Stock
releases still have an empty catalog, so the gated UI is hidden. Other
capability mixes, missing trust, and mislabeled/damaged schema
36 remain unavailable, not repairable through ordinary overwrite.

## Smallest positive world format

Start with exactly one authored **open, one-way, two-stop** route following
one installed two-cell witness. Do not allow automatic dispatch on merely
nearby geometry, reverse travel, vias, loops, arbitrary fractional stops, or
shared street trams in this first version. Copy neither producer tile bytes
nor the provider's mutable endpoint into source or an allow-list fallback.
Only separately verified raw evidence is admitted to the existing bounded
archive.

The new capability is a distinct world field, not an optional subfield of a
legacy `BusPathBinding`:

```json
{
  "format": "rosenfu-world",
  "schema_version": 36,
  "world": {
    "ofm_installed_seam_bus_routes": {
      "route-id": {"version": 1, "catalogKey": "<installed recipe reference>"}
    }
  }
}
```

The snippet shows only new fields. Existing envelope and authoritative
route metadata (stable route ID, owner, display name), road evidence table,
and complete graph/stop validation still apply. Schema 36 must be emitted
only for a world with an admitted nonempty seam map. Legacy binds continue
under their exact grammar and existing schema selection when that map is
absent. On first implementation admit only classic ordinary bus operations
and reserve all unsupported capability mixes, including trams, standard
rail, parallel/refuge rail and fractional bus; do not widen this guard until
there are passing combined profile/ownership tests.

## WorldEditor transaction

1. Validate the operation token, company ownership, existing route ID, and
   immutable evidence descriptors before changing the live world. On an
   isolated candidate attach the two release-approved verified tile blobs
   through the existing evidence-store admission; never accept renamed raw
   hashes, client coordinates or trusted digest from the save.
2. Resolve the recipe through the installed release catalog. Use the native
   replay's two owned source atoms and shared raw vertex to derive three
   nodes, two directed edges, and terminal-only fictional stops. Edge IDs
   need a new namespace and must include the source context and raw ordinal;
   they are not aliases of formerly split legacy edges. Geometry and source
   metadata are derived, not trusted from stored road transport. No old
   stops, edge IDs or reservations are remapped.
3. The validator joins the separately rebuilt route graphs and compares the
   exact complete graph. It permits one compatible classic legacy v1 route
   on either certified tile; duplicate IDs, other road modes, raw hashes
   and archive revisions are refused. Recompute and validate all
   stored nodes, edges, stops and route occurrences on one candidate. Verify
   topology and permissions; persist a new version only after successful
   compilation, never eagerly materialize edges in the live world on error.
4. Mixed compilation adds matching fragment locks for the same resolved
   archive context to the witnessed route and the affected legacy route.
   For now the legacy route reserves its matching fragments on every leg,
   which is conservative and avoids inferring a split-edge alias. The existing resource engine
   reserves these for the whole leg. Keep legacy physical edge keys and
   crossing protections. Until geometric compatibility can be proved across
   revisions and other road modes, reject the relevant mixed world rather
   than infer a resource identity or omit a lock. This first slice may
   reject inserting/removing a witnessed route while operation is active;
   do not migrate active occupation from legacy to new resource sets.
5. On save restore re-admit outer schema and recipe, rebuild geometry and
   the full resource-bearing service specs from external installed evidence,
   compare against saved projections/specs, then restore operation. Refuse
   checkpoints with mismatched lock sets or shared occupancy. Validate
   renderer handling separately; no invisible or inferred movement from
   an ordinary-bus fast path that currently checks only legacy bindings.

## Rollback and promotion gates

- Failed preview/admission/reopen must leave the original World, route
  tables, runtime, generation, save, and backup unchanged. A missing
  installed entry is unavailable capability, not corrupt data to replace.
- Keep previously admitted catalogs append-only for releases that promise
  reopening. Removing an entry cannot substitute another route at the
  same key; releasing compiled default code with an empty catalog must
  refuse capability-specific checkpoints without modifying them.
- Before ordinary writer rollout: synthetic and archive-sourced two-cell
  positive; false-direction and ambiguous geometry refusal; overlap with
  a live legacy route; independent archive re-admission; mid-leg
  save/reopen with identical dispatch continuation; downgraded/corrupted
  save refusal; byte-identical legacy roundtrip. Actual data is fetched
  and verified only in the test environment.

## Reproducible storage QA

From committed sources, run `python scripts/check-installed-seam-world.py`
(Python 3.11+, Git, Cargo and already cached Rust dependencies). It archives
the current commit to a temporary directory, generates a two-tile invented
road and a catalog entry for a synthetic revision, and runs the real storage
writer/reader including operation wait, resumption, backup and refusal of
forged locks. A separate process starts with an empty user profile and
cache, reopens the normally written checkpoint, and verifies both buses
continue with the preserved wait. Pass `--isolate-network` to require a
Linux user/network namespace with no route during the cold start. This
checks the saved road/dispatch state, not availability of offline basemap
imagery or the desktop WebView. A second build restores the unmodified
catalog and verifies
the same checkpoint is rejected without changing the file or creating a
backup. No Git ref, production catalog, default build target or network is
modified. Set `ROSENFU_SEAM_QA_TARGET_DIR` to a dedicated QA-only directory
if you want to reuse compilation outputs.

## Desktop authoring (installed receipts only)

In the normal **バス** construction panel, **確認済みの地図境界横断バス**
appears only when the installed release includes a matching receipt and both
exact raw tile bindings are already retained by the current world. Native
discovery replays the candidate against that world and shows its start
coordinate and one-way distance. The player names and explicitly creates it;
the editor rechecks the displayed world token, ownership, current routes,
edit bounds, trusted proof and normal saveability before committing one
construction edit. No coordinates, map bytes or digest authority come from
the authoring request. The created route is selectable and can use the usual
bus operation panel. Unverified geometry rewrites are disabled while this
restricted world profile is active; an undo for this edit is available while it remains in the current edit history.
Stock releases have no installed receipts, so the new controls stay hidden.


When an installed receipt names exact tiles not yet retained, the bus panel
can offer a separate **確認済み区間の道路を取得** action. It requests the
registered OpenFreeMap revision and coordinates directly, without depending
on current map metadata. It rejects redirects and checks the exact SHA-256:
newer bytes returned for an older alias are not substituted. This path does
not register new tiles in the ordinary road observation cache. Native code
derives all missing bindings from the installed catalog reference, refuses
re-binding an existing tile, and replays the complete route before retaining
both tiles. Missing, changed or retired bytes therefore leave the world
unchanged. No request is made in an empty-catalog build.

## Preparing an additional release candidate offline

This does not install trust or add a route to a running game. A reviewer must
first identify a supported official producer edition, independently extract
both original z14 payloads from its immutable release archive, and review the
original OSM way and producer configuration. In particular the edition must
have original way IDs enabled with the reviewed Planetiler/OpenMapTiles
one-way identity and numeric mapping. A dated tile URL can answer with today's
bytes and is not evidence of the edition. This tool does not download tiles,
authenticate the source by itself, or grant real road access.

Privately prepare:

- An approval JSON object with `format: "rosenfu-seam-archive-approval-v1"`,
  `producer_id_rule: "planetiler_openmaptiles_osm_way_10_plus_2_v1"`,
  `tiles_west_east` (two full `TileBinding` objects, in geographic order),
  `osm_way_id`, `osm_way_version`, and `reviewed_way_sha256`. Obtain the
  SHA-256 of the exact approved file from the independent reviewer, not from
  the same unreviewed payload as the inputs.
- `reviewed-way.json`: the reviewer's normalized snapshot of that original
  source way, with positive numeric `id` and `version`, ordered `points`
  as `[longitude,latitude]`, and string-valued `tags`. Review the snapshot
  against the original way at the same revision context before pinning it.
- The two exact archive-extracted MVT payloads, uncompressed. Do not use
  live dated aliases as a substitute for independent extraction.

Run the local executable with:

```text
rosenfu-cli world prepare-seam-catalog approved.json <reviewer-approved-sha256> west.pbf east.pbf reviewed-way.json <new-private-candidate.json>
```

It verifies the input hashes, checks the reviewed source way's direction,
class and boundary crossing against the tiles, derives unambiguous raw
vertex ordinals, and replays the normal native proof (including exact
shared raw geometry). On success it creates a new one-receipt manifest
candidate only, never overwrites a file, and does not update the built-in
catalog. Keep real results and raw inputs out of source control. Promoting a
reviewed receipt to the compile-time product catalog, licensing/provenance
review, and supported official archive access remain separate release steps.
