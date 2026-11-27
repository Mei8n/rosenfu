# React状態管理と編集プレビュー

## 構造と依存関係

対象は`dev/construction-review-integration`。比較元は`6582380`（前回の確定済み描画レイヤー分離後）、変更後の実装は`62126e6`。

- `constructionEditor.ts`: 点列、Redo、完了状態、経路・計画ID、計画見出し、退避経路、接続先、編集済みジオメトリ、分岐元、高さ情報を管理する純粋Reducer。
- `App.tsx`: Reducerへの操作の組み立て、既存履歴ジャーナル、非同期処理、World・地図操作を維持する。設定や参照コピー・道路追従固有のセッション状態は独立したまま。
- `usePointerFrame.ts`: React stateに渡す前の一時タスクをrefに保持し、種類ごとに最新の1件だけを同じ描画フレームで処理する。保存・履歴用stateを持たない。
- `ConstructionLayer.tsx`: 確定した建設下書き・退避計画・候補線のストローク。既存のヒット領域とアンカーの間に置き、SVGラッパーを増やさない。点列・カメラ・線路数が同じなら投影と複線計算を再利用する。
- `GsiBackground.tsx`: 既存背景レイヤーに標準の`React.memo`を適用する。タイルのload/errorによるローカルstate更新は引き続き描画される。
- `useConstructionPreview.ts`: 入力配列が同じ場合の再シリアライズを避ける。World・リクエスト内容・水域データrevisionによる結果の有効性確認と、実行中の最新候補1件だけを待機させる仕組みを維持する。

前回の`BuiltTrackLayer`・`StationLayer`・`RouteLayer`・`ReferenceGraphLayer`を再利用する。単にSVG全体を包む`InteractionLayer`は追加せず、更新前のタスクを局所的なrefへ移した。深い比較や新たな保存形式は導入していない。

## Reducerの操作

主なActionは`START_PATH`、`ADD_POINT`、`FINISH_PATH`、`RESUME_PATH`、`NEW_PATH`、`NEW_PLAN`、`SELECT_PATH`、`SELECT_EMPTY_PLAN`、`RENAME_PLAN`、`CLEAR_PLAN`、`PARK_REFERENCE_PATH`、`SET_PARALLEL_PATH`、`APPLY_PATH_EDIT`、`UNDO_POINT`、`REDO_POINT`、`RESTORE_PLANS`。接続の初期化には`CLEAR_CONNECTIONS`を使う。

`SELECT_PATH`は退避と対象経路の復元を一度に行う。`ADD_POINT`は点列・高さ・Redo・接続先を一緒に更新する。ID採番、非同期検証、既存の履歴ジャーナルへの記録はReducerに入れない。Undo/Redo Actionは既存ジャーナルのスナップショットを適用するもので、別の履歴を新設しない。履歴の記録境界と保存時の`ConstructionPlans`形式は従来どおり。

Reducerの導入は整合性と保守性の改善であり、単独でレンダリング性能改善とは扱わない。

## 入力精度と更新タイミング

通常のカーソル移動・ホバー表示は次のフレームに最新値を反映する。クリック、パン・範囲選択・アンカードラッグ、およびその終了処理は同期処理を維持する。クリック/up/cancel時には保留中の表示タスクを捨て、それぞれのイベント座標を使う。

Controlによるスナップ切替と駅配置のEnter/矢印キーは、フレーム前でも最新の入力座標をrefから使い、先行する表示タスクを破棄する。カメラ・ツール・World変更、退出、blur、unmountで保留タスクを破棄する。ReactのSyntheticEventはフレームをまたいで保持しない。

回帰テストの`settledPointerMove` / `settledPointerEnter`は表示フレームを待つ。フレーム制御そのものとクリック精度のテストはrawイベントと手動のrAFキューを使う。

## 計測の再現

```sh
ROSENFU_PROFILE=1 ROSENFU_PROFILE_OUTPUT=/tmp/rosenfu-profile.json \
  npm test -- src/RenderingProfiler.test.jsx --maxWorkers=1
```

`RenderingProfiler.test.jsx`は通常のテスト実行ではskipする。React開発版の`Profiler`を実際のAppの外側に置き、各シナリオ5回のウォームアップ後に30操作を測る。小規模Worldは線路1本・64点・4駅、中規模Worldは線路16本・1024点・64駅。ポインターは1表示フレームあたり4イベント、パン、ホイールズーム、建設中の候補更新、計画名編集を同じ条件で処理する。rAFは手動で進める。

出力はcommit数、commitごとのReact `actualDuration`、操作ごとの`actualDuration`合計、jsdomでの操作経過時間の平均とp95。`commitTime`は時刻であり所要時間ではないため、commit-phaseのDOM変更時間として報告しない。実ブラウザのpaint・FPS・GPU・ネイティブ検証の実コストは未測定。

比較時は別のテストやビルドを並走させず、同じマシン・React開発版・手動rAF・fixtureで実行する。比較元には同じ計測ファイルを持ち込み、ソースだけを`6582380`に切り替える。マシン負荷やJITの影響があるため、単一runの所要時間から本番の改善率を保証しない。

