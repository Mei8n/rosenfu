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

## Crossing inputs with actual retained road geometry

A separate Linux x86-64 native fixture retained these **actual source
geometries** in the same World:

- Shipped Tokyo pack: 36 cells, 37,643 way entries and 224,908 coordinates.
  Coverage was verified relative to the pinned source extraction.
- Frozen OSM extracts for Shimada and Dejima: 35 way entries and 642
  coordinates, with coverage **unverified**. These are neither released
  road packs nor OpenFreeMap tiles.

The seven fictional test tracks include four paths using the independent
**synthetic** MVT archive described above. That archive still holds 256
bindings, with a maximum of 16 tiles per scope. It does not provide
actual OpenFreeMap coverage in any of the three places.

The mixed file occupied 14,894,811 bytes, 1,882,405 bytes below the
16 MiB limit. It reopened offline in a fresh native process and reproduced
the same canonical bytes. A separate, valid synthetic addition of 600 ways
and 60,000 coordinates exceeded the storage limit: capacity check, encoding,
and a compare-and-save with the correct prior version each rejected it.
The existing save and backup still reopened with identical bytes and
version hashes.

For at most 64 alignments, the crossing assessor skips direct geometry
preparation for snapshots whose declared scopes cannot intersect any
queried alignment. Verified coverage still comes from the original
sources; split-way continuation uses its exact source point; the global
OpenFreeMap seam budget and matching are unchanged. Larger alignment
collections retain the broad preparation path. The mixed fixture prepared
4 of 38 snapshots (2,455 of 37,678 way entries). Ordered crossing results
matched the baseline exactly. Three paired Release runs of the native
assessment alone (31 samples per process) measured median times of
1.90–2.06 ms before and 0.11–0.26 ms after the change.

This improves crossing assessment, not save size, JSON decoding, viewport or
desktop responsiveness. It does not establish connected long-distance
routing or nationwide capacity.

## Verify an independently approved captured alignment

For an isolated existing save with an already authorized source capture,
verify the approved fixture locally in the headless CLI:

```sh
cargo run --locked --offline -p rosenfu-cli -- world verify-road-proof \
  APPROVED.json APPROVED_SHA256 CAPTURE.rosenfu
```

The independently reviewed JSON manifest has these exact fields:

- `format`: `rosenfu-road-path-scope-v1`.
- `origin`: `captured` for a separately authorized provider capture, or
  `synthetic` for generated geometry. This label is a declaration.
- `save_sha256`: the exact SHA-256 of the approved isolated save bytes.
- `alignment_id`: the positive decimal string ID of the selected alignment.
- `spans`: exact `world.road_path_provenance` entries for the selected
  alignment in increasing start-point order.
- `tiles`: exact `world.road_evidence_descriptors` entries, in the order
  their full keys first occur among the spans' `requiredBindingKeys`.

Pass the manifest's approved SHA-256 separately. The CLI verifies the exact
discardable input bytes with the production save loader, then checks that the
selected alignment is fully and contiguously protected and has points in at
least two bound z14 cells. Its JSON receipt includes ordered span ranges and
digests, not raw tiles or coordinates. A mismatch emits no receipt. This
command does not write either file, fetch tiles, change any game state, or
verify real-world coverage, legal access, or source licensing. Verification
of a real capture remains separate from the synthetic acceptance above.
