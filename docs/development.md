# 開発環境と起動手順

## この土台の範囲

Issue #1の技術検証としてTauri 2、React / TypeScript、Rustの構成を追加しています。
共有RustライブラリをデスクトップとCLIから呼び出し、エンジン名と版番号を返します。
基本モデル・アルファ用ファイル保存と、自作サンプル背景への駅配置・直線の駅間接続・会社名設定・路線編集画面を追加しています。選択式の地理院地図（オンライン・移動とズーム対応）を表示できます。詳細な線形編集・シミュレーション・通信サーバーは未実装です。CLIは専用サーバーではありません。

構成は検証用の暫定採用です。3 OSの実機起動と配布を確認してから正式採用を判断します。
MapLibre、SQLite、通信方式はまだ導入・確定していません。

## 必要な環境

- Node.js 24、npm
- Rust stable（Cargo、rustfmt）
- [TauriのOS別前提条件](https://v2.tauri.app/start/prerequisites/)

WindowsではC++ビルドツールとWebView2、macOSではXcode Command Line Tools、
LinuxではWebKitGTK 4.1等の開発ライブラリが必要です。詳細は上記の公式手順を参照してください。
Rustだけの確認にはGUIライブラリは不要です。

## 起動

リポジトリのルートで実行します。

```sh
npm ci
npm run tauri -- dev
```

画面に「エンジン接続済み」と版番号が表示されることを確認します。駅配置の確認手順は [小地域試作](station-prototype.md) を参照してください。
ブラウザで画面だけ確認する場合は`npm run dev`を使い、`http://localhost:1420`を開きます。
この場合はRustへ接続せず、プレビューであることを表示します。

```sh
cargo run --locked -p rosenfu-cli
```

引数なしのCLIは共通エンジンの情報をJSONで出力して終了します。保存ファイルの操作は後述のサブコマンドで行います。

## 確認コマンド

```sh
npm test
npm run build
cargo fmt --all --check
cargo test --locked --workspace
npm run tauri -- build --no-bundle
```

`cargo test`の既定対象はcore・storage・CLIです。`--workspace` を付けるとデスクトップの操作テストも含み、OS別のGUI開発依存が必要です。最後のコマンドで配布前の実行ファイルを別途ビルドします。
配布用インストーラー、署名、公証、更新機構は未対応です。
Cargo.lockとpackage-lock.jsonをコミットし、依存関係の更新はPRで確認します。

## ディレクトリ

| 場所 | 役割 |
| --- | --- |
| `src/` | Reactの起動画面と接続状態表示 |
| `src-tauri/` | デスクトップの起動とRustコマンドへの橋渡し |
| `crates/rosenfu-core/` | UI・保存方式に依存しない共有処理 |
| `crates/rosenfu-cli/` | GUIなしで共有処理を実行する確認用CLI |
| `.github/workflows/check.yml` | 3 OSでのテストとデスクトップビルド |

## 技術判断と残る確認

この分割ではUIやWebViewに依存せずに共有処理を実行できます。
別方式との比較評価や地図描画の性能測定は未実施で、今回の疎通だけで製品全体の適性を保証しません。
GitHub Actionsのビルド成功もGUIの実機起動成功とは区別します。

| 検証項目 | 今回の状態 |
| --- | --- |
| LinuxのCLI実行・共有処理との一致 | 確認済み |
| フロントエンド型検査・ビルド・接続状態テスト | 成功（件数と範囲は後述のローカル検証を参照） |
| Chromiumでのブラウザプレビュー | 表示とページエラーなしを確認 |
| Linuxのネイティブ起動 | 作業環境にGTK/WebKitGTK開発ライブラリがなく未確認 |
| Windows / macOSの実機起動 | 未確認 |
| 3 OSの自動ビルド | CIを追加。実行結果はPRを参照 |

Issue #1は実機起動と採用判断が残るため、この変更だけでは完了にしません。

## Windows ローカル検証（2026-11-12）

- Node.js 24.18.0 / npm 11.16.0 / Rust 1.99.1（MSVC）で確認。
- `npm test`: 駅・線路・会社・路線編集と保存・読込画面の 43 テスト成功。
- `npm run build`: 型検査とフロントエンドビルド成功。
- `cargo fmt --all --check`: 成功。
- `cargo test --locked --workspace`: コア・採番・保存・CLI・デスクトップ操作の計 66 テスト成功。
- `cargo clippy --locked --all-targets -- -D warnings`: 成功。
- `npm run tauri -- build --no-bundle`: Windows 実行ファイルのビルド成功。
- 出力: `target/release/rosenfu-desktop.exe`。GUI の実機表示確認は別途必要。
- 基盤 PR #8 の Windows / macOS / Ubuntu CI は成功確認済み。
  これは今回のモデル変更を含む CI 実行ではありません。

構築データの初期設計と未実装範囲は [ワールド基本モデル](world-model.md) を参照してください。
Tauri がビルド時に生成する `src-tauri/gen/schemas/` はバージョン管理対象外です。

## 保存の CLI 検証

```sh
cargo run --locked -p rosenfu-cli -- world new sample.rosenfu.json
cargo run --locked -p rosenfu-cli -- world inspect sample.rosenfu.json
```

`new` は空のワールドを新規作成します。同名ファイルがあれば上書きせず失敗します。
`inspect` は形式・参照整合性を検証し、各データの件数と次の ID を JSON で出力します。
どちらも親ディレクトリを自動作成しません。エラー時は標準エラー出力と非ゼロ終了コードを返します。
GUI の保存・読込は建設試作に接続済みです。運行系統の基本構造はコア・保存・CLIに追加していますが、系統タブで経路と停車位置を編集できます。ゲーム時間・旅客シミュレーションは未実装です。
保存の制約とバックアップは [保存基盤](storage.md) を参照してください。
## クラウド再構築版の検証環境（2026-11-13）

再構築の由来と未回収項目は[復旧記録](reconstruction-2026-11-13.md)を参照。OSファイルロックAPIにはRust 1.89以上が必要で、今回Rust 1.100 stableで検証した。`Cargo.lock`を使用し、通常は最新stableを推奨する。`npm ci`でfrontend依存を用意する。

`cargo test --locked` / `cargo clippy --locked --all-targets -- -D warnings`はworkspaceのdefault-members（core/cli/storage）だけを対象にする。`cargo test --locked --workspace`にはLinuxのGTK/WebKitGTKなどnative開発依存が必要。sessionの単独試験だけではTauriコマンド登録・OS起動を検証したことにならない。

`node scripts/benchmark-reference-spatial.ts`はNode 24以上で実行する合成データの索引比較。全体ゲーム負荷や実OSMの速度保証ではない。

## Pull後のF5と依存関係の同期

VS Codeのデバッグ／リリースビルドとhot reloadタスクは、先に`node scripts/ensure-dependencies.mjs`を実行します。
初回、`package.json`／`package-lock.json`、NodeのバージョンやOSが変わった場合だけ`npm ci --include=dev`で同期します。
これにより、Pull後に`fake-indexeddb`などの新しい開発依存が見つからずF5が止まる問題を防ぎます。
同期時の`npm ci`は既存の`node_modules`を削除・再作成します。そこに加えた手編集は保持されません。ソースやセーブデータは変更しません。
同期成功の記録はGit対象外の`node_modules`内に置き、通常の繰り返しF5ではインストールを省略します。
初回／依存更新時にはネット接続（またはnpmキャッシュ）が必要です。失敗時はビルドへ進まず、次回再試行します。
この処理は開発用タスクだけであり、配布ゲームの起動・オフラインプレイ時には実行しません。

手動でビルドする場合とCIでは従来どおり先に`npm ci`を実行してください。
`node_modules`を部分的に削除するなどして壊した場合は`npm ci`で修復できます。
同じチェックアウトで依存インストールと別のビルドを同時に走らせないでください。

### 共同開発者のVS Code初回セットアップ

1. OS別のTauri前提条件、Node.js 24、Rust stableを導入し、`node --version`、`npm --version`、`cargo --version`がIDEのターミナルでも動くことを確認する。
2. `git clone https://github.com/Mei8n/rosenfu.git`後に`cd rosenfu`、`git switch feat/application-foundation`。`package.json`と`Cargo.toml`があるリポジトリルートをVS Codeで開く。
3. WindowsはMicrosoft C/C++拡張（`ms-vscode.cpptools`）、macOS / LinuxはCodeLLDB拡張（`vadimcn.vscode-lldb`）を導入する。Rust Analyzerも推奨。拡張は自動導入されない。
4. 「実行とデバッグ」でOSに対応する`Rosenfu: debug desktop (...)`を選びF5。依存同期→フロント／Rustデバッグビルド→ネイティブデバッガの順で起動する。初回のRustビルドは時間がかかる。
5. Rust側のブレークポイントを使って停止・再開を確認する。React/TypeScript側の調査にはWebViewの開発者ツールを使う。F5のネイティブデバッガがJSのブレークポイントまで扱うわけではない。

更新後はアプリとデバッグを終了してから`git pull --ff-only`し、再度F5を実行する。ローカル変更がある場合は先に保全し、強制リセットしない。
ビルド失敗時は「デバッグを続行」せず、タスクのターミナルにある最初のエラーを確認する（以前の実行ファイルを起動してしまうため）。
macOS / Linuxの起動構成は追加したが、IDEからの実機起動・ブレークポイント停止は未検証。CIのビルド成功とは区別する。

## npm の監査・警告

テスト用依存は Vitest 4.1.12 以降を使用します。以前の Vitest 3 系が含む
`@vitest/mocker` の開発サーバー向け脆弱性と `tinypool` の prototype pollution
関連脆弱性を避けるための更新です。アプリの実行時依存とは分けて確認します。

```sh
npm audit
npm audit --omit=dev
```

`npm audit fix --force` は破壊的なメジャー更新を含むため、警告が出ても機械的に
実行せず、対象・修正版・テスト結果を確認して依存関係を更新してください。

npm の `allow-scripts` に関する `esbuild` 警告は、インストールスクリプトの
許可方針に関する通知です。脆弱性の件数やビルド失敗とは別です。
全依存スクリプトの一括許可や保護設定の無効化で消さず、使用している npm の
バージョンと方針を確認し、必要なパッケージだけを個別にレビューしてください。

参考: [Vitest の任意ファイル読み取り](https://github.com/advisories/GHSA-82fw-gwwq-j7x9)、
[モック登録時のコード注入](https://github.com/advisories/GHSA-qxg6-89mg-rqv8)、
[tinypool の worker options](https://github.com/advisories/GHSA-5gmw-xhrv-c9v3)・
[run options](https://github.com/advisories/GHSA-85c8-ppgw-ccpr)、
[npm のスクリプト方針](https://docs.npmjs.com/cli/v11/using-npm/config/)
