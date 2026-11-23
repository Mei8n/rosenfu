# Rosenfu

日本の実際の地図上に鉄道路線を建設し、ダイヤと旅客の流れを楽しむ架空鉄道ゲームの開発プロジェクトです。

路線建設・ダイヤ設定・旅客需要・旅客流動を中心に、少人数で同じワールドを共有するマルチプレイを目指します。複雑な経営管理よりも、路線や運行を変えたときの鉄道網の変化を重視します。

## 開発状況

α0.2の開発系列では、実地図上での線路・駅・路線の編集と保存に加え、共通時計・ダイヤによる列車運行、道路データを使ったバス系統とトラム敷設、仮の人数を指定する固定OD試算を試せます。道路データが確認できない場所での道路追従や経路確定には制限があり、全国での動作、実人口に基づく旅客需要、マルチプレイの完成を示すものではありません。

Windows x64の内部テスト版には[テスト手順](docs/windows-alpha02-test-play.txt)があります。通常版とはセーブ先が別です。これは一般公開や全OSの実機確認を示すものではありません。開発環境で起動する場合は[開発環境と起動手順](docs/development.md)、機能の条件は[α0.2の運行仕様](docs/alpha02-operation.md)を参照してください。

## ドキュメント

まず [開発ドキュメントの入口](docs/README.md)から、[実装状態と次の作業](docs/implementation-plan.md)を確認してください。需要のメッシュ/ゾーン案は[需要・流動計画](docs/demand-flow-plan.md)にまとめています。

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

## 技術構成

現行試作は Tauri 2、React / TypeScript、Rust、JSON保存で構成しています。MapLibre GL JS、SQLite、マルチ通信方式は検討対象であり、採用済みではありません。地理・統計データの取得・配布方法も検証して決定します。
