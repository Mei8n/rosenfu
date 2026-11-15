# National road packs and derived crossings: implementation checkpoint

This branch is not a completed national road release. `NATIONAL_ROAD_RELEASE` remains null and the existing Tokyo provider remains active. Construction now uses the road-aware build command when a current snapshot is available. Preview geometry validation remains independent of optional road data.

Implemented foundations:
- Bounded PBF extraction into pinned regional/sharded manifests, with explicit unknown boundary cells and source-version status.
- Same-origin pinned national index reader; source revision/hash checks, shared transfer budget, bounded concurrent loads and retry cooldown. Cross-shard requests currently fail explicitly.
- Saved immutable road snapshots and derived ground-level crossings, including explicit connected rail endpoints. Road absence is only confirmed inside verified coverage; unknown road height is reported. Missing data does not disable ordinary construction. Water is neither loaded nor inferred by this module.
- Atomic road-aware preview/build commands, save-size validation before commit, and recomputation after edits/removal/Undo.
- Construction waits at most 25 seconds for a shared current road request, supports cancellation, and rejects stale world/plan results. Failure keeps normal construction available but explicitly unverified. Later acquisition alone does not retrofit saved references; see automatic-road-crossings.md.

Pending before user-facing completion:
- Source clipping-polygon version verification and national hosting/distribution.
- National runtime publication; generated Shikoku data is an offline verification artifact, not shipped coverage.
- Nationwide runtime data and actual GUI acceptance. App wiring, derived map markers and unverified feedback have focused tests; this is not proof of national availability.
- Snapshot deduplication/retention for long sessions (the existing 16 MiB save limit remains authoritative).
- Final independent review and current-head three-OS CI.

Save format changes from schema 22 to schema 23 for road snapshots. Older schemas are rejected without deleting their files, following the alpha compatibility policy. No save conversion or user file deletion is performed.
