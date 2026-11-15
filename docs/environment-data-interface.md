# 道路・水域・実在線路の接続境界

実在線路取得は別repositoryとして実装し、道路取得とサーバーqueue/coverageを混同しない。

- 共通の原形状は `src/referenceGraph.ts` / `ReferenceGraphBundle`。元OSM node/way ID（decimal string）、segmentIndex、経緯度を保持。rail/roadのmodeを区別。MVT feature IDを元OSM IDとして使わない。
- source revision/hash とgraph hashを保存。今回roadPackRepositoryはbuild-pinned manifest SHA、cell SHA、revision一致を確認してRoadSnapshotを返す。戻り型はautomaticRoadData.tsの既存型と同じ。raw XML生成器はscripts/road-pack、配信先固定はsame-origin `/road-data/`。
- 取得状態は `loading/ready/error/unavailable`。全必要領域を検証するまでreadyにしない。明示的空セルと失敗/404/欠損は別。古いworld/planの応答はcacheに入ってもUIへ公開しない。
- 環境判定は `EnvironmentSnapshot` のroad_coverageとwater_coverageを独立に要求する。railの参考線取得完了は、道路または水域coverageの代用にはならない。
- 線路実高さはAlignment.section_profiles優先、次にvertical_profile、profileなし時だけ各区間level。OSM layer/level/bridge/tunnelは構造タグでありゲーム高さmへの変換は禁止。
- `preview_environment` / `build_plan_with_environment` は同じ候補生成・検査経路。元道路geometryを追従していても、着工時には当該環境snapshotと線路実高さで検査する。
- `PlannedAlignment`はanchor_levels/start_level/junction_sourceを受け取り、preview/buildとも同じrequestを渡す。道路用取得UIはこれらを独自解釈しない。
- 保存はWorldDraft.environment。更新はproviderごとの範囲内置換、範囲外保持。踏切は保存snapshotと現在のgeometry/profileから再導出し、撤去/Undoで孤立資産を残さない。

最終統合試験: 同じOSM node/wayを取得→参考線追従→高さ付計画→環境preview→着工→保存/復元→撤去/Undo。
道路・水域・railのいずれかの取得失敗、world切替後の古い応答、境界セル欠損でも無断の空扱いをしない。
