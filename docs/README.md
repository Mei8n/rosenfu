# 開発ドキュメントの入口

照合対象と現在の開発範囲は[実装方針と現況](implementation-plan.md)を参照する。`main`（α0.1）、開発統合（α0.2）、実機確認、配布は区別する。

## まず読むもの

| 知りたいこと | 管理元 |
| --- | --- |
| いま動く範囲・未対応・次に行う作業 | [実装方針と現況](implementation-plan.md) |
| 段階ごとの目標・完了条件 | [ロードマップ](roadmap.md) |
| ゲームとして実現すること | [要件](requirements.md) |
| clone・ビルド・テスト・デバッグ | [開発手順](development.md)、[VS Code](vscode.md)、[参加手順](../CONTRIBUTING.md) |
| 保存形式と互換性 | [保存仕様](storage.md)、[オートセーブ](autosave.md)、[互換方針](world-compatibility.md) |
| 0.2のバス・トラム道路上設置 | [道路交通計画](road-transport-plan.md) |
| 需要の具体的な設計案 | [需要・流動計画](demand-flow-plan.md) |

## 文書の役割と更新ルール

- **採用方針**はユーザーの要求、**実装あり**はコード上の存在、**検証済み**は記載した試験の通過を指す。互いに同義ではない。
- 全体の優先順・機能状態は `implementation-plan.md`、版ごとの範囲は `roadmap.md` に集約する。各機能の操作/API・制約は詳細文書に置き、同じ進捗表を複製しない。
- 調査資料の日時・出典・推測・未検証事項は保持する。第三者の感想や参考ゲームの値は、Rosenfuの決定事項へ自動昇格させない。
- 一時設計は実装後、確定契約・根拠・試験を仕様文書へ移し、未完了分だけ残す。全項目の移管後に参照を更新して削除する。資料整理だけで未実装機能を完了扱いにしない。
- 古いスキーマ番号は導入時点の履歴。現在の書出形式は保存仕様を参照する。取り消した案を新しい予定として復活させない。

## 詳細仕様の案内

- 建設: [操作](track-construction-interaction.md)、[複数計画](independent-construction-plans.md)、[駅配置](station-direct-placement.md)、[延伸](built-alignment-extension.md)、[設計速度](track-design-speed.md)、[円弧](circular-track-drawing.md)、[線路種別](track-types.md)
- 路線・系統: [モデル](world-model.md)、[路線候補](route-candidate-preview.md)、[系統](operating-services.md)、[ServicePath導入設計](service-path-design.md)
- 地図: [背景](map-backgrounds.md)、[ズーム](smooth-map-zoom.md)、[参考駅](reference-station-packs.md)、[参考グラフ](osm-reference-graph.md)、[バンドル](reference-graph-bundle.md)、[出典と延伸](reference-extension-provenance.md)、[オフライン調査](offline-map-research.md)
- 将来設計: [分岐・勾配・車両](junction-and-grade-design.md)、[日本向け運賃](japan-fare-model-notes.md)、[駅運営・暦等の要求詳細](construction-and-service-planning.md)
- 参考調査: [TrainsWorld](trainsworld-research.md)、[報告からの試験案](trainsworld-findings-integration-audit.md)、[空想鉄道・A列車](comparative-railway-games-plan.md)
- 履歴: [復旧記録](reconstruction-2026-11-13.md)、[初期駅間接続試作](station-prototype.md)、[撤回した計画永続化案](construction-plan-persistence-design.md)

参照駅/道路変換ツールの手順は各機能文書から参照する。履歴文書の「現在」「未実装」は記録日時点の記述として読む。
