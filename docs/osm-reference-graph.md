# OSM交通参照グラフとスナップの設計案

調査日: 2026-11-12。実装確認基点: `10a2013`。

本書は最新実装と[既存のオフライン地図調査](offline-map-research.md)を突き合わせた設計提案であり、機能実装・データ取得・方式採用・配布許諾の完了を意味しない。今回、公開サイトの再取得や実データの生成・容量計測は行っていない。以下のアルゴリズムと境界条件は提案であり、実OSMデータでの検証が必要。

## 1. 境界と現在地

背景表示、OSM交通参照グラフ、ゲーム設備の3層を分ける。

- 背景画像/ベクタータイルは簡略化、境界分割、属性省略があり得るため、接続の原本にしない。
- 現行 `Alignment` は独立ID・所有者・名前・緯度経度列。高度、線路本数、OSM由来情報は未保持。点数2〜4096、隣接点間1cm以上、全長20m以上の制約がある。
- **OSM原形を `smoothAlignment()` に渡さない。** 現行関数のCatmull–Rom補間は参照線路からはみ出す可能性がある。手描きの制御点補間と、参照形状の区間抽出・直接保存を別経路にする。OSM由来形状も精度が保証された土木線形ではない。
- 既存Route候補APIはゲーム内Track/Alignment上の駅を探索するもの。OSM参照グラフの探索と混ぜない。
- 現行参照駅はローカルJSON＋IndexedDB、Repository抽象あり。鉄道/道路スナップ、交通グラフ、実データは未同梱。
- 現行World保存はスキーマ4。新たな由来情報等を永続化する場合、既存の厳密な読込契約に合わせたスキーマ設計・移行試験が必要。

## 2. 地域パックと保存方式

想定パックは、背景アーカイブ、交通参照DB、表示資産（スタイル・アイコン・日本語フォント）、manifest、ライセンス文書を別々に持つ。manifestには地域範囲、原典日時、形式/生成器版、ファイル容量・ハッシュ・出典を含める。PMTiles等の正式採用は未決定。

交通DBの概念テーブル:

- `datasets`: 地域・版・由来。
- `osm_elements`: 元要素種別とID、必要な元タグ。
- `graph_nodes`: グラフ端点と位置、元node。
- `edges`: 両端、交通種別、長さ、方向属性、階層属性、品質/欠損フラグ。
- `edge_geometry` / `edge_source_spans`: 形状と元way内区間、逆向き走査の対応。
- `reference_stations`: 参考駅。ゲームの駅とは独立。

OSM IDは要素種別と組み合わせて保持し、JavaScriptの安全整数を超えないよう文字列として受け渡す。

**SQLiteはRDBの一種**であり「RDBかSQLiteか」の二者択一ではない。

| 方式 | 用途・注意 |
| --- | --- |
| JSON＋メモリ索引＋IndexedDB | 小地域fixture/試作に適する。大量データは全体読込・メモリ使用が課題 |
| 地域別読取専用SQLite＋RTree | デスクトップの部分読込、空間検索、参照整合性に有力。Rust側からの検索を想定 |
| サーバー型RDB | 共同編集/配布基盤など別用途。シングルプレイの必須依存にしない |

RDBへ入れるだけで経路探索が実装されるわけではない。隣接リスト・探索器・空間候補検索を分ける。ブラウザ版のSQLiteはWASM/OPFS等の別検証が必要なのでRepository契約で実装差を吸収する。World全体のSQLite移行を先行条件にはしない。

## 3. 抽出と接続

ゲーム起動時ではなくパック生成工程で地域PBFから抽出する。Osmium等による参照整合性を保つ抽出を候補とし、境界バッファや境界外への接続情報を検証する。道路・鉄道wayの参照nodeを欠落させない。

