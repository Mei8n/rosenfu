# Installed OFM seam routes: World admission and rollback

**Status:** Schema 36 now has a typed-only normal storage reader/writer
for the isolated, open, one-way route with both fragment locks. The reader
reattaches the bounded archive, rebuilds the projected graph through the
release-baked catalog, and recompiles saved operation for comparison. The
writer performs the same admission before touching the destination. Stock
releases still have an empty catalog, and the game UI cannot author this
route. Other capability mixes, missing trust, and mislabeled/damaged schema
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
3. The implemented isolated validator checks the exact entire graph and
   refuses preexisting roads/trams. To extend it, join the separately validated
   route sets; reject duplicate IDs across both. Recompute and validate all
   stored nodes, edges, stops and route occurrences on one candidate. Verify
   topology and permissions; persist a new version only after successful
   compilation, never eagerly materialize edges in the live world on error.
4. The implemented isolated run adds both fragments without altering any
   legacy run. The later mixed projector must add matching fragment locks
   for the same resolved archive
   context in both the witnessed route sections and every relevant legacy
   leg which uses either original fragment. The existing resource engine
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
forged locks. A second build restores the unmodified catalog and verifies
the same checkpoint is rejected without changing the file or creating a
backup. No Git ref, production catalog, default build target or network is
modified. Set `ROSENFU_SEAM_QA_TARGET_DIR` to a dedicated QA-only directory
if you want to reuse compilation outputs.
