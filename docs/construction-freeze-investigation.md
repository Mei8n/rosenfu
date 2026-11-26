# 着工フリーズの調査

作業先は`dev/react-rendering-performance`。比較元は`a72ef1f`。最初に計測した`c92bd41`と比較元のRust実装は同一。既存のReactレイヤー分離・Reducerは変更せず、計測と原因箇所に絞って修正した。

## 分かったこと

道路を追加する場合だけでなく、通常の線路・鉄道参照からの着工でも、World更新後に道路交差の再集計が走る。さらに、Worldに接続や線路数変更区間がある場合は物理線路の描画ジオメトリも再計算される。

長時間処理は次の2箇所で再現した。

1. `roads.rs`の交差重複判定が蓄積済みの全交差を走査し、離れた候補にも道路ID比較と地理距離計算を繰り返していた。
2. `track_render_geometry`が各出力点で`Alignment::point_at()`を複数回呼び、そのたびに全区間の距離を再計算していた。参照線形の点数が多いほど増加した。

これらを実行する`road_assessments`と`preview_track_render_geometry`、および着工IPCは同期Tauriコマンドだった。Tauriの同期コマンドラッパーはワーカーへ移さずコマンドを直接呼ぶため、IPC受信側のUIスレッドを占有する経路になる。演算の長さとUIスケジューリングの問題を別々に修正した。

ユーザーの保存データ・実際のTauriウィンドウでの再現は未確認。以下は実Rust処理の人工データ計測と、IPCをモックしたReact計測であり、実アプリのend-to-end時間やFPSとは区別する。

## Rustの前後比較

Rust 1.100.0、デバッグビルド（unoptimized）、各3回の中央値、単位ms。道路・線路は合成した平行線と交差線で、密な交差を意図的に作るstress fixture。実地図の平均ケースを表すものではない。[全サンプル](performance/construction-freeze.json)に生データを保存した。

| 線路数 / 道路数 | 着工本体 前→後 | snapshot JSON化 前→後 | 着工後の道路交差集計 前→後 |
|---|---:|---:|---:|
| 20/100 | 2.833 → 2.476 | 0.931 → 0.789 | 4.651 → 2.491 |
| 80/1000 | 27.056 → 23.061 | 8.050 → 6.616 | 1514.050 → 129.338 |
| 80/5000 | 116.177 → 112.648 | 35.222 → 31.766 | 32089.738 → 666.259 |

交差集計の位置範囲検索では、保存済みcrossingの挿入順・採用IDを保ち、最後に従来と同じ「地理距離1cm未満、線路上距離1cm未満、元道路ID条件」を適用する。位置の索引は一時的なRustメモリだけで、Worldや保存形式には追加しない。

参照線形は道路コピーと鉄道グラフ由来の両モードを測った。次の時間は`world_for_ui_preview`による検証と物理線路ジオメトリの合計。着工そのものとは別の後処理である。

| 参照モード / 点数 | 描画用検証・ジオメトリ 前→後 |
|---|---:|
| 道路 / 64 | 0.553 → 0.156 |
| 道路 / 2048 | 506.372 → 92.219 |
| 道路 / 4096 | 2038.136 → 366.721 |
| 鉄道 / 64 | 0.491 → 0.152 |
| 鉄道 / 2048 | 505.858 → 90.610 |
| 鉄道 / 4096 | 1959.508 → 382.310 |

サンプリングは、区間距離だけを1回計算して再利用する。`point_at`と同じ減算・補間順序を共有し、座標の丸め方を変えない。キャッシュあり・なしの座標の完全一致、既存の分岐・線路接続・スロープのテストを実行した。区間探索自体の線形走査は残している。

## React・JavaScriptの計測

`ConstructionPhaseProfile.test.jsx`で実際のAppをReact Profilerで包み、20線路/100道路と80線路/5,000道路を比較した。1回warmup、3サンプル。プロバイダーはoffline、Rust IPCは即時応答のモックなので、実通信・Rust計算・実際のIPCペイロード転送の時間は含まない。

| World | 着工操作中のcommit数 前/後（各試行） | React actualDuration合計の中央値 前→後 |
|---|---|---:|
| 20線路 | [5, 5, 5] / [5, 5, 5] | 154.721 → 148.741 ms |
| 80線路 | [5, 5, 5] / [5, 5, 5] | 184.955 → 156.343 ms |

commit数は変化していない。Reactの構造改善は今回行っておらず、時間差をReact最適化の効果とは評価しない。少数サンプルで、JIT・GC・マシン負荷の影響もある。

変更後の80線路fixtureの各spanの中央値は次のとおり。await区間は待機中に行われるReact処理も含み、span同士は重複するため足し合わせない。

| span | ms | 解釈 |
|---|---:|---|
| `prepare` | 0.010 | commitAlignmentの準備 |
| `road.ensureCurrent` | 66.190 | offline providerの待機区間。待機中のbusy表示のReact処理も含む |
| `road.attachment` | 0.009 | 取得結果の添付判断。このfixtureでは道路取得失敗のため軽い |
| `ipc.build` | 0.027 | モックの待機。実Rust IPC時間ではない |
| `perform` | 66.508 | 非同期操作からbusy解除まで |
| `world.apply-state` | 0.121 | setWorldと計画・履歴関連setterの呼び出し |
| `world.schedule-to-layout` | 41.082 | state更新予約から新Worldのlayout effectまで |
| `world.layout-to-frame` | 65.256 | layout effectから次のrAFまで。paint時間ではない |

