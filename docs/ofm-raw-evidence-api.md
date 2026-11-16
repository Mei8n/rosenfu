# Raw road tile evidence API (not connected to App)

`createOpenFreeMapRoadRepository().readTileEvidenceBatch(refs, signal?)` returns `{bindings, rawTiles}`. Each reference supplies revision, z14, x/y, rawSha256, and decoderRule=`ofm_transportation_z14_v1`. It must match a successfully decoded tile observed by this repository. No caller URL or reconstructed MVT is accepted.

Bindings carry canonical fixed provider URL/key, raw SHA, tile coordinates/revision, ODbL license and attribution. Raw blobs are deduplicated by SHA and returned as copies. This aligns with the frontend shared-evidence and Rust EvidenceStore contract, but does not imply evidence has been installed, persisted or validated by Rust.

Original network bytes are owned by a bounded 16MiB/16tile LRU after decode, not re-encoded. The normal decoder does not retain another raw copy. Known observed hashes are bounded to64 tiles. Evicted raw bytes may be re-fetched only from their exact revision URL, sharing the provider's serial queue, request timeout and lifetime50MiB/3000tile network budget; changed SHA rejects. No alternative revision, empty evidence or partial result is substituted. A stale unknown reference requires a fresh observation, not a guessed source.

Each evidence batch is <=16 tile bindings and <=8MiB unique bytes (same as Rust request limits). Concurrent batches <=16, each has25s deadline and independent optional cancellation. The returned copies belong to the caller: callers must not accumulate unlimited batches and must still enforce complete-draft/storage budgets. Repeated cached references do not bypass Rust validation or construction permission. Internal SHA checks prove byte consistency, not provider authenticity, road completeness, or legal permission.

This branch changes no App/runtime wiring, no background map, no world/save schema, no main branch and no local user PC. Runtime request/UA/provider terms from the OFM adapter remain applicable.
