# 自動道路取得・踏切（実装途中）

## 取得契約

`src/automaticRoadData.ts` は固定 URL `https://overpass-api.de/api/interpreter`
から OSM 道路 way の完全な geometry と属性を取得する。画面接続・デスクトップCSP対応は後続作業。

`RoadSnapshot` は要求した `bounds`（取得範囲）、`fetchedAt`、既存の
`ReferenceGraphBundle` を持つ。graphのboundsは境界を跨ぐway全体を収めるため広がる場合がある。
**取得済み判定にはgraph.boundsではなくsnapshot.boundsを使う。**
元の layer/level/bridge/tunnel は GraphEdge に保持する。構造属性の異常は
roadWays.structureUnsupported で保持し、nullへの変換による地上判定を禁止する。
OSM layerは相対的な上下関係でありゲームの6m段階とは同一ではない。

取得は最大0.04度四方、応答5MiB、元way1万件、グラフは既存上限を適用。
同じ範囲の要求を共有し、異なる範囲も直列処理。成功キャッシュは24時間・最大12件、
失敗は60秒の待機。待機要求も12件に制限する。現段階のキャッシュはメモリ内のみ。
タイムアウトやOverpassのremark、欠損形状を空の成功にしない。
有効な空配列は「OSM応答に道路がない」であり実在道路の完全性保証ではない。

## 提供元と制約

- [Overpass公式運用指針](https://dev.overpass-api.de/overpass-doc/en/preface/commons.html)
  は一般アプリの常用バックエンドとして公開インスタンスに依存する運用を問題例とする。
  本取得器は小範囲試作。配布規模での継続提供には専用配信基盤が必要。
- [OSM layer](https://wiki.openstreetmap.org/wiki/Key:layer)
- [OSM level](https://wiki.openstreetmap.org/wiki/Key:level)
- 表示帰属: © OpenStreetMap contributors / ODbL-1.0

水域提供元は未確定。道路成功を水域取得成功に流用しない。OSMの局所water polygonや
coastlineがないことは陸地の証明にならない。海を含む水域カバレッジが必要。

## 保持するゲーム仕様

費用計算・請求・維持費は今回は実装しない。踏切1か所1億円は将来の仕様。
高さ倍率の単一定義は別担当のheightCostModelに委ねる。
地上線の水上建設は禁止。高架+1/+2/+3と開削-1は連続水面横断が30km超で禁止、
ちょうど30kmは許容。地下-2/-3はこの距離制約の対象外。未取得を陸地にしない。
道路共線・重なりは横断踏切にしない。非平坦な縦断区間に踏切を置かない。
