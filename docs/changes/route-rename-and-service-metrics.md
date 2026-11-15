# 路線名変更と一覧列

- RouteDirectory の名前変更は `rename_route(routeId, name, expectedName)` を使用する。現在の名前が変わっていたら拒否し、入力は保持する。
- backend は対象会社/名前を検証し、複製上で名前だけ変更、全体と保存可能性を検証してから一括適用する。ID、経路、所属駅、系統、binding は変更しない。
- 路線一覧は名前・距離のみ。乗客・収支・車両数は系統一覧に移し、未実装は「—」と明示する。
- App の rename callback、worldEpoch key、onDirty 接続は連続路線編集ブランチ側の依存変更。これらを一体統合してから機能完成とする。
- GUI 未確認。系統の実数値計算は追加していない。