- 共有node、分岐、重要タグの変化、抽出境界で辺を分割する。
- 座標上の交差や近い端点だけで接続しない。元node共有を検査し、交通モード・階層矛盾も評価する。
- 鉄道と道路が共有する踏切nodeを、道路から鉄道へ移動できる探索辺にしない。接続グラフと交差/干渉情報は別。
- `bridge`、`tunnel`、`layer`、`level`等を保存。未知値を地上と確定しない。`layer`は相対上下関係で実メートル高度ではなく、`level`や標高とも異なる。ゲームの地上±3段へ機械的に対応づけない。
- 立体交差、タグ矛盾、抽出境界、参照不足は状態として残し、接続可能/不可能の断定を避ける。
- 廃線・建設中・側線などの分類は保持し、表示/探索対象の既定値とユーザー選択を別途決める。

### 方向属性

元wayのnode順序と運行上の方向は別物。元方向、片方向タグ、モード別制約、逆向き探索時の形状順を明示的に扱う。方向不明を無条件に双方向運行可能と断定しない。

建設形状の追従は自動車/列車の運行経路案内ではない。道路のonewayやturn restrictionを無条件に建設禁止へ流用せず、元情報を保持して、形状追従用の走査規則と将来の運行規則を分ける。鉄道の方向・分岐・信号・容量・運行権限の正確な再現は本段階の対象外。

### 複線・道路・並行建設

複数軌道が別wayの場合と属性で表現される場合があり、近い並行線を自動統合しない。初期は一本の参照形状を選び、ゲーム側の単線/複線/複々線設定を独立させる。ただし選んだ形状が線路群の中心とは限らないことを表示する。

道路中心線は車線ごとの軌道位置・幅員・建設可否・法的権利を保証しない。「道路に沿う」は形状補助とする。「既設ゲーム線路の横に沿う」はWorldのAlignmentを使う別モードでありOSMの複線判定とは区別する。

## 4. 候補探索と建設操作

1. マウス付近の辺を空間検索し、メートル距離、進行方向、モードで候補を提示する。
2. 始終点を辺途中へ投影。探索用の仮ノードを作るが元DBは変更しない。
3. 初期は同じ辺/way上の区間追従、その後に複数辺の有界探索を追加する。
4. 分岐は経由点指定・候補切替で選ぶ。候補が複数ある場合に無言で確定しない。
5. プレビューは読取専用。明示確定時だけ、検証済み形状を独立したゲーム設備へコピーする。

探索には距離、訪問辺数、候補数、処理時間等の上限を設ける。具体値は小地域で計測して決める。上限到達を `incomplete` として区別し、候補0件でも「不通」、1件でも「唯一」と断定しない。範囲外・パック未導入・欠損・切断・曖昧・中断をUIに返す。既存Route探索の上限をそのまま転用する決定ではない。

OSM欠落時に「現実に線路なし」と判断したり、切断を勝手に直線で橋渡ししたりしない。手描きへ明示切替でき、参照区間と手描き区間の境目を確認できるようにする。

4096点を超える形状を黙って切り捨てない。初期は明示拒否/短区間への分割操作を案内する。将来簡略化する場合、分岐や橋/トンネル境界等を保護し、許容誤差と接続不変を検証する。簡略化後に一般スムージングをかけない。

## 5. 更新時に既設設備を変えない

ゲーム設備は独立IDと確定時の座標列を保持する。任意の由来メタデータとしてパックID/版、元way・区間、生成器版を記録する案とする。

新パック導入で更新するのは参照表示と今後の建設候補。既設線路・駅・路線を自動移動/削除/再接続しない。旧パックがなくても既存Worldの表示・保存・ゲーム処理は継続でき、該当参照機能の不足のみを明示する。起動時更新を必須にしない。破損パックは検証段階で拒否し、有効な現在版を保持する。

## 6. 小さく実装する順序

1. 合成fixture、検証器、Repository、近傍投影。直線/分岐/立体交差/複線/欠損/方向反転をネットなしで試験。
2. 参照線表示、同一way区間追従、緑の候補線、手描き切替、Alignmentへの確定。元形状から逸脱しないことを検証。
3. 接続探索、候補/経由点、上限と不完全状態、境界欠損表示。
4. 小地域PBF変換とSQLite比較。サイズ、ピークRAM、検索時間、境界参照を実測。
5. 別工程で実地図背景パックを導入。フォント等を含め、キャッシュに頼らないネット遮断試験。

