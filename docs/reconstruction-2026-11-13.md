# クラウド作業コピーの再構築（2026-11-13）

## 由来と限界

本版は元の `a56fa39` の復元ではない。`8d68b3f` のGit bundleを基点に、検証用archive（作成記録の基準 `7b47c80`）のソースと、Vite変換キャッシュのsource mapから回収した41ソースパスを統合した新しい再構築である。Git履歴の欠落区間を再現したとは主張しない。元bundle/archive/source mapの原文とSHA-256 manifestは作業リポジトリ外で別保全している。

後続Rustソースは回収できなかったため、保存安全対策を現在の要件と回帰テストに基づき再実装した。フォントCSS・raw型宣言・TS import設定も再構成した。旧版のテスト全件が戻ったわけではなく、回収版に不足する道路・索引・保存競合のテストを新規追加した。プッシュはしていない。

## 現在の挙動

- 通常／高速／トラム、未着工の独立計画、円弧、駅配置・編集、路線／系統分離、保存・オートセーブの既存基礎を維持
- 限定的な実在道路追従は同一OSM way上のみ。実データ・実機での確認は未実施
- 計画はセッション内のみ。タブ移動で保持し、終了時の消失確認あり。手動保存／オートセーブは適用済みワールドのみ
- LINE Seed JP公式Regular/Bold WOFF2とOFL全文を同梱。外部フォントCDN不要
- 保存時はSHA-256版比較とOS sidecar try-lockで競合を拒否。編集時点で保存可能サイズを確認
- 通常manual保存は.bakを更新。オートセーブ復元後も正常manualは同様。壊れたmanualの明示修復では既存.bakを保持。将来スキーマは破損として上書きしない
- autosave statusが遅延して復元後に届いても、同一版への既存承認を無効化しない。別版・失敗後は再確認が必要

## 残る制約

- 最新の全設計MD・試験・ビルド設定の同一性は未証明。取得済み草案は未回収参照を明記して取り込み
- 動的旅客流動、列車運行、運賃計算、マルチプレイ等は未実装。研究草案を実装済みと扱わない
- 大ワールドの編集ではclone・サイズ計算が全ワールドを走査し、参照検証には線路数×駅数などの二乗的な検索も残る。経路候補のグラフ構築は線形別に駅を一度だけ集約し、ソート後の同位置判定を近傍比較に改善したが、全体の差分検証・ID索引化は未実施。地図全体描画の負荷・実OSM・Windows/macOS GUIは要検証
- ロック非協調外部writerやnetwork filesystemの完全な競合制御は保証しない
- Linux native checkはgobject-2.0開発ライブラリ不在で停止。フロントビルドや独立session試験はnative Tauriビルドの代わりではない

## 検証記録

- `npm run build`: 成功
- frontend標準5秒timeout・単worker: 47ファイル、319件中315成功／4 timeout。Appのbrowser preview、削除guard、背景pan/zoom、背景patternの既存テストで発生。標準設定で全成功とは扱わない
- 同一コード・単worker・`--testTimeout=15000`: 47ファイル319件全成功。機能期待値は緩和していない。処理時間の安定性改善は残る
- `cargo test --locked`: default-members 119件成功（schema 1〜7移行と新しい保存競合回帰を含む）
- 独立session harness: 32件成功、clippy成功。正常manual backup退行・status/restore順序の独立レビューと回帰追加後の結果
- `cargo fmt --all --check`、default-members `cargo clippy --locked --all-targets -- -D warnings`、`git diff --check`: 成功
- `cargo check --locked -p rosenfu-desktop`: gobject-2.0開発ライブラリ不足で停止。native build/GUI未検証
- 合成13,000node/25,998edgeの索引benchmarkはscanとの候補一致・表示件数一致を検証。実OSMや全体ゲーム性能の保証ではない

初回旧テストのasync待機不足は適切なawaitへ修正した。標準5秒のtimeoutは隠さず上記に残す。元のa56fa39にあった335件一式が復元されたわけではなく、今回319件を新たに検証した。

## 後続の再照合・再検証

`502ab51` の経路候補グラフ修正後、担当者の再検証報告はフロント319件（単worker・15秒timeout）、Rust default-members 120件、型検査・ビルド・Clippy・rustfmt成功。上記119件は初回復元時の記録として保持する。`570ea3a` では比較調査設計を追加。今回の文書整合修正ではコードテストを再実行しておらず、実OSM・ネイティブGUI・全体性能の未確認は変わらない。
