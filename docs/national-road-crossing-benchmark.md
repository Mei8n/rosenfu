# 道路踏切判定の性能計測

2026-11-15、東京の同梱道路 pack を用いた core の局所計測。全国の性能保証ではない。

## 対象と条件

- baseline: `2cb1ac8f697cc2c9013840f51912ad168f5b2fcf` の `crates/rosenfu-core/src/world/roads.rs`。
  baseline ファイル SHA-256: `7fa47372efc2946570319c4f16a054ea290811dcb98e14ccce54602c43271d8e`。
- optimized: 同 baseline に本変更の bbox broadphase を適用したもの。
  ファイル SHA-256: `57f1d5fc6eabf721bb16470a4b3e42fec252ec7df60ef6f6f07e58fdfb0573a9`。
- Linux x86_64 cloud container、CPU 表示 AMD EPYC 9V74、rustc 1.100.0
  (`0ef1bc386 2026-11-09`)、Cargo `--release`。共有ホストの単発実測であり統計的ベンチマークではない。
- `public/road-data/manifest.json` SHA-256:
  `ac62a1f9ac146830434c9266ba8601bcfa3a06e270a50d59c2413f1a517e6d3d`。
- revision: `2026-11-14T21:20:41Z`。出典:
  `https://download.geofabrik.de/asia/japan/kanto-261114.osm.pbf`。
  parent PBF SHA-256: `29967275fa5925a5dc224708e41869800471745d53db004b25104c6eddca52dc`。
  OSM / ODbL-1.0。既存 pack 以外の道路データや巨大生成物は追加していない。
- 1 cell: `6985_1783`。4 cells: `6985_1783`, `6986_1783`, `6985_1784`, `6986_1784`。
  36 cells: manifest 全セル（139.68–139.80 E、35.60–35.72 N）。
- 各セルを独立した snapshot とし、元 way 全点・タグを保持。セル間重複は残す。
  4 重条件は同じ道路を複製し、各 coverage の west のみ 0 / 0.000001 /
  0.000002 / 0.000003 度内側へ変更した、異なる取得範囲の重複保持を模した負荷。
  完全同一 snapshot の冪等追加を意図的に迂回する測定ではない。`World::new` で検証する。
- 線路は実在線路ではなく、実道路上の固定した合成直線。経度 139.701→139.719、
  緯度 35.665 + 線番号 × 0.002、地表高 0 m、各線約 1.6 km。
  2 / 101 / 501 点に等間隔分割。線間の接続はない。
- 計時対象は `World::road_assessments()` 1 回のみ。ファイル IO、JSON parse、
  World 構築/検証、結果 JSON 出力、IPC、描画を含まない。検証時間は別出力。
  各実行を `timeout 20s` で制限し、以下より大きな条件は生成しない。

## 実測

単位 ms。表は最初の比較実行。速度保証や CI の時間閾値にはしない。

| cells | 線数 × 点数 | 重複数 | road 数 | 座標数 | baseline | optimized | 踏切数 |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 × 2 | 1 | 918 | 5,449 | 0.06 | 0.05 | 34 |
| 1 | 1 × 101 | 1 | 918 | 5,449 | 1.77 | 0.09 | 34 |
| 1 | 1 × 501 | 1 | 918 | 5,449 | 7.82 | 0.14 | 34 |
| 4 | 3 × 101 | 1 | 4,229 | 24,770 | 23.85 | 0.56 | 95 |
| 4 | 3 × 501 | 1 | 4,229 | 24,770 | 111.74 | 0.89 | 95 |
| 4 | 3 × 501 | 4 | 16,916 | 99,080 | 434.20 | 2.45 | 95 |
| 36 | 3 × 101 | 1 | 37,643 | 224,908 | 584.39 | 3.17 | 95 |
| 36 | 3 × 501 | 1 | 37,643 | 224,908 | 2,063.36 | 3.53 | 95 |
| 36 | 3 × 501 | 4 | 150,572 | 899,632 | 7,241.46 | 16.78 | 95 |

