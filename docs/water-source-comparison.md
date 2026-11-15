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
|OSM由来water-polygons|海・大洋の面。内水面を含まない|ODbL、OSM attribution|coastlineから生成。自動修復に失敗すると更新停止。分割面には重なりがある。湖沼・河川は別途必要|世界原典取得と高松切出し・reader検証済み。全国加工・内水面融合は未実施|

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
全国全セル生成、GSI原典取得、MLIT/OSM間の融合、独自の世界海岸線組立は未実施。

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

## 続報: 分割生成・海域面取込と道路側境界

`grid.py` は1地域PBFを1回読み込み、最大256の明示セルへ分配する。
実四国PBFから高松4セルを生成し、catalog pinと各manifest/payload hashを確認、
全セルをTypeScript readerで読み込めた。境界重複込み395観測であり、
全国生成完了や実世界の水域網羅を意味しない。自動全国partition、再開処理、
地域の版整合、extract境界の保証、アプリcatalog loaderは未実装。

`marine.py` は由来・ODbL条件を確認したOSM water-polygons専用。
EPSG:4326の既成面を小領域へ切り出し、島の穴を保持する。raw ringの閉鎖と
Shapefile変換時の穴→外周昇格を検査し、自動修復を拒否する。
世界の海岸線をこの実装で組み立てたわけではない。初回の配布ZIP接続は
タイムアウトしたが、再接続で取得成功。高松の実面を切出しreaderで検証済み。
同データは[海・大洋のみで内水面を含まず、分割面は重なる](https://osmdata.openstreetmap.de/data/water-polygons.html)。

道路packの既存文書ではcoverageを「入力snapshotの対象道路を抽出した範囲」と
している。水域には未組立面・幅不明河川・内水面不足があるため、それをそのまま
`water_ready`へ転用しない。既存共通型の`water_coverage`へ渡せる認定範囲は
今回も空であり、App/hookの切替は行っていない。道路担当への提案契約は、
「証拠は表示・検査用に保持可能、readyはfalse、空観測も不明、sourceの版/hash/条件を保持」。
新しいcatalogやmarine packを道路担当が取り込む合意はまだない。

朝の評価時に実用可能なのは、取得済み地域データの面・線・穴を失わず保管し、
複数セル生成と改ざん・欠損検査を再現する基盤。全国で建設可否を完結する機能は
未達で、完全性の根拠がない領域を許可へ緩める変更は含まない。


## 実データ続報: 全国原典・海域時刻・狭い河川

全国8地域のGeofabrik `261114` PBFが揃った。提供元MD5との転送照合、SHA-256、
全PBF内部時刻の一致を検証した。2,552,220,912 bytes、内部時刻は全て
2026-11-14T21:20:41Z。`source_set.py` は欠落地域・重複・別版・改変・検証中差替えを
拒否するが、入力receiptの由来認証や実世界の完全性は証明しない。
[入力台帳](water-source-set-20261114.json)自体もunknown・空coverage。
全国原典を取得できたことと、全国全セル生成・ゲーム統合は別である。

海域ZIP（908,035,793 bytes）も実取得し、内部READMEのデータ時刻は
2026-11-13T15:00:00Zと確認。HTTP更新時刻2026-11-14T01:59:53Zや内水面PBF時刻で
上書きしない。高松で1面893座標の切出しとreader検証成功。版差を保持した別packであり、
同一snapshotの海域/内水面統合や陸地認定は行っていない。hashと利用条件はREADME参照。

[OpenMapTiles schema](https://openmaptiles.org/docs/schema/)のwaterは面、waterwayは線で、
covered=yesの水面除外とzoom別の線選別がある。ただし今回の既存自動取得は
`automaticWaterData.ts`のGSI PMTiles・WAだけを使用し、OpenMapTilesのwater/waterwayを
読んでいない。GSIのRvrCLも現行decoderでは扱わない。schemaのリスクと、実装における
確認済み欠落を混同しない。

高松の実OSM packにはwaterway-line 285件。幅タグなし271件、数値タグあり14件。
トンネルtagはculvert 91件、yes 43件、no 1件、未記載150件。coveredタグは全件未記載だが、
これは地上水面である証拠ではない。具体例は `way/408475615` のdrain/width=2と、
隣接する `way/408475616` のcanal/width=2/tunnel=yes/layer=-1、清水川
`way/1151252894` のstream/tunnel=culvert/幅未記載。原タグ・座標はpackに保持される。
この地点のGSI実タイル比較は取得timeoutとなり、WAに欠落するとまでは断定していない。
数値の幅tagも全区間・現況・水面外周を保証しないため、任意bufferやcoverage昇格に使わない。

一部の元タグはKSJ2出典を記録していた。今回の直接入力はGeofabrikがODbLとして配布する
OSM PBFであり、W05の新規直接取得・混入はしていない。[OSMの過去import記録](https://wiki.openstreetmap.org/wiki/Import/Catalogue/Japan_KSJ2_Import)
は2012年の確認と2015年の条件変更を区別している。元タグを保持し、現在のW05利用条件を
OSMへの新規import許可として読み替えない。

## 全体CIの順序依存

HEAD 62626adのCheckは3OS成功。前HEADのmacOS失敗はConstruction.test.tsx:113の
ホーム寸法変更通知assert。独立再現では、遅いlist_save_slots→autosave_statusが
コマンド無指定のmockImplementationOnceを消費し、寸法変更commandが既定mockで
失敗した。command名を限定したmockでは同じ順序で成功（再現2件成功）。
実CIの正確な呼出順は記録されていないため同一原因と断定せず、再現可能な競合として
[担当者へ修正案を共有](https://github.com/Mei8n/rosenfu/pull/139#issuecomment-6897302504)。
既存Construction.test.tsx/Appの変更は行っていない。

直島周辺の実海域probeではsource split境界を跨ぎ、3地物→4面・穴30をreaderまで保持。
重複面積が正であることも実計測し、split面を単純加算できないことを確認した。
海域/陸域境界を独自に修復せず、重複と元形状を保持する。穴の外周化を拒否する
テストだけでなく、実島の穴保持も検証できた。ただし個々の海岸線の現況は未検証。

日本8地域の全PBFを読み取り、札幌・仙台・東京・父島・名古屋・大阪・広島・高松・福岡・
那覇の10地点で計1,247観測のpackを生成。全てTypeScript readerのpin/hash検証成功。
中国地方原典の `relation/20045092: invalid-area` は広島packにも保持し、完全性を
主張しない。[各地点のbounds・件数・hash・issue](water-live-probes-20261115.json)を公開。
これは10地点の生成検証であり、全国全セルの生成完了を意味しない。
