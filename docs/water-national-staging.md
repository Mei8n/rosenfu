# 日本8地域の水域stagingと版を保持した複合pack

この工程は日本8地域の入力PBFにある対象観測を保存し、必要な小範囲を再抽出する
ためのオフライン基盤。実世界の水域網羅や建設可能な陸地を保証しない。
`coverage=[]`、`geographic_completeness=unknown`、readerの`status=incomplete`を維持する。
App/hook/session/共通物理判定への接続は含まない。

## ディスク上の全地域staging

`scripts/water-pack/stage.py stage` はpin付きsource-setを読み、各entryの地域・日付URL・
SHA-256・bytes・内部時刻・ODbL契約を検証する。PBFのhashを読み取り前後で確認し、
選択対象の面・河川線・海岸線をSQLiteへ保存。最後にDB hashを含むmanifestを発行する。
manifestなしの部分生成は利用不可。別地域の成功だけで全国完了にはしない。

```
python scripts/water-pack/stage.py stage hokkaido-261114.osm.pbf \
  --source-set docs/water-source-set-20261114.json \
  --source-set-pin 5a57533706f1d2accd30c93787df95e575eb18c91d3f36cb2466e22cf9c0304e \
  --region hokkaido --output new-hokkaido-stage
python scripts/water-pack/stage.py extract new-hokkaido-stage \
  --pin STAGE_MANIFEST_SHA256 --bounds 141.33 43.04 141.37 43.08 \
  --output new-sapporo-evidence
```

抽出はDB hashを前後照合し、読み取り専用のbbox queryから既存leaf契約へ変換する。
元の全形状・穴・タグを保持し、線を任意の幅へ膨らませない。元PBF全体の再走査は不要。
source issueは位置不明の場合もあるため、抽出範囲が狭くても引き継ぐ。
空queryは不明であり陸地ではない。元のsource-set pinやDB pinは独立に保管する。

上限は入力768MiB、1観測100,000座標、地域500,000観測/12,000,000座標、
観測body合計384MiB、**完成DB512MiB**。抽出leafは従来の5MiB/10,000観測/
100,000座標/0.2度制限。完成DB上限は最大disk使用量ではない。位置索引とjournalが
別に必要で、実中部の位置索引だけで約1GiB使用した。十分なworkspace空きと
TMPDIRを確保し、地域は逐次処理を推奨する。

全国stagingのみ位置索引を`sparse_file_array`へ配置する。既存地域builderの
既定`flex_mem`は変えない。成功時にその実行が作成したlocations.idxを除去するが、
異常終了時のcache・部分DBは残る。既存outputは上書きせず、他担当や復旧用データを
削除しない。未完ディレクトリを再開して完成品とみなす機能は未実装。

初回2並列の中部・中国がstderrなしで異常終了し、manifestなしとして保持した。
原因を記録できていないためOOMとは断定せず、disk索引・逐次の新出力へ再試行し、両地域とも完了した。

## 海域と内水面の複合pack

`compose.py` は両partのpin・payload hash・leaf schemaを検証し、同一要求boundsの
partだけをまとめる。元bytesを変更せず、各source/revision/issueを別々に保持する。

```
python scripts/water-pack/compose.py \
  --inland inland-pack --inland-pin INLAND_MANIFEST_SHA256 \
  --marine marine-pack --marine-pin MARINE_MANIFEST_SHA256 \
  --output new-composite
```

`readWaterEvidenceComposite(indexBytes, indexPin, files)` は全partを既存readerで厳密に
読み込む。欠損partを飛ばさず全体を拒否し、呼出側bytesをasync処理前にsnapshotする。
最大16part/32files/20MiB、合計20,000観測/200,000座標。partごとの版の主張が異なると
`hasMixedRevisionClaims=true`。同じ文字列でも実測日が同じと保証するものではない。

これは出典を保持する融合表示・検査用の構造で、geometry unionや物理評価ではない。
`observedAreas`はsourceKey付きで保持し、split重複や地域重複を削除しない。
重複した面や長さを単純加算して30km判定に使わない。既存EnvironmentSnapshotとして
そのまま渡せる型にはしていない。幅不明河川や未観測水域は解決されない。

実高松では内水面70面と海域1面を読込成功。内水面2026-11-14T21:20:41Zと
海域2026-11-13T15:00:00Zを区別し、mixed=true、coverage空のまま。
composite pin: `b389089557600816c4980686bb53b11fadb2242bb1e18fe35d65cbee918ff885`。
workspace: `/workspace/shared/rosenfu-water-pack-artifacts/takamatsu-composite-v1`。

## coverageへ昇格できない残件

|確認事項|現在の意味・制限|
|---|---|
|原典bytesと版|8地域のhash/内部時刻一致を確認。実世界網羅とは別|
|地域scan終了|有効な完成manifestで確認。全candidate relationの組立保証ではない|
|面の欠落|pyosmiumのcallbackなし欠落があり得る。issuesが空でも完全ではない|
|狭い河川|線は保持するが水面幅不明。数値widthタグも現況外周の保証ではない|
|地下・覆蓋・季節性|元タグを保持。地上の水面とも乾いた陸地とも推定しない|
|海域との整合|別版を保持。coastline修復による古さとsplit重複が残る|
|建設許可|新しいready/coverage契約の合意なし。今回の全出力は不明|

全国原典の観測保存・小地域抽出・海域との版別検査は評価可能になるが、
全国の着工可能判定が完成したとは扱わない。入力とデータ利用条件はコードライセンスから
分離し、OSMデータのODbL/attributionを各partに保持する。


## 全国stagingの実生成結果

日本8地域の全入力を走査し、合計1,180,640観測・27,352,637座標を保存した。
完成DB合計1,303,355,392 bytes。これは地域ごとの選択観測数であり、地域境界の
重複を除いた実世界の水域数ではない。中国のrelation/20045092 invalid-areaを保持。
callbackなしの未組立を検出し尽くしたとは主張しない。

全8DBのhashを検証したうえで10地点を再抽出し、原典を直接読んだ旧probeの
payloadとbyte一致、さらにTypeScript readerで全件読込成功を確認した。
[全地域manifest/DB hashと再抽出pin](water-national-stage-20261114.json)を記録。
index SHA-256: `644c8dcc012bfe1d8e55c854299ec205e43405f8d905092df2757e06a1be4181`。

workspace `/workspace/shared/rosenfu-water-pack-artifacts/` 下の
`national-stage-v1`に6地域、`national-stage-disk-v2`に中部・中国の完成出力がある。
最初の未完2地域も残してあり、indexは成功した新出力だけを指す。
`stage-extracted-v1`は10地点の再抽出、`national-stage-index-v1.json`は上記台帳。
全国全セルを事前配布する方式ではなく、全地域stageから小範囲を抽出する方式である。
アプリの自動取得、配信サーバ、catalog切替、着工許可への接続は未実装。

朝の評価範囲は、8地域の全観測保存・任意の小範囲抽出・海域/内水面の版別複合読込。
1回の抽出上限を超える場所は分割が必要で、すべての範囲の成功は保証しない。
全国全域で建設可能とするcoverageや道路側へのready移行は、引き続き未認定。
