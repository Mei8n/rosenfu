# 保存仕様

## 現行形式：schema 27（α0.2）

`format: "rosenfu-world"`、`schema_version: 27` で保存します。ワールド設備・地理根拠・運行状態に加え、固定ODの試算条件を任意で1組保存します。試算結果は保存しません。[固定OD](alpha02-fixed-od.md)を参照してください。

直前のschema26は試算条件なしとして検証・読込できます。読込だけで本体を書き換えません。明示保存・有効化済みのオートセーブでのみ新形式を書き、通常保存の成功時は直前の本体をバックアップします。旧1〜25と未来の形式はワールドの復元前に非対応として拒否します。アルファ期の将来互換や27から26へのダウングレードは保証しません。読込失敗・通常保存拒否では本体と既存バックアップを変更しません。

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

全編集は候補ワールドの検証と、UTF-8 JSONの保存本体全体（末尾改行を含む）に対する16 MiB上限確認を通ってから反映する。従来どおり整形JSONを優先し、それだけが容量上限を超えた場合は、全フィールドを保持した同じschema27の非整形JSONで再試行する。非整形でも超える場合は読込・保存・着工を拒否し、既存の保存とバックアップを変更しない。形式不正やI/Oエラーでは再試行しない。サイズ確認は保存本体をメモリに構築せずに行う。手動更新および自動保存は、容量内の候補を同期してから従来のファイル版確認と原子的置換を行う。

整形で収まる保存は従来のバイト列と同じ。非整形が必要な大きな保存は、旧版では同じschema27でも旧容量判定（整形し直し）により開けない。その場合は本対応版を使い、旧版に戻す場合は更新前の保存を使用する。移行のために読込だけで書き換えたり、保存済みの道路・地理根拠を削ったりはしない。

セッションは読込・保存時のSHA-256を保持し、保存先が外部変更・削除されていれば拒否する。協調するプロセス間は`.lock` sidecarのOS try-lockで排他し、ロック競合は待ち続けずエラーにする。失敗した保存を成功扱いにせず、再読込／明示確認が必要。ロックを無視する外部編集やネットワークFSの完全なトランザクション保証ではない。

オートセーブの状態確認は既に承認済みの同一ファイル版を無効化しない。古い応答は画面の世代識別で無視し、復元直後に遅れて状態確認が実行されても同一版への承認を保持する。自動保存は対象スロットの状態・版を確認して自動開始する。確認後の外部変更は拒否する。競合後は自動再試行せず、ユーザーの再確認を待つ。不存在の再確認後はMissing版との比較で新規作成できる。

オートセーブ復元後の通常保存では、正常な既存manualを従来どおり.bakへ保存する。破損／容量超過manualのみ明示的復元に基づく修復保存を許し、既存.bakを保持する。未知の将来スキーマや読み取り権限エラーは破損と見なして上書きしない。

## 導入履歴：schema 10の未着工計画（現在の互換範囲ではない）

`world.construction_plans` に型付きの計画を保存します。旧schema 1〜9にはこのフィールドがなく、空の計画として復元します。型不正は読込前に拒否し、途中の線形は着工時に検証します。計画も16 MiB上限・既存バックアップ・競合検出の対象です。

## 導入履歴：schema 11の既設線上の計画駅（現在の互換範囲ではない）

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
world can coexist with that file. Slot deletion is available after confirmation. It moves the slot into a recoverable archive; the deleted-save list can restore it. Deleting the active slot detaches the save target while preserving in-memory edits. A new company/world creates its own slot automatically; creation failure must not retain another world's save target. Legacy deletion moves only its manual save and leaves legacy autosave/backup untouched.
Current-format storage tests cover independent labels/files/backups, incomplete
creation visibility, invalid IDs and directory symlink rejection. Session and frontend regression tests cover these operations. Native GUI and real-device acceptance remain separate verification gates.

### 現行形式の拡張メタデータ（schema 20で導入）

`service_appearances` は系統ID、`#RRGGBB` 色、丸/四角/ひし形、任意記号を保持する。記号は名前の先頭文字から初期化し、本人編集後は名称変更で上書きしない。複製は新しい系統IDへ経路・停車・binding・色・形・記号をコピーし、その後は独立する。

`service_route_bindings` の `routes` は複数の明示的関連路線を保持する。形状からの暗黙推測ではない。線形区間の線路数・7段階の高さ・縦断と分岐の追加もこの候補形式で扱う。モデルの保存対応、最終統合CI、GUI受入は別判定とする。

### OFM 根拠データの保持と計算の上限

道路参照から確定した経路などは、経路記述と、元MVTバイト列・SHA-256・タイル座標/版/規則を単一の保存ファイルに保持します。元データはSHAごとに1回だけ保存し、タイルの紐付けは別に記録します。復元では元データを再検証・デコードし、記録された範囲ごとに経路を再生成して照合します。設備を表す `world` 部分だけでは復元用バックアップになりません。描画・計算用のキャッシュは保存しません。

| 対象 | 上限 |
| --- | --- |
| schema 27のワールド全体に保持するOFM根拠 | タイルの紐付け・異なるSHAの元データがそれぞれ256件、SHAで重複を除いた元データが合計8 MiB。さらに、紐付け先ごとに再利用分も足した合計が64 MiB |
| 1回のOFM取得根拠の追加または経路計算範囲 | 最大16タイル、紐付けごとに再利用分も足して合計8 MiB、元データ1タイル最大5 MiB |
| OFM道路参照の建設計画・建設済み根拠レコード | ワールド全体で合わせて32件。バス系統数や全建設計画数の上限ではありません |
| 保存ファイル全体 | 建設・運行・地理根拠、Base64、末尾改行などを全て含め16 MiB |

同一ワールドの保持根拠に別の提供版のタイルは混在できません。読込可能な旧schema 26のOFM根拠には保持側にも旧上限（16件、紐付けごとに合計8 MiB）が適用されます。上限・欠落・改変のいずれも未確認を成功扱いせず拒否します。実際に収まるタイル数はその密度や他の保存内容にもよります。全国対応、データの網羅性、あるいは16タイルを超える1経路の保証ではありません。

合成MVTテストで、離れた地域の複数経路と256紐付けのオフライン再読込、元データの重複除去と紐付け上限の独立性を確認しています。別々の実データ地域をまたぐ長い連続経路、全国規模の容量・実機性能は未検証です。