自動立体化、勾配、線路本数の物理モデル、踏切費用、列車競合判定は別段階。形状追従だけでそれらまで対応済みと表示しない。パック更新で既存Worldが不変であること、参照パックなしで保存再開できることも回帰試験に含める。

## 7. 出典・ライセンス・未確認事項

公開資料の根拠は[既存調査](offline-map-research.md)に集約。今回の提案は同資料の2026-11-12調査を利用しており、下記の再取得・法律判断は行っていない。

- [Geofabrik日本](https://download.geofabrik.de/asia/japan.html): 地域PBFの入手候補。
- [Osmium](https://osmcode.org/osmium-tool/manual.html): 地域/タグ抽出・整合性確認の候補。
- [Protomaps](https://docs.protomaps.com/basemaps/downloads): 背景アーカイブの候補。
- [OSM標準タイル利用方針](https://operations.osmfoundation.org/policies/tiles/): 標準画像タイルの一括保存は採用しない。OSMデータの取得と配信タイルの利用規約は別。
- [OSMライセンス案内](https://www.openstreetmap.org/copyright)、[帰属指針](https://osmfoundation.org/wiki/Licence/Attribution_Guidelines)、[Produced Work指針](https://osmfoundation.org/wiki/Licence/Community_Guidelines/Produced_Work_-_Guideline): ODbLの帰属に加え、抽出/加工DBの配布条件を実構成に即して確認する。別ファイル化だけで義務が消えるわけではない。ゲームコード全体が自動的に同ライセンスになるとの断定もしない。

既存調査のPBF容量は元データの参考値であり、完成パック・SQLite・必要RAMの実測値ではない。全国取得を先行条件にせず、小地域で背景・DB・表示資産を含む容量と更新費用を計測する。

タグの地域別品質、鉄道ネットワーク完全性、スナップ精度、OSM方向タグの採用範囲、SQLite/PMTiles性能、Windows実機、ネット遮断、法的最終確認は未実施。背景・交通DB・スタイル・フォント・追加素材にはそれぞれ別の条件があり得る。

## Session-only reference line display

The game now accepts the converter's three files together: `graph.json`, `source.json`, and `complete.json`. The completion manifest and both SHA-256 hashes must validate before replacing the current pack. This checks bundle integrity, not authenticity, geographic accuracy, completeness, legal permission, or real-world traversability. Invalid/partial replacements keep the current pack; removing the pack or closing the component invalidates in-flight reads. No network transmission, World mutation, or persistence is performed.

At zoom 14 and above, rail is a purple solid line and road a blue dashed line, with a textual legend. The viewport uses edge bounding-box intersection, including segments whose endpoints lie outside the view. At most 1,500 segments are rendered in source order, and omitted counts are shown. Two SVG paths represent the two modes, with pointer events disabled below game infrastructure. Every segment is the original endpoint-to-endpoint straight segment, not smoothed. View indexing is memoized independently of construction hover; a view query scans at most 40,000 indexed edges. False-positive bounding-box overlaps can consume the display budget and are clipped by the viewport.

This is not construction snapping, network routing, automatic track building, or an offline background map. Unknown layer/bridge/tunnel metadata remains unknown; the layer does not claim to show grade-separated connectivity. Attribution and provenance are displayed. Real-world maximum-size bundle UI responsiveness and Windows WebView interaction have not been measured; the 5 MiB graph and 20 MiB source validation can temporarily block the main thread, and a worker/spatial index may be needed later.

External provenance text (including source date) is displayed at a maximum of 200 JavaScript string code units per field, with an explicit truncation marker. The reader size/validation contract and original metadata are unchanged; full text is not copied into DOM attributes.
