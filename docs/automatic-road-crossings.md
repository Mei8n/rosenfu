# 自動道路取得・踏切

## 取得契約

`src/automaticRoadData.ts` は固定 URL `https://overpass-api.de/api/interpreter`
から OSM 道路 way の完全な geometry と属性を取得する。画面接続と固定URLのデスクトップCSP許可を実装。実WebView動作は未検証。

`RoadSnapshot` は要求した `bounds`（取得範囲）、`fetchedAt`、既存の
`ReferenceGraphBundle` を持つ。graphのboundsは境界を跨ぐway全体を収めるため広がる場合がある。
**取得済み判定にはgraph.boundsではなくsnapshot.boundsを使う。**
元の layer/level/bridge/tunnel は GraphEdge に保持する。構造属性の異常は
roadWays.structureUnsupported で保持し、nullへの変換による地上判定を禁止する。
OSM layerは相対的な上下関係でありゲームの6m段階とは同一ではない。

取得は最大0.04度四方、応答5MiB、元way1万件、グラフは既存上限を適用。
同じ範囲の要求を共有し、異なる範囲も直列処理。成功キャッシュは24時間・最大128件、
失敗は提供元全体で60秒の待機。道路リクエスト開始間隔は最低1秒。待機要求も12件に制限する。取得キャッシュはメモリ内。着工時の参照スナップショットはワールドに保存。
タイムアウトやOverpassのremark、欠損形状を空の成功にしない。
有効な空配列は「OSM応答に道路がない」であり実在道路の完全性保証ではない。

## 提供元と制約

