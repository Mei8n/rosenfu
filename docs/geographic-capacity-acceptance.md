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

## Bounded disk reopen memory

The normal file-open APIs release their input buffer after parsing and
computing the version of those exact bytes, before reconstructing the world.
The byte-slice API still borrows bytes from its caller; the parsed document
and all previously required validation are unchanged. Evidence decoding still
requires canonical padded standard Base64, but no longer allocates an encoded
copy of each decoded tile to make the same check a second time. No limits,
archive versions, file bytes or route validation rules change. This does not
implement per-region world paging or raise the 16 MiB capacity.

The geographic integration regression reopens two regions with one shared
proof and an active synthetic service on a 90 km alignment through both file
APIs and the byte-slice API. It checks that harmless wire formatting is
reflected in the exact version even when the same world and runtime reopen
(including on the compatibility JSON path), that both scopes remain usable,
and that bad Base64 padding or spare bits are still rejected. A disk-based
test preserves the 29/30 and 31/32 labels' downgrade/capability diagnostics
with the Standard feature both on and off; reading never changes these files.

For an apples-to-apples Linux Release RSS/time comparison, generate one
isolated fixture and use identical bytes in separate baseline and candidate
processes. The fixture combines eight distinct 1 MiB synthetic MVT blobs
(1,024 roads per tile, four distant regions) and the independently authored
long synthetic rail service. It does not simulate a continuous real road
between any of those regions.

```sh
scratch=$(mktemp -d)
ROSENFU_GEOGRAPHIC_REOPEN_FIXTURE="$scratch/fixture.rosenfu" \
  cargo test --locked --release -p rosenfu-storage --test geographic_capacity \
    prepare_dense_and_long_geographic_reopen_fixture -- --ignored --exact --nocapture
# Separately build baseline/candidate CLI with the same toolchain and features.
sha256sum "$scratch/fixture.rosenfu"
# Repeat and alternate these from fresh processes; compare complete stdout.
time_bin=/usr/bin/time
"$time_bin" -v BASELINE/target/release/rosenfu-cli world inspect "$scratch/fixture.rosenfu"
"$time_bin" -v CANDIDATE/target/release/rosenfu-cli world inspect "$scratch/fixture.rosenfu"
sha256sum "$scratch/fixture.rosenfu"
```

Also run the ordinary `geographic_capacity` suite, the
`file_reopen_preserves_street_tram_header_and_downgrade_diagnostics` test with
and without `standard-periodic-rail`, and the
`exactly_16_mib_compact_save_reopens_and_keeps_exact_backups_and_cas` test.
The removed restoration-time file allocation is at most the allowed input
size, and one transient Base64 string per tile is also no longer allocated.
Filesystem cache and parser peaks can differ; these counts do not establish a
particular RSS or latency improvement without the paired measurement.

## Wire-capacity preflight during editing and reopening

Capacity preflight counts compact JSON once. Pretty JSON adds only
whitespace, so compact is the smallest supported representation and its
exact bounded conversion is equivalent to asking whether either supported
layout fits. This is only the in-memory count: the actual file writer still
uses pretty JSON whenever it fits and preserves existing bytes, final
newline, exact version checks, and backup semantics. The 16 MiB limit and
document validation are unchanged.

```sh
cargo test --locked --release -p rosenfu-storage --lib \
  measure_compact_storage_preflight -- --ignored --nocapture
```

An offline Linux x86-64 Release run tested the actual footprint function
against the old pretty-then-compact count in one binary, alternating the
order of the two arms. Fixture construction and draft validation are outside
the measurement. Four independent processes gave these ranges of medians
(25 samples per arm per process):

| Synthetic alignments | Former preflight | Compact preflight |
| ---: | ---: | ---: |
| 100 | 65.7–94.9 µs | 38.0–57.5 µs |
| 1,000 | 0.66–1.10 ms | 0.39–0.65 ms |
| 10,000, padded to exactly 16 MiB compact | 25.8–38.6 ms | 11.2–17.2 ms |

