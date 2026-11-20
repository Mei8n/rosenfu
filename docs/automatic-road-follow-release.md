# Automatic road following: bounded initial release

> Historical release scope (PR #149), not the nationwide target architecture.
> The current decision is on-demand OpenFreeMap vector tiles; custom national
> pack generation is no longer the primary implementation path. The Tokyo pack
> described below is still the integrated baseline at dev `528be958`. See
> [implementation status](implementation-plan.md) and [alpha 0.2 road transport](road-transport-plan.md).

This release automatically loads a pinned, same-origin OSM road pack when road
following is selected. No manual import or third-party runtime API is required.
The bundled region is Tokyo centre only (see public/road-data/README.md).
Outside that region, zoomed-out requests beyond the bounded query size, corrupt
files, and incomplete downloads are reported as unavailable, never as no roads.
A successful reference snapshot does not certify that all real roads are mapped.

Road reference loading is independent of water loading. Existing ordinary
construction, anchor height previews, schema22 save data and real-rail following
are unchanged. An active reference selection freezes its source while the map
moves; cancelling permits the current view to load again. Source provenance is
saved through the existing reference-copy construction path.

**Not implemented by this PR:** automatic level crossings, water restrictions,
complete national road coverage, environmental approval of construction, costs.
Those remain separate work. In particular this PR does not add the experimental
global environmental gate that would reject ordinary construction outside
certified coverage. It does not claim unverified water areas are land.

Recovery source: local checkpoint 8346ce67, preserved as a complete verified
bundle. The combined environment integration was separately checkpointed at
05ec415; it is not the released tree. Published work targets a new branch and
never updates the original road worker's branch.
