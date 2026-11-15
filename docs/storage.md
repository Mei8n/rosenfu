# 保存仕様

## 現行形式：schema 19

暫定アルファ JSON は `format: "rosenfu-world"` と `schema_version: 19` を持つ。schema 19 で書き込み、18 は系統色未設定として読み込む。1〜17 は非対応を明示して拒否する。旧 Track モデルの自動移行を撤去したため、データは新しいワールドで作成し直す。読込処理は旧データを変更・削除しない。

設備は Alignment と端点接続。すべての路線・系統に RoutePath / ServicePath が必要で、旧 Track、Route.tracks、OperatingService.tracks/stops は非対応。旧データを現形式番号に書き換えただけの偽装も拒否する。

未着工計画、編集済み形状、駅・駅グループ、視点、速度設定、系統の経路・停車・明示的路線関連を現形式で保存する。路線所属駅と停車駅を区別し、系統経路編集では既存 ID と参照整合性を保護する。列車運行や乗換効果を保存モデルの存在だけで実装済みにしない。

ID と採番状態はコアモデルの文字列表現をそのまま保存します。描画データや検索索引は
保存せず、再生成対象とします。読み書きする JSON は暫定で 16 MiB に制限しています。
全国規模を収容する上限ではありません。サイズ超過はエラーとして扱います。

## 書き込みと復旧

- 新規保存は同じディレクトリの一時ファイルに書き込み、ファイルを同期してから
  目的の名前で確定します。準備中に同名ファイルが作られた場合は上書きせず失敗します。
- 既存保存の更新時は、既存データを読み込み・検証してから、元のバイト列を
  `<保存ファイル名>.bak` へ保存します。その後、新しい本体へアトミックに置換します。
- バックアップは直前の有効な保存 1 世代です。更新成功時、以前の `.bak` は置き換わります。
- 既存本体が破損・非対応スキーマ・読み取り不能の場合、本体とバックアップを変更せず
  更新を拒否します。別名で保存するか、ユーザーが復旧する必要があります。
- バックアップ作成に失敗した場合、本体を変更しません。新本体の確定に失敗した場合も
  元の本体が残り、作成済みのバックアップから復旧できます。
- 本体・バックアップのパスにあるシンボリックリンクやディレクトリは受け付けません。
- 親ディレクトリは自動作成しません。保存先の選択・作成は呼び出し側の責務です。

アトミック置換は一つのファイルの更新についての保証です。本体とバックアップを
まとめたデータベーストランザクションではありません。また、ファイル内容は同期しますが、
全 OS・全ファイルシステムで電源断後のディレクトリエントリーまで保証するものではありません。

同じパスへの保存は、権威側エンジンが直列化する単一 writer を前提とします。
ロック非協調の外部編集やクラウド同期競合の自動解決、自動復旧、
複数世代バックアップは未実装です。バックアップを自動で読み込むこともありません。
復旧時は対象 `.bak` を明示的に `load_world` へ渡し、検証結果を確認します。

## API と確認

- `encode` / `decode`: 上限・スキーマ・モデル整合性を確認するメモリ上の入出力。
- `create_world`: 新規保存のみ。同名パスがあれば上書きせず失敗します。
- `save_world`: 新規またはバックアップ付き更新。結果にバックアップパスを返します。
- `load_world`: 上限付き読み込みと検証。失敗を呼び出し元へ返します。
- `backup_path`: 対象保存のバックアップ名を決定します。

```sh
cargo test --locked -p rosenfu-storage
```

テスト対象はグラフと採番状態の往復、形式・バージョン・参照不正、サイズ制限、
新規保存、バックアップ更新、破損保存の保護、バックアップ作成失敗時の保護などです。
保存・読込ボタンと[固定2分のオートセーブ](autosave.md)を実装しています。未着工計画も同一ファイルへ保存します。[計画保存](construction-plan-persistence-design.md)を参照。

## 復旧版で再実装した保存安全対策（2026-11-13）

全編集は候補ワールドの検証とpretty JSONの16 MiB上限確認を通ってから反映する。小さなcompact JSONでも再保存時の上限を超える場合は読込を拒否する。サイズ確認は全出力をメモリへ確保しないカウンタで行う。

セッションは読込・保存時のSHA-256を保持し、保存先が外部変更・削除されていれば拒否する。協調するプロセス間は`.lock` sidecarのOS try-lockで排他し、ロック競合は待ち続けずエラーにする。失敗した保存を成功扱いにせず、再読込／明示確認が必要。ロックを無視する外部編集やネットワークFSの完全なトランザクション保証ではない。

オートセーブの状態確認は既に承認済みの同一ファイル版を無効化しない。古い応答は画面の世代識別で無視し、復元直後に遅れて状態確認が実行されても同一版への承認を保持する。既存スロットは状態確認時の版に対する明示置換許可を要求し、確認後に変化した場合は拒否する。競合後は自動再試行せず、ユーザーの再確認を待つ。不存在の再確認後はMissing版との比較で新規作成できる。

オートセーブ復元後の通常保存では、正常な既存manualを従来どおり.bakへ保存する。破損／容量超過manualのみ明示的復元に基づく修復保存を許し、既存.bakを保持する。未知の将来スキーマや読み取り権限エラーは破損と見なして上書きしない。

## schema 10: 未着工計画

`world.construction_plans` に型付きの計画を保存します。旧schema 1〜9にはこのフィールドがなく、空の計画として復元します。型不正は読込前に拒否し、途中の線形は着工時に検証します。計画も16 MiB上限・既存バックアップ・競合検出の対象です。

## schema 11: 既設線上の計画駅

計画駅の対象を `pathId`（未着工経路）または `alignmentId`（既設線）で区別し、既設線対象は `planId` で所属計画を指定します。schema 10以前の省略されたplanIdは経路の所属から解決します。

## Multiple save slots

New worlds are saved into independent `saves/slot-<opaque-id>/` directories. Each
slot has a display-label manifest and its own world, rolling backup and autosave;
labels never become filesystem paths. Creating a slot is create-only, even if its
label matches another slot. A failed creation does not publish a partial world.
The session selects one active slot and uses its own observed file version for
manual/autosave conflict protection. Selecting a slot must load successfully
before replacing the active in-memory world.

The previous fixed-path world remains available as a legacy list entry. Unsupported
schemas are explained rather than migrated, erased or overwritten. A new current
world can coexist with that file. Slot deletion is not exposed by this change.
Current-format storage tests cover independent labels/files/backups, incomplete
creation visibility, invalid IDs and directory symlink rejection. Full session,
UI and real-device acceptance remain separate verification gates.

### 系統色（schema 19）
`service_appearances` は系統IDと `#RRGGBB` 色の明示的メタデータ。不在は従来の表示色を維持する。18の入力にこのフィールドが混入した場合は拒否する。19を18専用の旧アプリでは開けない。複製は新しい系統IDを割り当て、物理経路・停車・binding・色をコピーする。複製後の編集は独立。
