import {useEffect, useId, useRef} from "react";
import "./UnsavedCloseDialog.css";

type Props = {
  error: string;
  closing: boolean;
  onCancel: () => void;
  onDiscard: () => void;
};

/** Native modal dialog keeps the map inert and traps focus in the top layer. */
export function UnsavedCloseDialog({error, closing, onCancel, onDiscard}: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const titleId = useId();
  const descriptionId = useId();
  useEffect(() => {
    const previous = document.activeElement;
    const element = dialog.current!;
    element.showModal();
    cancel.current?.focus();
    return () => {
      element.close();
      if (previous instanceof HTMLElement && previous.isConnected) previous.focus();
    };
  }, []);
  return <dialog ref={dialog} className="unsaved-close-dialog" aria-labelledby={titleId} aria-describedby={descriptionId}
    onCancel={event => {event.preventDefault(); if (!closing) onCancel();}}
    onKeyDown={event => event.stopPropagation()}>
    <h2 id={titleId}>未保存の内容を破棄して終了しますか？</h2>
    <p id={descriptionId}>保存していない変更や未確定の参照候補は失われます。保存済みの未着工計画は、次回読み込んだときに復元できます。</p>
    {error && <p className="close-dialog-error" role="alert">{error}</p>}
    <div className="close-dialog-actions">
      <button ref={cancel} type="button" className="secondary" disabled={closing} onClick={onCancel}>編集に戻る</button>
      <button type="button" className="danger" disabled={closing} onClick={onDiscard}>{closing ? "終了中…" : "破棄して終了"}</button>
    </div>
  </dialog>;
}
