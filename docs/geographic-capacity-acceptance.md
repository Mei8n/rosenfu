# #197: bounded geographic archive acceptance

The storage integration test exercises the actual Rust world build/stage,
evidence archive and file save/reopen/integrity APIs. Every MVT line is
**synthetic**. The OpenFreeMap-style keys are fixture identifiers, not URLs
to download or a claim about a provider, route or coverage.

```sh
cargo test --locked -p rosenfu-storage --test geographic_capacity -- --nocapture
# Add --offline when dependencies are already cached.
```

- At four separated geographic positions it places 16 pins each, with
  different synthetic densities, and reconstructs four separate two-tile
  seam paths. Each is built in a single world; one is also parked before
  the builds. Another 192 pins follow a contiguous tile row for archive
  stress, **not** a continuous built road. These add up to the
  256-binding archive limit.
- The saved file is reopened through the production storage API without a
  network or a previous process's decoded scope. The test compares the full
  world and re-encoded bytes, checks all old-scope geometries and retained raw
  bytes across two complete sweeps, then revalidates all four derived paths
  from the retained archive. A fifth alignment is built from an already
  retained two-tile seam in the dense region after reopening, without fetching or admitting bytes.
- The alternate revision is independently readable, but a path scope may
  not silently substitute it. The entire saved world rejects mixed provider revisions. A 17-tile
  scope and a 257th pin are rejected. The failed world edit does not change
  the saved file, backup or loaded world. A mixed-revision on-disk
  archive also cannot be loaded or adopted as a backup through a save
  using the current file version.
- The test prints exact binding count, unique raw bytes and save bytes, plus
  host measurements for archive construction, fresh file reopen and two
  scope sweeps. Times are observations, not pass/fail thresholds. To measure
  peak process RSS, build with `cargo test --release --locked --no-run
  -p rosenfu-storage --test geographic_capacity`, then run the printed
  test executable separately under a peak-process memory tool (e.g.
  Linux `/usr/bin/time -v`); don't count the compiler. This test retains
  fixture input and a comparison world, so process RSS is not a production
  renderer/client or incremental archive measurement.

An example Linux x86-64 Rust 1.100.0 release run (three fresh processes,
base source `02c605f` plus this test): 256 bindings, 248 unique raw blobs,
428,830 raw bytes and 867,185 save bytes. Archive construction was
28–44 ms, file reopen 35–61 ms, and the two scope sweeps 106–132 ms.
Peak RSS for the full acceptance executable was 18,172–18,412 KiB.

This complements the committed historical
[road-crossing benchmark](national-road-crossing-benchmark.md), whose
shipped Tokyo road pack and metrics are a separate dataset. No official
regional MVT pack is included. This does not verify actual long-distance
connected route construction, complete nationwide capacity, desktop
Undo/Redo, or multiplayer source reconciliation. Save version, path,
archive, and request limits are unchanged.
