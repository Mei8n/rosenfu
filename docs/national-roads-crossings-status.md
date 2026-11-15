# National road packs and derived crossings: implementation checkpoint

This branch is not a completed national road release. `NATIONAL_ROAD_RELEASE` remains null and the existing Tokyo provider remains active. No application UI calls the new road-aware build/preview commands yet.

Implemented foundations:
- Bounded PBF extraction into pinned regional/sharded manifests, with explicit unknown boundary cells and source-version status.
- Same-origin pinned national index reader; source revision/hash checks, shared transfer budget, bounded concurrent loads and retry cooldown. Cross-shard requests currently fail explicitly.
- Saved immutable road snapshots and derived ground-level crossings, including explicit connected rail endpoints. Road absence is only confirmed inside verified coverage; unknown road height is reported. Missing data does not disable ordinary construction. Water is neither loaded nor inferred by this module.
- Atomic road-aware preview/build commands, save-size validation before commit, and recomputation after edits/removal/Undo.

Pending before user-facing completion:
- Source clipping-polygon version verification and national hosting/distribution.
- National runtime publication; generated Shikoku data is an offline verification artifact, not shipped coverage.
- App wiring, map display, and clear unverified-road feedback; integration with ongoing water changes.
- Snapshot deduplication/retention for long sessions (the existing 16 MiB save limit remains authoritative).
- Final independent review and current-head three-OS CI.

Save format changes from schema 22 to schema 23 for road snapshots. Older schemas are rejected without deleting their files, following the alpha compatibility policy. No save conversion or user file deletion is performed.
