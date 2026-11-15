# 全国水域の元データ比較とpackの現在地

2026-11-15 UTC調査。全国で入手できることと、建設判定に必要な全水面が
完全に含まれることは別。今回の実装は、観測された面・河川線・海岸線を
検証可能なpackに保存する段階であり、全国の陸地認定や着工可能範囲は増やさない。

## 候補比較

|提供元|全国範囲・形状|利用条件|完全性・無データ・狭い河川|今回の扱い|
|---|---|---|---|---|
|GSI基盤地図情報・基本項目|全国、2次メッシュ。海岸線・水涯線等のGML。都市計画区域は1/2,500、その他は1/25,000相当|GSIコンテンツ規約。基本測量成果であり利用方法によって測量法の申請が必要|全国整備は全水面polygonの提供保証ではない。線から面への復元、メッシュ接合、対象地物・取得基準の確認が必要。欠落を陸地にしない|登録制ダウンロードのため未取得。アカウント操作や代替認証はしていない|
|GSI最適化ベクトルタイル|WA面、RvrCL河川中心線、Cstline海岸線等|GSIコンテンツ規約、出典・加工表示等|描画用の簡略化・微小面削減がある。WA省略と完全な陸地は同義でない|PR137は取得復旧。完全水域の元データとしては未採用|
|国土数値情報W09湖沼|閲覧できたW09-05は全国の対象湖沼・貯水池の面、2005年度作成|当該ページは「商用可」。共通PDLを一律適用せず、この版の旧利用約款を確認する|品質の漏れ0%は指定された湖沼原典との比較。全ての池・河川・海を意味しない|比較候補。今回packに混在させていない|
|国土数値情報W05河川|全国の河川中心線・端点。掲載版は地域別2006–2009年度|当該ページは**非商用**。一般PDLや他データの条件で上書きしない|面幅を復元できない。古い版であり、全ての現在の水路を保証しない|ゲーム配布データとして採用せず、仕様比較のみ|
|OSM/Geofabrik日本・地域PBF|日本全体と地域別の生のnode/way/relation。water面、waterway線、coastline線|ODbL-1.0。帰属とデータベース派生物の条件をコードと別管理|抽出範囲は原データの範囲であり実世界の完全性ではない。未登録水域、欠損relation、幅不明・間欠・地下等を保持する必要|既存四国PBFから高松の小地域packを実構築|
|OSM由来water-polygons|海・大洋の面。内水面を含まない|ODbL、OSM attribution|coastlineから生成。自動修復に失敗すると更新停止。分割面には重なりがある。湖沼・河川は別途必要|将来の海域面候補。世界データの取得・加工は未実施|

出典:
- [GSI基盤地図情報の仕様・登録・利用条件](https://service.gsi.go.jp/kiban/app/help/)
- [GSI最適化ベクトルタイル](https://github.com/gsi-cyberjapan/optimal_bvmap)
- [W09-05湖沼の対象範囲と利用条件](https://nlftp.mlit.go.jp/ksj/jpgis/datalist/KsjTmplt-W09.html)
- [W05河川の年代・形状・非商用条件](https://nlftp.mlit.go.jp/ksj/gml/datalist/KsjTmplt-W05.html)
- [国土数値情報の利用規約と個別条件の優先](https://nlftp.mlit.go.jp/ksj/other/agreement.html)
- [Geofabrik日本](https://download.geofabrik.de/asia/japan.html)、[四国](https://download.geofabrik.de/asia/japan/shikoku.html)
- [OSMのライセンス](https://www.openstreetmap.org/copyright)
- [海のwater-polygons](https://osmdata.openstreetmap.de/data/water-polygons.html)、[coastline生成処理](https://osmdata.openstreetmap.de/processing/coastline.html)

## GSIのWA省略について得られた証拠

実PMTiles metadataはWAのZL4–16を記録するが、タイルごとの完全性maskはない。
生成器記録は `tile-join v1.36.0`、生成オプションに `-pf -pk -S 2` があり、
微小面削減を止める `-pt` はない。記録と同版の
[生成器資料](https://raw.githubusercontent.com/mapbox/tippecanoe/1.36.0/README.md)
は微小polygonの一部省略・四角化を説明する。`-pf/-pk` は完全な形状保持を意味しない。
実際のTippecanoe本体バイナリの版までは検証していない。
[生成処理](https://raw.githubusercontent.com/mapbox/tippecanoe/1.36.0/tile.cpp)
は地物数0のレイヤーを省略する。省略は正常でも起こり、障害とも陸地とも断定できない。

富士山 `14/14505/6469` とZL15の4子タイルはいずれもWAなし。
河口湖内 `(138.754,35.514)` はWA `5200` とAdmArea `1104` の両方に含まれた。
よって行政区画の包含は陸地証明ではない。親子タイルの多数決でも入力段階の
欠落を排除できない。生成器の全tileサイズ上限による省略は、WAだけがない
有効タイルとは別の問題として扱う。

## 実装したpack契約

- 新規 `scripts/water-pack/` と `src/waterEvidencePack.ts` に分離。
- 面の穴・元タグ・地物IDを保持。河川線を任意幅で膨らませず、海岸線から海を推定しない。
- source hash、PBF header、payload hash、manifest pinを記録・検証する。
- `geographic_completeness=unknown`、`coverage=[]` を強制する。空結果でも不明。
- 自己申告の `complete` や非空coverageを、正しいhash付きでもreaderが拒否する。
- 複数面のrelation組立に失敗しても、pyosmiumがcallbackを出さない場合がある。
  エラー一覧が空でも完全性を主張しない。
- App/hook、session、共通型、物理判定、既存の自動水域取得は変更していない。

高松の実pack生成・読込は成功。67水域地物、70面、穴21、河川線285、海岸線21。
[再現コマンド・hash・制限](../scripts/water-pack/README.md)を参照。
全国一括生成、GSI原典取得、MLIT/OSM間の融合、世界海岸線処理は未実施。

## 陸地を認定できるpackへ進む条件

今回の証拠packだけでは、正常な陸地を着工可能にする機能は完成しない。
必要なのは、版・対象地物・精度・有効範囲が明確な元データと、海・湖・河川を
含む抽出・接合・欠落検査である。粗い土地被覆、標高、行政界、画像の色は代替しない。
狭い河川の幅が不明な場所や、海岸線が閉じない場所は不明領域として残す。
認定可能な領域だけを別の検証済みcoverage契約として発行し、未提供・読込失敗と
認定された空水域を区別する必要がある。manifestにcompleteと書くだけでは不十分。

GSI登録データの取得と利用手続確認、または品質根拠のある別の原典が次の実データ作業。
配布・統合時に求める精度や対象水域を変更する場合は、担当間で契約を合意してから
readerのunknown-only制約を変更する。現時点で地上敷設禁止や30km規則の判定を
このpackで許可側に緩めない。
