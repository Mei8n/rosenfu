import { useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

type EngineInfo = { name: string; version: string };
type Status =
  | { kind: "loading" }
  | { kind: "preview" }
  | { kind: "ready"; info: EngineInfo }
  | { kind: "error" };

export function App() {
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    if (!isTauri()) {
      setStatus({ kind: "preview" });
      return;
    }
    setStatus({ kind: "loading" });
    invoke<EngineInfo>("engine_info").then(
      (info) => {
        if (active) setStatus({ kind: "ready", info });
      },
      () => {
        if (active) setStatus({ kind: "error" });
      },
    );
    return () => {
      active = false;
    };
  }, [attempt]);
  return (
    <main>
      <p className="eyebrow">路線譜 / 開発プレビュー</p>
      <h1>Rosenfu</h1>
      <p>路線をつくり、ダイヤを描き、街の流れを変える。</p>
      <section aria-labelledby="status-title">
        <h2 id="status-title">起動確認</h2>
        <div role="status" aria-live="polite">
          {status.kind === "loading" && <p>共通エンジンに接続しています。</p>}
          {status.kind === "preview" && (
            <p>
              ブラウザでの画面確認です。共通エンジンへの接続はデスクトップ版で確認できます。
            </p>
          )}
          {status.kind === "ready" && (
            <p>
              {status.info.name} エンジンに接続しました。
              <br />
              バージョン {status.info.version}
            </p>
          )}
          {status.kind === "error" && (
            <p>共通エンジンに接続できませんでした。</p>
          )}
        </div>
        {status.kind === "error" && (
          <button onClick={() => setAttempt((x) => x + 1)}>再試行</button>
        )}
      </section>
      <p className="note">
        現在はアプリケーションの土台を検証しています。路線建設やワールドの保存は今後追加します。
      </p>
    </main>
  );
}