- [Overpass公式運用指針](https://dev.overpass-api.de/overpass-doc/en/preface/commons.html)
  は一般アプリの常用バックエンドとして公開インスタンスに依存する運用を問題例とする。
  本取得器は小範囲試作。配布規模での継続提供には専用配信基盤が必要。
- [OSM layer](https://wiki.openstreetmap.org/wiki/Key:layer)
- [OSM level](https://wiki.openstreetmap.org/wiki/Key:level)
- 表示帰属: © OpenStreetMap contributors / ODbL-1.0

水域提供元は国土地理院PMTiles（後述）。道路成功を水域取得成功に流用しない。OSMの局所water polygonや
coastlineがないことは陸地の証明にならない。海を含む水域カバレッジが必要。

## 保持するゲーム仕様

費用計算・請求・維持費は今回は実装しない。踏切1か所1億円は将来の仕様。
高さ倍率の単一定義は別担当のheightCostModelに委ねる。
地上線の水上建設は禁止。高架+1/+2/+3と開削-1は連続水面横断が30km超で禁止、
ちょうど30kmは許容。地下-2/-3はこの距離制約の対象外。未取得を陸地にしない。
道路共線・重なりは横断踏切にしない。非平坦な縦断区間に踏切を置かない。

## 実装した経路（追記）

道路は画面範囲の0.005度グリッドに丸めて自動取得し、既存道路追従・参照形状の
ReferenceGraphBundleへ接続。ユーザーによるファイル読込は不要。取得待ち・範囲外・
失敗を画面表示する。パン/ワールド切替後の古い応答は適用しない。
保存済みEnvironmentSnapshotはオフライン判定に再利用できる。

水域は [国土地理院最適化ベクトルタイル](https://github.com/gsi-cyberjapan/optimal_bvmap)
の現行PMTiles、`WA`水域面レイヤーを取得する。固定URLは
`https://cyberjapandata.gsi.go.jp/xyz/optimal_bvmap-v1/optimal_bvmap-v1.pmtiles`。
z14の最大16タイル、Rangeと展開後データをそれぞれ5MiBまでに制限。
未提供タイルやWA水域レイヤーのないタイルは未取得扱い。行政区画レイヤーのみでも陸地と判定しない。
水域面は穴を保持する。海岸線から海を推測しない。
現段階は地図表現の参照データ精度であり、現地の水際・小水路の完全性保証ではない。
CSPのconnect-srcは上記固定2URLのみ追加。任意URL・wildcardは許可しない。

coreの `EnvironmentSnapshot` は revision、road_coverage、roads（元の構造タグと
polyline）、water_coverage、water（穴を含むrings）を保持し、WorldDraft.environment
として既存保存経路で永続化する。範囲・座標・件数を検証する。
踏切はこの保存済み参照と現在の線路から導出する資産。独立した古い踏切配列や
課金履歴を保持しないため、撤去/Undo/復元で再計算され、再課金は存在しない。
将来費用を追加する際は、この導出資産と請求履歴を分けて設計すること。

`preview_environment(alignments, stations, environment)` と
`build_plan_with_environment(alignments, stations, environment)` は同じ候補構築・判定を
使う。後者は取得不足/高さ不明/水域制約違反時に変更を公開しない。
`automatic_crossings()` は現在の導出踏切一覧を返す。
プレビューcrossingsは既設箇所を除いた新規件数。assessmentは各変更線路の全交差。
線路接続チェーンを合わせて水面距離を確認し、線路分割による30km制約回避を防ぐ。
旧低レベル建設APIは内部・既存試験向けに存続し、新UIはチェック付きAPIを使う。

OSMに構造タグがない通常道路はsurfaceという参照モデル上の仮定を用いる。
明示された橋/トンネル/非ゼロ階層には投影交差だけで踏切を作らず、解釈不能な
構造タグは未知として着工を止める。これは実測標高の一致を保証しない。
共線・接触・接線を除外し、way分割点を扱う。coreでは0.5m以内の同位置候補を統合。

### 統合上の留意点

- この作業基点にはまだvertical_profileが存在しない。
  `assess_alignment_environment(alignment, ground_sections, data)` のアンカー区間別フラグを、
  スロープ統合時に「平坦かつ実高さ0m」の判定から供給すること。
  session/environment.rsのassessment関数が接続箇所。一律trueを使わず、現基点では各区間の地上レベルを確認する。
  区間別Hermite対応は別ブランチ `fix/environment-profile-adapter` に独立させる。
- 計画がある場合は線形に沿う0.02度セルを最大128領域、逐次取得する。
  画面外の長大計画も対象。取得/保存上限を超える計画、欠損、失敗は着工不可。
- 高さ/線路数編集は取得済み環境に対して再検証する。新しいスロープや分岐APIにも
  同じ検証を接続する必要がある。
- 2026-11-15クラウド検証でGSIの実データRange取得と面デコードは成功。
  Overpassは小範囲のPOSTでもHTTP406（nginx）となった。実道路はfixtureで検証。
  実デスクトップWebViewのCSP/CORS経路は未検証。

### 範囲単位の更新

各道路・水域面の任意 `coverage` はその形状版の有効範囲（西・南を含み東・北を含まない矩形群）。
省略された既存保存/取得応答は、その提供元のsnapshot coverage全体を意味する。
再取得は道路・水域を別々に置換する。成功した空応答でも、その取得範囲内の旧地物は消える。
旧版の有効範囲から更新矩形を差し引き、範囲外の旧形状と取得済み状態を保全する。
同一way IDの形状版が空間的に分かれて残ることがある。踏切は有効範囲内だけから導出する。
水域の有効範囲境界も連続水面計算の分割点になる。再分割は地物あたり128矩形、総数10000まで。
上限超過は更新全体を拒否し、旧ワールドを変更しない。revisionは最後に適用した入力の識別子であり、
全領域の同時刻取得や鮮度を保証する時刻ではない。取得応答の適用順は画面/ワールド世代で制御する。

### 回廊と輪環

回廊は4096頂点まで、道路/水域の保存coverageは各128矩形、各10000地物・合計100000座標まで。
道路と水域がすべて成功するまで新しい回廊snapshotを公開しない。古い計画/画面/ワールドの応答は捨て、
残りのセルを予約しない。ロード済みsnapshotは従来どおりオフライン判定に利用できる。
水域タイルのメモリキャッシュは128件・24時間、タイル開始間隔は250ms、失敗後は60秒休止。
輪環は始点を跨ぐ連続水域も調べる。全周30km以下を二重計上せず、陸地や深い地下区間で距離を区切る。
現基点の端点モデルは非分岐チェーン/輪環のみ。曖昧な複数接続を勝手に一経路へ絞らず拒否する。
将来の分岐モデル統合では分岐経路の判定を別途接続する必要がある。

### 2026-11-15 取得経路の再調査

固定Overpassに0.001度四方の1回のPOSTを実施し、HTTP406を再現した。応答はserver: envoy、
HTML本文はnginxの406。ブラウザ外でも失敗するためCSP/CORSだけを原因にしない。
CSPには固定interpreter URLが許可されているが、WebViewからの実取得は未確認。
認証変更・別公開サーバー巡回・User-Agent偽装は行っていない。提供元側の拒否条件は確定できていない。

TrainsWorldは設計参考に限定し、コード/公開packを流用していない。
[OpenFreeMap](https://openfreemap.org/) は商用利用と公共インスタンスを案内するが、
[利用条件](https://openfreemap.org/tos/) では自動収集を制限し、オフライン取得は1回3000tileまたは50MB、
最大4並列、アプリを識別するUser-Agentが条件。無制限なゲーム用データ抽出の許可とは解釈しない。
帰属はOpenMapTilesとOpenStreetMapが必要。表示用z14のtransportationに道路が含まれても、
欠けたlayer/地物を建設可の根拠にする完全性manifestは確認できていない。
[スキーマ](https://raw.githubusercontent.com/openmaptiles/openmaptiles/master/layers/transportation/transportation.yaml)
のlevelはsteps/footway向けの実験属性であり、元道路の全構造タグと等価ではない。
そのため現取得器を黙ってOpenFreeMapへ切り替えていない。
配布用には、利用条件と出典が明確なOSM等の原データから、元属性・非簡略形状・
明示的な完全/欠損coverage・revisionを持つ範囲別packを自前配信する方式が候補。
ホスト/更新運用/水域の完全性データは未用意であり、現在の取得失敗は未解決。