The oversized pretty representation only occurs in the padded case. These
are CPU timings for the save-size check used by native construction preview,
edit confirmation and reopen; they are not full edit, load, GUI, or disk
write benchmarks and do not demonstrate nationwide capacity. The regression
tests separately check refusals immediately below the compact limit, exact
fit, supported escaped text and the unchanged pretty-file preference.

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

## Selective manifest inspection from disk

For an existing save, the read-only CLI can report the retained OpenFreeMap
tile declarations inside an inclusive slippy-map z14 rectangle. The example
selects two x and two y coordinates in the Tokyo synthetic fixture:

```sh
cargo run --locked --offline -p rosenfu-cli -- world road-coverage \
  WORLD.rosenfu 14552 6451 14553 6452
```

It scans the current bounded JSON file but does not deserialize the complete
world, decode evidence blobs, construct a region grid proportional to the
rectangle, or fetch data. Matching entries are ordered by x/y. The response
is explicitly `manifest_only_not_validated`: a declared tile is not proof of
valid bytes, a missing tile does not rule out roads or other geography, and
the result never authorizes construction. Full world loading and raw proof
validation remain unchanged; this is not an alternative loader or a new
save format. It can inspect the shared manifest in known schemas 26–35; the
combined schema requires a recognized ordered capability list. None of this
enables the operation or street-tram capabilities of those saves.
It does not page partial worlds for gameplay.

Run the actual CLI integration checks with
`cargo test --locked --offline -p rosenfu-cli --test road_coverage`. To compare
resource use, run `world inspect` and `world road-coverage` from fresh Release
processes on the same bounded multi-region fixture, under a peak-RSS tool.
The operations are intentionally not semantically equivalent: the former
validates the whole saved world. No nationwide memory or timing measurement
is claimed here.

## 分散した駅の配置・グループ検証

着工の確定や読込で、各ホームの線形・駅と同一駅グループの所属を
ID索引で確認する。離れた地域の無関係な駅や線形をホームごとに
走査しない。検索以外の検査順序、勾配・設置位置の条件、セーブ形式は
変えていない。選択・運行・描画の空間索引は対象外。

以下は、札幌・仙台・東京・名古屋・大阪・福岡付近に配置した
**合成**データ。各グループに2ホーム・各線形に9点がある。一社だけの
ワールドで、路線の連続性・実地図や走行中の車両負荷は含まない。
整合性検査、ネイティブ編集での線形1本追加、一時フォルダ内の
上書き保存（以前の保存をバックアップ）、再読込を別々に計時する。
復元するワールド、正確なファイル版、旧ファイルのバックアップ、
再エンコード後の保存バイトも検査する。

```sh
for platforms in 100 1000 6000; do
  ROSENFU_STATION_BENCH_PLATFORMS="$platforms" \
    cargo test --locked --release -p rosenfu-storage \
      --test construction_preview_benchmark measure_geographic_station_edit_storage \
      -- --ignored --exact --nocapture
done
```

OSのファイルキャッシュを消去する試験ではない。観測した時間をCIの
合否条件にはせず、全国の完全な保存容量や、操作画面の応答速度の
保証としては扱わない。

Linux x86_64（rustc 1.98.1、Release）で、変更前（`9268bb0`）と
変更後を交互に各3プロセスで実行。各プロセスの5試行の中央値の範囲（ms）。
すべての試行で、新旧の保存バイト数・SHA-256が同じで、再読込・
バックアップの一致検査にも成功した。

| ホーム数 | 保存サイズ | 整合性検査 | 1線形の追加 | 上書き保存 | 再読込 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 100 | 139,202 B | 0.126–0.155 → 0.092–0.105 | 0.705–0.738 → 0.566–0.698 | 2.71–3.04 → 2.46–3.04 | 0.98–1.28 → 0.90–1.33 |
| 1,000 | 1,394,394 B | 2.16–2.70 → 1.10–1.18 | 10.4–12.5 → 7.21–8.05 | 26.8–30.8 → 24.9–26.5 | 9.53–10.51 → 8.78–9.76 |
| 6,000 | 8,473,217 B | 44.2–48.7 → 6.49–7.81 | 201–211 → 81–89 | 242–269 → 155–169 | 95.0–96.3 → 54.5–60.8 |
