import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function useUnsavedCloseGuard(unsaved: boolean) {
  const unsavedRef = useRef(unsaved);
  unsavedRef.current = unsaved;
  const [confirmClose, setConfirmClose] = useState(false);
  const closingRef = useRef(false);
  const [closing, setClosing] = useState(false);
  const [closeError, setCloseError] = useState("");
  useEffect(() => {
    const beforeUnload = (event: BeforeUnloadEvent) => {
      if (!unsavedRef.current) return;
      event.preventDefault(); event.returnValue = "";
    };
    window.addEventListener("beforeunload", beforeUnload);
    let disposed = false;
    let unlisten: undefined | (() => void);
    const register = async () => {
      try {
        const cleanup = await getCurrentWindow().onCloseRequested(event => {
          if (!unsavedRef.current) return;
          event.preventDefault(); setConfirmClose(true);
        });
        if (disposed) cleanup(); else unlisten = cleanup;
      } catch {
        if (!disposed) setCloseError("終了確認を登録できませんでした。未着工の計画は終了時に失われます。着工・保存するまで閉じないでください。");
      }
    };
    if (isTauri() && "__TAURI_INTERNALS__" in window) void register();
    return () => { disposed = true; unlisten?.(); window.removeEventListener("beforeunload", beforeUnload); };
  }, []);
  const discardAndClose = async () => {
    if (closingRef.current) return;
    closingRef.current = true; setClosing(true); setCloseError("");
    try { await getCurrentWindow().destroy(); }
    catch {
      closingRef.current = false; setClosing(false);
      setCloseError("終了できませんでした。計画は保持されています。");
    }
  };
  return { confirmClose, closeError, closing, cancelClose: () => {if (!closingRef.current) setConfirmClose(false);}, discardAndClose };
}
