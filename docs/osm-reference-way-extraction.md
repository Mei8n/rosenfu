# 同一wayの原形区間抽出（第二段階・純関数のみ）

`extractReferenceWayPath()` は、同一データセット・同一revisionの2つの投影位置を受け取り、同じOSM way・mode内の元座標列を返す。これは読み取り専用の部品で、敷設機能の完成を意味しない。

## 入出力と拒否条件

- 投影位置はdataset ID、revision、edge ID、元way方向のfraction（0〜1）。キャッシュされた座標は信用せず、検証した元segmentから投影点を再計算する。
- source segment indexが連番で、前のsegmentのtoと次のfromの**元node ID**が一致することを要求する。座標一致のみでは接続しない。他wayの枝へ乗り換えない。
- 同way/modeに含まれる全segmentのlayer/level/bridge/tunnelが一致しない場合も、元way属性の矛盾として保守的に拒否する。unknownと既知値を同一視しない。
- オブジェクト入力にも正規化JSONの5MiB上限を適用する。
- 欠番、不連続、重複source segment、古いrevision、別dataset/way/mode、無効なfractionを明示拒否する。パック自体の重複・不正は第一段階の厳格validatorが拒否し、選択エラーは`ReferencePathError.code`で識別する。
- 最大256segment（呼出し側がさらに減らせる）。必要segment数が上限を超えたら、候補を黙って切らず拒否する。巨大なsource indexの範囲も展開前に拒否する。
- 元way順・逆方向を保持する。端点以外は元nodeの座標をそのまま返し、同一座標の連続端点のみ重複除去する。平滑化・形状簡略化はしない。長さゼロの選択は拒否する。極小fraction差が浮動小数点の丸めにより同じ座標になった場合も、empty-pathとして拒否する。
- 返却edge IDは方向順。元way内のループも選択したsource index区間をそのまま辿り、最短路へ変更しない。

## 未対応と制約

World・Alignment・保存schema・UIを変更しない。交差点を跨いだ経路探索、実OSMデータ、取得器、SQLite、法的provenanceの確立、ライセンス検証は未対応。返却source IDは追跡用で、配布許諾の証明ではない。

全パックの入力検証とedge走査があり、返却上限はCPU時間保証ではない。第一段階の座標・1cm最小segment制約が適用される。投影点は元segmentの線形内分であり、測地線や土木線形ではない。合成fixtureでのみ検証している。