36 cells / 3 × 501 / 1 の再計測を含む範囲は baseline 2,026–2,190 ms、
optimized 3.43–3.53 ms。保存 JSON 13,988,683 bytes で frontend の約 15 MiB
添付制限内。4 cells / 4 重は 6,228,296 bytes。
36 cells / 4 重は 55,752,496 bytes で core の制約内だが frontend 添付制限外。
最後の行を通常 UI 操作の再現結果として扱わない。

## 結果の同一性と残る制限

36 cells / 3 × 501 / 1 について、踏切数だけでなく `road_assessments()` の
全 `BTreeMap` を `serde_json::to_vec` で保存し、新旧を `cmp` で byte 比較した。
両方の SHA-256 は次のとおり。

```
294f747a5ecedfc52e884dba18087831b0d07c5e9d45828dde324739886ae571
```

core 全 143 tests、fmt、diff check が成功。既存の接続端点・立体分離テストに加え、
coverage 境界上の交差、遠方 unknown road、近傍 unknown / elevated を回帰検証。
最適化は道路/線路 bbox による候補削減のみ。bbox を source coverage に転用せず、
道路 split endpoint 補完は全道路を参照する。schema と保存形式は不変。

密集した長大道路、全線路が同じ bbox を通る世界、split endpoint 大量発生、
IPC 転送や描画の負荷はこの計測だけでは保証しない。永続空間 index は追加していない。

## 再現用 harness

リポジトリを変更しない一時 Cargo project を使用する。下の Rust コードを
一時ディレクトリ `src/main.rs` に保存し、以下の Cargo.toml を置く。
`/absolute/path/to/repo` は検証 checkout の絶対パスに置換する。

```toml
[package]
name = "road-perf"
version = "0.1.0"
edition = "2021"
[dependencies]
rosenfu-core = { path = "/absolute/path/to/repo/crates/rosenfu-core" }
serde_json = "1"
```

