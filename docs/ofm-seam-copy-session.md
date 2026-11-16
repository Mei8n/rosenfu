# Isolated OFM seam preview hook

`useOfmSeamCopy` is dependency-injected preview preparation, not App or backend wiring. It receives the same provider's `readTileEvidenceBatch`, frozen full tile snapshots, explicit complete required tile coordinates, two fragment/segment/fraction picks, world identity, tool and enabled state.

The reader returns `{status:'ready',evidence:{rawTiles,bindings}}`, `{status:'unavailable',reason}` or `{status:'stale'}`. It must wrap PR173's same-repository raw API; a fresh repository has no observed SHA identity and is incorrect. Provider window/world checks complement the hook's generation checks.

Each preparation snapshots all context before awaiting, validates exact required tile membership and the returned binding/raw set, then invokes the PR169 pure adapter. Raw evidence hashes, per-binding 8MiB totals, geometry and full context are checked by that adapter. No viewport-cropped candidate graph may replace the full frozen tile snapshots. Single-fragment/OSM/rail flows are not changed. Missing evidence or an unresolved boundary remains an error, never an invented connection.

World, tool, enabled, picks, required tile coordinates, frozen snapshot identity or reader changes invalidate old results immediately at render. Effect cleanup aborts requests; generation checks reject late responses even from a noncompliant reader. Repeat requests supersede older work. Manual cancellation and unmount discard in-flight results. Inputs are immutable React values: mutate neither frozen snapshots nor world objects in place. The consumer must keep reader and frozen array identity stable during an unchanged editing session.

Output contains a draft proposal only. Backend must independently decode raw PBF and reconstruct the path in the same atomic transaction as construction. No source registration, construction, save, App, session command or storage operations are performed here. Dependencies PR169 and PR173 and backend wiring must be integrated and tested before runtime enablement.
