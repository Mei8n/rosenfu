# Rosenfu

日本の実際の地図上に鉄道路線を建設し、ダイヤと旅客の流れを楽しむ架空鉄道ゲームの開発プロジェクトです。

路線建設・ダイヤ設定・旅客需要・旅客流動を中心に、少人数で同じワールドを共有するマルチプレイを目指します。複雑な経営管理よりも、路線や運行を変えたときの鉄道網の変化を重視します。

## 開発状況

現在は0.1アルファに向けた起動・共通エンジン・基本データモデル・保存基盤の検証段階です。自作サンプル背景と選択式のオンライン実地図で駅・線路・路線の編集と保存を試せますが、列車運行・旅客シミュレーションを備えたゲームはまだ完成していません。
Windows / macOS / Linux向けのパッケージ配布を予定しています。公開日と動作要件は未定です。

起動方法と検証範囲は[開発環境と起動手順](docs/development.md)を参照してください。

## ドキュメント

- [要件と設計方針](docs/requirements.md)
- [開発ロードマップ](docs/roadmap.md)
- [バージョン管理](docs/versioning.md)
- [ワールドデータ互換方針](docs/world-compatibility.md)
- [基本ワールドモデル](docs/world-model.md)
- [運行系統の基本構造](docs/operating-services.md)
- [アルファ版の保存基盤](docs/storage.md)
- [小地域の建設・会社・路線試作](docs/station-prototype.md)
- [実地図背景の技術検証](docs/map-backgrounds.md)
- [0.1アルファの作業項目](docs/alpha-0.1.md)
- [開発への参加と確認手順](CONTRIBUTING.md)

## 技術候補

Tauri 2、React / TypeScript、MapLibre GL JS、Rust、SQLite、WebSocket等を検討しています。
採用技術、地理・統計データの取得方法と配布方法は、初期検証を経て決定します。