`actualDuration`はrender時間で、Profilerの`commitTime`は時刻である。commit-phaseのDOM変更だけの所要時間・実WebViewでのpaint・FPS・ユーザーデータの実IPC往復は未測定。

## 修正内容と互換性

- 交差重複判定の全件走査を、線路上の位置の保守的な範囲検索に置き換えた。近傍候補には従来の厳密条件を適用する。旧判定との比較テストで、近接道路・同位置・OFM ID・折り返し線路・入力順の反転を検証した。
- 物理線路描画のサンプリングで区間距離を再利用する。補間処理を共有し、保存・点追加・スナップの計算は変えない。
- 既存の撤去用ワーカーを`run_world_edit`に一般化し、通常・駅付き・道路付き・道路/鉄道参照の着工にも再利用した。ロックの取得もワーカー内で行い、トランザクション自体はそのまま。
- 読み取り専用の交差集計・線路ジオメトリは、ワーカー内でeditor snapshotを取得し、live editorのMutexを解放してから計算する。遅延した応答を旧Worldに結び付ける既存のReactガードを維持する。
- IPC名・JSON引数・戻り値・World保存形式は同じ。建設検証、道路の25秒deadline、取り消し、stale拒否、着工後の履歴境界は維持する。新しいReducerや大きなReactコンポーネント分割は追加していない。

## 手元のアプリでの計測

Rust側のログを有効にして起動する。

```sh
ROSENFU_CONSTRUCTION_TIMING=1 npm run tauri -- dev
```

WebViewの開発者ツールで次を設定して再読み込みする。Profilerの値を得るにはReact開発版、またはprofiling対応ビルドが必要。

```js
localStorage.setItem('rosenfu.construction-timing', '1');
location.reload();
```

着工後に次でspanを取得できる。通常時は計測OFF。計測データはWorldに保存せず、外部送信もしない。

```js
console.table(performance.getEntriesByType('measure')
  .filter(e => e.name.startsWith('rosenfu.construction.'))
  .map(e => ({attempt:e.detail.attempt, phase:e.detail.phase, ms:e.duration})));
// 終了時
localStorage.removeItem('rosenfu.construction-timing');
location.reload();
```

`ipc.build`と`ipc.road_assessments` / `ipc.preview_track_render_geometry`で、着工本体と後処理を分離する。Rustログの`world_edit.queue` / `world_edit.lock` / `world_edit.transaction`、`world_snapshot.queue` / `world_snapshot.lock_clone`、実際の交差/線路計算時間と突き合わせる。`click-to-first-frame`は新Worldのlayout effect後の次のrAFまでであり、後続IPCすべての完了やpaint完了を意味しない。

## 自動計測の再実行

```sh
cargo test -p rosenfu-desktop measure_construction_phases -- --ignored --nocapture
cargo test -p rosenfu-desktop measure_reference_construction_phases -- --ignored --nocapture
ROSENFU_CONSTRUCTION_PROFILE=1 ROSENFU_CONSTRUCTION_PROFILE_OUTPUT=/tmp/construction.json \
  npm test -- src/ConstructionPhaseProfile.test.jsx --maxWorkers=1
```

前後比較では、同じ計測用test関数を`a72ef1f`のworktreeにも追加して同じfixture・debug設定で実行する。通常テスト・ビルド・他の計測を並走させない。デバッグビルドの数値からrelease版の改善率やUI停止時間を推測しない。

## 残る負荷

このfixtureでは交差結果だけで42,770,584 bytes、変更後のJSON化に中央値1489.131 msかかった（変更前の同項目は未測定）。ワーカーに移しても演算・転送量そのものは消えない。実WebViewのJSON受信/parseや多数の交差データのGCは別途確認が必要。集計値のみのAPIへの変更は今回行っていない。

4,096点のジオメトリにもdebug版では数百ms残る。大きなeditorのsnapshotコピー、他の同期IPC、実際の道路取得・タイルdecodeのコストもこの人工データだけでは評価しきれない。次は手元の再現Worldで上記のspanを収集し、残る支配項を確認する。

## 検証

- `cargo test --workspace`: 成功（944 passed / 26 ignored。複数ハーネスに同じsessionテストを含む）。
- 交差許容誤差・ID・折り返し位置の旧実装との比較、補間座標の完全一致、既存の着工・保存・Undo/Redo・分岐・スロープ・道路境界テストが成功。
- frontend全体: 1129 passed / 4 skipped（最後の診断API失敗テスト追加前）。Profiler計測は別途成功。
- 対象テスト43件が成功。さらに最終変更後の4ファイル・20テストが成功し、計測OFF、診断API失敗、IPC失敗、最初のフレームより遅い後続IPC、道路確認、stale拒否、参照コピー、履歴境界、線路描画を検証した。
- `npm run build`、`cargo fmt --all -- --check`、`git diff --check`: 成功。既存の500 kB超のbundle警告は残る。
