# 未組立候補を照合した全国水域stage

原典内の水域タグ付き候補を照合する reader は PR151 / `d17469a0bff4f32863f98fe3fa8363e68bceb747`。8地域の入力は `2026-11-14T21:20:41Z` の固定PBFで、元ハッシュは `water-source-set-20261114.json` に記録する。実世界の水域網羅性を保証するデータではない。海域の原典時点は別で、今回の内水面stage再生成では更新していない。

再生成結果のDBハッシュが旧版と違っても、行の挿入順などで変わるため、面が増減したとは限らない。受入は原典pin・時点・DB integrity・全観測のID/形状/タグ/bboxの比較・source issueの保持・固定10地点の抽出で確認する。未完の地域は利用せず、完了した新manifestのpinを固定する。旧stageと公開試験packを上書きしない。

## 原典で確認した未組立候補

- 四国 `relation/8242073`（黒瀬川）は `type=waterway`, `natural=water`, `waterway=river`。面の取得漏れが確認されたものではなく、非面型の候補である。
- 中国 `relation/17140040` は `type=multipolygon`, `natural=water`, `water=reservoir` だが、唯一のouter memberが `node/11578332025`。外周wayを持たず、水面の境界を復元できない。点の位置から水面幅・面積を捏造しない。
- 中国 `relation/20045092` の従来の `invalid-area` は引き続き保持する。

これらは2026-11-14の取得済みPBFを直接調べた結果であり、現在のOSMの状態を示すものではない。元データ: © OpenStreetMap contributors / ODbL-1.0、Geofabrik Japan extracts。コードのライセンスとは別に扱う。

## 建設判定へ渡す条件

1. `coverage=[]` / `geographic_completeness=unknown` を維持する。空の検索結果、source issueが0件、全8地域の取得成功のいずれも陸地の証明ではない。
2. 原典ID・tagsを保持する `observations` を使う。`water` / `observedAreas` の形状だけでは `covered` / `tunnel` / `layer` の情報が落ちるため、そのまま地表水面と解釈しない。属性付き面も、実世界の水面を保証するものではない。
3. 水路線を固定幅で面化しない。widthタグの存在と、単位・幅・位置の検証は別。海岸線単独で海側も確定しない。
4. 座標範囲・閉環・件数の検証は、自己交差・穴の包含・実際の境界精度の保証ではない。抽出bboxは検索条件であり、返却する完全形状の有効coverageではない。
5. source issueは原典全体に属する。issueの存在だけで経路との交差を判定しない。欠落の不確実な位置や広がりを局所化して、周囲を安全な陸地に変えない。
6. 道路snapshotの取得・参考追従・踏切交差は、水域取得失敗と分離する。道路取得成功から水域coverageを推定しない。地上水域禁止や30 km制約を適用する最終coreとの接続は、上記の根拠を持つ評価が別途必要。

全国地上の建設許可を実現するには、水域不存在を認定できるcoverage契約がなお不足している。この再生成はその代替ではない。未取得でも成立する最悪条件proofと実水域観測は混合せず、全域を建設不能にするgateを追加しない。追加UIは設けない。