比較元は別のworktreeで再現できる。計測ファイルだけを同じものに揃える。

```sh
git worktree add --detach /tmp/rosenfu-profile-baseline 6582380
cp src/RenderingProfiler.test.jsx /tmp/rosenfu-profile-baseline/src/
cd /tmp/rosenfu-profile-baseline
npm ci
ROSENFU_PROFILE=1 ROSENFU_PROFILE_OUTPUT=/tmp/rosenfu-profile-before.json \
  npm test -- src/RenderingProfiler.test.jsx --maxWorkers=1
```


## 今回の計測結果

Node 24.4.1、React 19.3.1開発版、jsdom 26.1.0。各runはテスト・ビルドを並走させず実行した。前後各1runの結果であり、繰り返しrunによる信頼区間は測っていない。[生データ](performance/react-state-profile.json)には全指標を保存した。

以下の時間は**1操作内のReact render時間の合計**の平均 / p95（ms）。commit-phaseの時間ではない。ポインター・候補更新の1操作は4イベントと1フレーム。

| World（線路 / 駅） | 操作 | commit数 前 → 後 | render平均 前 → 後 | render p95 前 → 後 | jsdom経過p95 前 → 後 |
|---|---|---:|---:|---:|---:|
| 1 / 4 | カーソル | 120 → 30 | 32.29 → 14.10 | 136.97 → 31.90 | 145.93 → 34.78 |
| 1 / 4 | パン | 120 → 120 | 58.24 → 30.60 | 111.72 → 102.85 | 156.00 → 142.79 |
| 1 / 4 | ズーム | 128 → 126 | 40.59 → 31.18 | 133.10 → 139.40 | 171.49 → 178.15 |
| 1 / 4 | 建設候補 | 120 → 30 | 51.81 → 5.67 | 144.33 → 33.97 | 154.82 → 36.99 |
| 1 / 4 | 計画名編集 | 60 → 60 | 34.85 → 36.14 | 38.07 → 40.25 | 104.50 → 115.66 |
| 16 / 64 | カーソル | 120 → 30 | 28.49 → 3.85 | 153.74 → 6.32 | 166.82 → 10.55 |
| 16 / 64 | パン | 120 → 120 | 34.19 → 23.69 | 119.35 → 114.89 | 162.19 → 153.45 |
| 16 / 64 | ズーム | 126 → 124 | 26.00 → 24.19 | 118.09 → 114.85 | 162.72 → 158.37 |
| 16 / 64 | 建設候補 | 120 → 30 | 34.78 → 11.16 | 172.89 → 38.54 | 189.53 → 41.67 |
| 16 / 64 | 計画名編集 | 60 → 60 | 40.70 → 38.61 | 44.27 → 40.89 | 194.53 → 169.24 |

カーソルと候補更新のcommit数は120 → 30。確定済み線路・駅・路線は比較元ですでにカーソル更新に対して0回であり、今回はその条件を維持し、背景地図も0回になることを回帰テストで確認した。20イベントを1フレームに送るテストでは、Appの更新は1回である。

計画名編集のcommit数は60のまま。小規模Worldのズームと計画名編集ではp95が悪化しており、全操作が速くなったとは評価しない。パン・ズームのcommit数の削減も意図していない。ズームはsmooth zoomが停止する時刻の違いでcommit数が少し異なるため、count差を最適化の成果としない。

## 残存する処理

表示フレームに反映する段階ではAppは再実行される。スナップ探索・建設候補・操作ガイド・サイドパネルの組み立ては依然Appにある。ドラッグ・パン・範囲選択は同期で処理するため、これらのイベント数そのものは削減していない。World全体の置換で参照が変われば、変更のない部分も再描画する。

次の改善候補は、ブラウザでのスナップ探索とガイド・パネル更新の計測、編集用UIの所有範囲の縮小、表示範囲による絞り込み。データ全体の深い比較は行わず、実測で優先順位を決める。

## 検証結果

- `npm test -- --maxWorkers=2 --reporter=dot`: 178ファイル成功、2ファイルskip。1117テスト成功、3テストskip。opt-inのProfilerは別途前後で各1回成功。
- `npm run build`: TypeScript型チェック・本番ビルド成功。500 kB超のチャンク警告は残る。
- `git diff --check`: 成功。
- 新しい回帰テスト: Reducerの点追加・経路切替・復元・Undo/Redo、20ポインターイベント/フレームに対する1commit、退出時の保留破棄、フレーム前クリックと駅配置Enterの座標精度、検証入力の再利用。
- 既存テストで、計画の保存・復元、履歴境界、線路種別・接続、駅・路線・系統、パン・ズーム、道路追従、ドラッグ、削除を検証した。表示タイミングが変わる箇所のみフレーム待機へ更新し、同期操作の即時検証は残した。

Rust側のコード・APIとWorldの保存形式は変更していない。実ブラウザでの目視確認とRustテストは今回実施していない。
