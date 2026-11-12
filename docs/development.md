# 開発環境と起動手順

## この土台の範囲

Issue #1の技術検証としてTauri 2、React / TypeScript、Rustの構成を追加しています。
共有RustライブラリをデスクトップとCLIから呼び出し、エンジン名と版番号を返します。
シミュレーション、地図、建設、保存、通信サーバーは未実装です。CLIは専用サーバーではありません。

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

画面に「Rosenfu エンジンに接続しました」と版番号が表示されることを確認します。
ブラウザで画面だけ確認する場合は`npm run dev`を使い、`http://localhost:1420`を開きます。
この場合はRustへ接続せず、プレビューであることを表示します。

```sh
cargo run --locked -p rosenfu-cli
```

CLIは共通エンジンの情報をJSONで出力して終了します。

## 確認コマンド

```sh
npm test
npm run build
cargo fmt --all --check
cargo test --locked
npm run tauri -- build --no-bundle
```

`cargo test`の既定対象はcoreとCLIです。デスクトップは最後のコマンドで別途ビルドします。
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
| フロントエンド型検査・ビルド・接続状態テスト | 成功（接続状態テスト3件） |
| Chromiumでのブラウザプレビュー | 表示とページエラーなしを確認 |
| Linuxのネイティブ起動 | 作業環境にGTK/WebKitGTK開発ライブラリがなく未確認 |
| Windows / macOSの実機起動 | 未確認 |
| 3 OSの自動ビルド | CIを追加。実行結果はPRを参照 |

Issue #1は実機起動と採用判断が残るため、この変更だけでは完了にしません。
