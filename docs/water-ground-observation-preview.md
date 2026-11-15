# 地上水域観測を既存建設候補へ照合する最小経路

`rosenfu_core::water_observations::observe_ground_water(before, candidate, areas)` は、既存native sessionのpreviewが生成した候補worldに対する観測APIである。候補と変更前worldをcoreでvalidateし、全ての変更alignmentを抽出する。承認・拒否・完全coverage・陸地の認定は返さず、保存データを変更しない。

区間別Hermite profile、全体profile、legacy infrastructureの順に確認し、全区間がcoreの平坦判定を満たし、開始高さが0である線路だけを面と照合する。非平坦・非地上は`unevaluated_profiles`へ返す。途中0接触・環状/分岐の連続水域30km・-2/-3免除の判定を実装したものではない。地上の平坦線路でも無接触は陸地の証明ではなく、coverageは常にUnknownである。

海域productの原典URLと`source_dataset`が一致する面を`MappedMarineSurface`という原典の主張として区別する。covered/tunnel/bridgeや非0のlayer/level/eleは高さ未解決、basinは湛水状態未解決、その他の内水面は地表分類未解決として扱う。layerを6mに変換せず、属性欠落を地上と認定しない。URL/tagだけで原典を認証するAPIではないため、呼出側で固定descriptorとpayload hashを検証することが前提。

形状照合は保存された経緯度平面の辺接触（数値許容差を含む）と内外判定。穴を保持し、両端が外でも途中を横断する線路を検出する。自己交差や穴の包含の完全な位相検証はこのAPIでは行わず、呼出側の検証が前提。観測10000、座標100000、辺比較2000000、接触出力1024の上限を持ち、超過時は部分結果を返さない。

## 実データと縦断検証

PR157の実delivery readerを使い、固定HTTPSの関門composite（fixture内descriptor参照）を取得して全hashを検証した。`marine-record/4084`のpolygon_part=1（0始まりで2番目）の分割面をそのままfixtureへ保存し、境界点と頂点平均の中間に試験線路を置いた。Shapelyで面の有効性と線路が厳密に面内であることを確認した。元の面形状は変更していない。

データは© OpenStreetMap contributors / ODbL-1.0。海域原典時点は2026-11-13T15:00:00Z。fixtureのsource_sha256はmarine component inventoryのpinで、zip本体のhashとは別。コードのライセンスと区別する。古い固定版の試験データであり、全国再生成stageの配信更新ではない。

storage crateの専用integration testが既存`src-tauri/src/session.rs`を読み込み、実WorldEditorのpreview→候補world→水域観測、build候補との一致、入力editor不変を確認する。GTK不足のためローカルdesktop targetはビルドできず、このheadless targetで検証した。Tauri IPC/WebViewや実アプリの着工拒否のE2E試験ではない。CIは保存fixtureを使い、ネットワークへ依存しない。

## 開発統合へ必要な共有差分・blocker

- App/道路/踏切/core world/sessionは今回変更していない。現在のアプリで水域禁止が有効になったわけではない。
- 有効化時は、preview/buildの両方が同じ固定水域入力と同じbefore/candidateを使い、候補を公開する前の同じ場所で評価する。previewだけ拒否しbuildが通る接続は不可。
- `MappedMarineSurface`も原典観測であり最終建設拒否ではない。分類・位置精度・原典時点の許容条件を決める必要がある。非地上/非flatを許可に読み替えず、完全な物理判定へ接続する。
- 水域が不明でも道路取得・参考追従を停止させない。全国地上建設を一律停止するgateは導入しない。既存小ウィンドウへ理由を渡す際も、観測/未評価と着工不可の確定判定を混同しない。
- source issue・線の幅不明・原典の未掲載水域・海域/内水面の版差は、この面照合で解消しない。登録するIPC、共有session配線、保存形式の扱いは道路/統合担当との調整が必要。
