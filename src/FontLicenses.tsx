import license from './assets/fonts/line-seed-jp/OFL.txt?raw';

/** Bundled text remains readable offline; no font CDN or external fetch. */
export function FontLicenses() {
  return <details className="font-license">
    <summary>フォントのライセンス</summary>
    <p>LINE Seed JP · Copyright 2020-2022 LY Corporation</p>
    <p>SIL Open Font License 1.1。Regular / Bold の公式配布ファイルを改変せず同梱しています。</p>
    <pre aria-label="LINE Seed JP ライセンス全文">{license}</pre>
  </details>;
}