```rust
use rosenfu_core::world::*;
use std::time::Instant;
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cells: usize = args[1].parse().unwrap();
    let vertices: usize = args[2].parse().unwrap();
    let rails: usize = args[3].parse().unwrap();
    let copies: usize = args[4].parse().unwrap();
    let path = std::env::var("ROAD_PACK").expect("ROAD_PACK must point to public/road-data");
    let m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{path}/manifest.json")).unwrap())
            .unwrap();
    let mut snaps = vec![];
    let names: Vec<String> = if cells == 1 {
        vec!["6985_1783".into()]
    } else if cells == 4 {
        vec![
            "6985_1783".into(),
            "6986_1783".into(),
            "6985_1784".into(),
            "6986_1784".into(),
        ]
    } else {
        m["cells"].as_object().unwrap().keys().cloned().collect()
    };
    for name in names {
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(format!("{path}/{name}.json")).unwrap())
                .unwrap();
        let b = &v["bounds"];
        let roads = v["ways"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| RoadWay {
                id: w["id"].as_str().unwrap().into(),
                coverage: None,
                points: w["coordinates"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| GeoPoint {
                        longitude: p[0].as_f64().unwrap(),
                        latitude: p[1].as_f64().unwrap(),
                    })
                    .collect(),
                layer: w["tags"]["layer"].as_str().map(String::from),
                level: w["tags"]["level"].as_str().map(String::from),
                bridge: w["tags"]["bridge"].as_str().map(String::from),
                tunnel: w["tags"]["tunnel"].as_str().map(String::from),
            })
            .collect();
        let s = RoadSnapshot {
            revision: m["revision"].as_str().unwrap().into(),
            source: "Tokyo-pack".into(),
            source_url: m["source"]["url"].as_str().unwrap().into(),
            source_sha256: m["source"]["parent_pbf_sha256"].as_str().unwrap().into(),
            attribution: m["attribution"].as_str().unwrap().into(),
            license: m["license"].as_str().unwrap().into(),
            coverage_verified: true,
            road_coverage: vec![CoverageBounds {
                west: b[0].as_f64().unwrap(),
                south: b[1].as_f64().unwrap(),
                east: b[2].as_f64().unwrap(),
                north: b[3].as_f64().unwrap(),
            }],
            roads,
        };
        for n in 0..copies {
            let mut s = s.clone();
            s.road_coverage[0].west += n as f64 * 0.000001;
            snaps.push(s);
        }
    }
    let roads: usize = snaps.iter().map(|s| s.roads.len()).sum();
    let coords: usize = snaps
        .iter()
        .flat_map(|s| s.roads.iter())
        .map(|r| r.points.len())
        .sum();
    let mut w = World::default();
    let p = w.add_player("p".into()).unwrap();
    let c = w.add_company(p, "c".into()).unwrap();
    for r in 0..rails {
        let pts = (0..vertices)
            .map(|i| GeoPoint {
                longitude: 139.701 + i as f64 / (vertices - 1) as f64 * 0.018,
                latitude: 35.665 + r as f64 * 0.002,
            })
            .collect();
        w.build_alignment(c, format!("r{r}"), pts).unwrap();
    }
    let mut draft = w.snapshot().clone();
    draft.road_snapshots = snaps;
    let bytes = serde_json::to_vec(&draft).unwrap().len();
    eprintln!("world_json_bytes={bytes}");
    let start = Instant::now();
    let w = World::new(draft).unwrap();
    let validate = start.elapsed();
    let start = Instant::now();
    let a = w.road_assessments();
    let elapsed = start.elapsed();
    if let Ok(out) = std::env::var("RESULT_JSON") {
        std::fs::write(out, serde_json::to_vec(&a).unwrap()).unwrap();
    }
    let crossings: usize = a.values().map(|a| a.crossings.len()).sum();
    println!("cells={cells} vertices={vertices} rails={rails} copies={copies} roads={roads} coords={coords} validate_ms={:.2} assess_ms={:.2} crossings={crossings}",validate.as_secs_f64()*1000.,elapsed.as_secs_f64()*1000.);
}
```

Rust toolchain を PATH に設定し、一時 project 内で実行する。
既存依存キャッシュがない場合は通常の `cargo build --release` を使う。

```sh
export ROAD_PACK=/absolute/path/to/repo/public/road-data
export CARGO_TARGET_DIR=/absolute/path/to/temporary-target
cargo build --release --offline
RESULT_JSON=/tmp/road-perf-optimized.json timeout 20s \
  "$CARGO_TARGET_DIR/release/road-perf" 36 501 3 1
# 引数: cells vertices rails copies。上の表にある条件だけを順に実行する。
```

baseline 比較は別 checkout または一時 core コピーで行い、作業中の repository の
ファイルを上書きしない。一時コピーなら `crates/rosenfu-core/src` 全体をコピーし、
その `world/roads.rs` だけを以下で取り出したファイルにする。

```sh
git show 2cb1ac8f697cc2c9013840f51912ad168f5b2fcf:crates/rosenfu-core/src/world/roads.rs
```

一時 core の Cargo.toml は以下とする。

```toml
[package]
name = "rosenfu-core"
version = "0.1.0-alpha.1"
edition = "2024"
[dependencies]
serde = { version = "1", features = ["derive"] }
```

harness の path 依存を一時 baseline core に変更して再 build し、同じ条件で保存する。

```sh
cargo build --release --offline
RESULT_JSON=/tmp/road-perf-baseline.json timeout 20s \
  "$CARGO_TARGET_DIR/release/road-perf" 36 501 3 1
cmp /tmp/road-perf-optimized.json /tmp/road-perf-baseline.json
sha256sum /tmp/road-perf-optimized.json /tmp/road-perf-baseline.json
```

完全な SHA 再現には記載 revision の core/pack と同じ入力順・harness が必要。
将来の schema/serde 出力変更後は、新旧同一入力の結果一致と性能を再測定する。
