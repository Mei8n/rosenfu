# 道路タイル提供元の評価（2026-11-15）

## 結論

OpenFreeMap のメタデータと単一 z14 タイルは、このクラウド環境で取得・復号できた。
ただし、描画用タイルから道路の不存在を証明できる契約は確認できていない。
現時点では Overpass の代替として `road_coverage` を付与せず、着工判定にも接続しない。
通信成功と、道路網の完全性・同じ高さの横断判定に必要な意味情報は別の検証項目である。

## 実測

診断クライアントの User-Agent は
`Rosenfu/0.1.0-alpha.1 (+https://github.com/Mei8n/rosenfu)`。
固定メタデータを1回、その応答に記載された同一ホストのタイル1枚を逐次取得した。
以下の時刻はレスポンス Date ヘッダー（UTC）。

- 14:26:48: `https://tiles.openfreemap.org/planet` は HTTP 200、JSON。
- revision `20261101_080013_pt`、maxzoom 14、transportation layer を確認。
- 14:28:03: `https://tiles.openfreemap.org/planet/20261101_080013_pt/14/14552/6451.pbf`
  は HTTP 200、748661 bytes、MVT。両応答とも `Access-Control-Allow-Origin: *`。
- 既存の `@mapbox/vector-tile` / `pbf` で復号: 13 layers、transportation 888 features、
  うち line 815、extent 4096。bridge 82、tunnel 130、brunnel 未設定676。
  IDは全featureに存在したが、元OSM way/nodeの識別子である保証は検証していない。

ブラウザ/Tauriからの実行は未検証。CORSヘッダーだけでアプリCSPや実行時を検証済みとはしない。
Overpass の HTTP 406 は引き続き原因未確定。今回の成功はその発生主体の特定にはならない。

## 提供条件と完全性

[OpenFreeMap利用条件](https://openfreemap.org/tos/)（2026-10-14更新）は自動収集を原則禁止し、
オフラインダウンロードに限り1回3000 tilesまたは50 MB、同時4以下、アプリ固有User-Agentを要求する。
継続的な画面追従取得を無制限に行ってよいとは解釈しない。採用する場合は、保存する計画領域単位、
両上限を守る容量予算、逐次取得、有限cache、失敗cooldown、固定ホスト/revision検証を設ける。
WebViewが固有User-Agentを送ることも実測する。普通のブラウザfetchに偽装UAを要求しない。
TileJSONが指定するOpenMapTilesとOpenStreetMapの帰属も保持する。

[Planetiler OpenMapTilesのTransportation実装](https://github.com/openmaptiles/planetiler-openmaptiles/blob/main/src/main/java/org/openmaptiles/layers/Transportation.java)
では z14 にpath/track/driveway等が出る設定だが、描画用の線結合・簡略化処理がある。
これは配信revisionのビルド設定・元データ範囲を証明するmanifestではない。
`brunnel` / `layer` は道路の構造属性であり、ゲームの実高さmには変換しない。
元OSM graphの識別子や原形状を保つ既存ReferenceGraphBundleへ、タイルのIDを偽装して入れない。

実用化には、対象道路種別・元データrevision・無省略範囲・境界継続性を確認できる配信契約が必要。
描画タイルの空応答やtransportation欠損から `road_coverage` を作る実装は不可。
この保証が得られない場合は、OSM由来の無簡略化道路パックを生成する管理下の配信基盤が必要になる。
TrainsWorldのコードや公開packはコピーしていない。pack全体を一律ODbLとして再配布しない。
