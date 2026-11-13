# ローカル参照地図バンドルの読み込み

`readReferenceGraphBundle(files: File[])` は `scripts/convert-osm-graph.mjs` のバージョン1出力を読むブラウザ向け純粋な読み込みAPIです。外部通信、永続化、World変更は行いません。

選択するファイルは **graph.json、source.json、complete.json の3個のみ**。名前は大文字小文字を区別し、重複、部分出力、余分なファイルを拒否します。全ファイルの読み込み前に申告サイズを確認し、読み込み後の実サイズも確認します。上限はそれぞれ5 MiB、20 MiB、4 KiBです。UTF-8を厳格にデコードし、JSON/契約/バージョンを検証します。

complete.json の SHA-256 はファイルの生バイト列と照合します。さらに source.json 内の graphSha256、地理範囲、出典、ライセンス、元wayのnd順序とgraphのセグメント、ノードIDなどを検証します。WebCryptoが使えない環境では読み込みを拒否します。graphには既存の `parseReferenceGraph` を再利用します。

返却値は `{ graph, source: { source, sourceDate, license, attribution } }`。sourceは未信頼の表示用文字列です。HTMLとして挿入したり、出典URLへ自動アクセスしたりしてはいけません。生のsource sidecarは返しません。sourceDate は既存CLIと同じ日付形式判定を使い、独立した文字数上限は設けません（source.json全体の20 MiB制限内）。長い小数秒を含む既存CLI出力も読み込めます。

**ハッシュ一致はファイル相互の整合性だけを示します。** 電子署名ではなく、出典の真正性、地理精度、道路・線路網の網羅性、方向・制約・経路探索能力は保証しません。データとハッシュを同時に改変することは可能です。区域境界では元セグメント両端が区域内にあるものだけが含まれ、区域外同士を結ぶ横断セグメントなどは欠落し得ます。原本XMLは別途保持してください。

テストは実際のCLI converterから架空の2ノード1wayを作成し、互換性、改変、部分出力、サイズ、UTF-8、JSON、sidecar不整合、WebCrypto不在を検証します。実在地理データの正しさを検証するテストではありません。
