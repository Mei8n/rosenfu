# 自動取得した実在線路に沿う（schema 22 候補）

「実在の路線網に合わせる」をONにすると、日本の参照graphをworkerで自動取得する。手動bundleは不要。既に手動bundleを読み込んでいる場合は従来の手動方式を優先する。点線表示後に追従を開始し、12画面px以内の起点・終点をクリックして接続経路を確認し、計画へ追加してから着工する。点線はユーザー所有設備ではない。

## 独立実装と供給元

TrainsWorld (`circularsquare/trainsworld`) の `map/realNetwork.ts`, `map/realSnap.ts`, `workers/realRoute.worker.ts`, `game/realGraph.ts`, `pipeline/realgraph.py` の表示／graph分離、明示節点接続、worker処理、公開バイナリ形式を調査した。コードはコピーしていない。Rosenfu独自の厳格デコーダ・有界取得・最短経路・形状保持を実装している。元コードのライセンスはAGPL-3.0だが、本変更には元コードを収録しない。

2026-11-15に以下を実通信で確認した。

- 公開ページ https://anita.garden/trainsworld/ の `tw-data` meta は `https://data.anita.garden/trainworld/2026-11-13/`。リポジトリ `trainworld/data-version.txt` も一致。
- `realgraph/index.json` に `jp` が掲載されている。
- `realgraph/jp.rgr` はHTTP 200、圧縮2,809,138 bytes、展開4,182,250 bytes、99,284 junctions、104,856 chains、336,080 vertices。CORS `Access-Control-Allow-Origin: *`。
- 日本graphの全chain端点はjunction座標と一致。ゼロ長chainが1本あり、それだけ表示／探索から除外する。座標の近接による代替接続はしない。

固定URLは推測していない。この版をアプリ設定に固定し、更新時にはmeta・index・形式・出典条件を再確認する。manifestと日本rail-only graph以外は取得しない。全国graphを1回取得する方式であり、長距離／複数国の動的region探索は未対応。停止・world読込ではworkerをterminateしてfetch／探索と古い結果を破棄する。通常保存では選択中の起終点を保持する。

## データ利用条件

供給者の [README](https://github.com/circularsquare/trainsworld/blob/main/README.md) は公開版データの直接参照・CORSを案内し、多数ユーザー時には自前配信を求める。[NOTICE](https://github.com/circularsquare/trainsworld/blob/main/NOTICE.md) はデータをODbLとして提供する。`pipeline/realgraph.py` のrail graph入力はnoritetsuのOSM extract・line modelである。境界／統計等のpackにある追加条件をrail graphへ推測適用せず、それらのpackを一切コピー／配信しない。

地図・出典UIに © OpenStreetMap contributors と [ODbL利用条件](https://www.openstreetmap.org/copyright) を表示する。供給元独自の選別・3m簡略化・1e-6度量子化済みのgraphであり、生OSM wayを完全再現するものではない。出典情報は真正性、OSM全路線の網羅性、土地利用権を証明しない。派生データやワールドを公開・再配布する際は [ODbL全文](https://opendatacommons.org/licenses/odbl/1-0/) に従う必要がある。本変更は全packの追加権利を精査したという意味ではない。

Overpassは本環境で406／タイムアウトになったため利用しない。OSM編集APIは読み取り専用プロジェクトを目的としていない [運用規約](https://operations.osmfoundation.org/policies/api/) のため代替配信元にはしない。

## 接続・高さ・形状

同じgraph内の明示junction indexのみで接続する。交差・近接・同じ座標の別junctionは接続しない。元chainの全頂点と起終点補間点をそのまま保持し、平滑化・曲線フィッティングはしない。点線・preview・計画・着工payloadは同じ点列を使う。ゼロ長chainは描画・探索から除外する。

経路は双方向の最短長で、運行経路ではない。分岐器の通過方向、駅ホーム、営業線／保守線、曲線規格、信号、運行権を保証しない。OSM layerやbridge/tunnel flagsをゲームの6m階層へ変換しない。ゲーム内高さ・線路種別は通常の計画設定が決める。最大256 chain・4096形状点、探索上限200,000節点。現在は日本graphだけなので隣接国との接続は行わない。

取得状態はunknown/loading/ready/failedを分離する。readyは日本graphの取得完了であり、任意地点の完全coverageを保証しない。近傍候補なし、経路なし、画面表示上限、取得失敗は別メッセージとし、実世界の線路不存在とは断定しない。低zoomや2,000 chain超の表示では選択を禁止し、拡大を要求する。

## 保存形式

World schemaは **22**。schema1〜21は読込前に非対応として拒否し、元ファイル／manual／autosave／backupを上書きしない。移行処理はない。

新出典方式 `rail_graph_route_v1` は `network_path: [{region, chain_id, from_fraction, to_fraction}]` を保持する。割合は供給chain上の累積距離で、OSM segment番号ではない。元OSM way IDが供給データにないので `osm_way_id` は空、旧way-slice位置欄は0とし、UIに「未収録」を表示する。旧方式とnetwork_pathの混在、偽way、範囲外割合、空経路は拒否する。`SourceRecord.conversion_format_version=2` と方式の一致を検証する。graph SHA256は展開した原データ、source SHA256は供給元・公開版・地域・形式・URL・license・attributionの固定順JSONのハッシュ。公開版日付をOSM資料日と偽らず `source_date=null` とする。

参照shapeを計画に追加した後は、既存の未着工保存と着工処理を再利用する。選択中の起終点はセッション内だけで、まだ保存されない。Undo/Redoは保存したchainと位置を再利用し、パン後に再スナップしない。

## 統合境界

PR135/136の道路・水域コードは取り込まない。Appの追従入口・MapCredits・Tauri CSPは統合時に競合し得る。道路・水域の取得状態もunknown/loading/ready/failedと「取得済みだが該当形状なし」を分けることを推奨する。地理座標とゲーム高さは別型／責務とし、出典付き確定点列を計画に渡す境界を共通化する。rail graphを環境・道路・所有線路の物理接続証拠として再利用しない。

CSPは確認済みrail-only公開版ディレクトリだけconnect-srcへ追加し、module workerはselfのみ。別担当の配信先許可と併合する際はどちらも保持する。横断統合は本PR単体の検証と別に必要。

## 検証コマンド

- `npm test -- src/realRailGraph.test.ts src/useAutomaticRail.test.tsx src/ReferenceCopy.test.tsx`
- `ROSENFU_REAL_RAIL_FIXTURE=<downloaded jp.rgr> npm test -- scripts/real-rail-live.test.ts`（データ本体をgitへ入れない）
- `npm test`, `npm run build`, `cargo fmt --all --check`, `cargo test --locked --workspace`
- draft PRのUbuntu／Windows／macOS CI。CIは外部配信の実通信を模倣しないので実データ検証は別途行う。
