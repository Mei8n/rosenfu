// @vitest-environment jsdom
import {afterEach, beforeEach, expect, test, vi} from "vitest";
import {cleanup, fireEvent, render, screen} from "@testing-library/react";
import {UnsavedCloseDialog} from "./UnsavedCloseDialog";

beforeEach(() => {
  // jsdom has no top layer; browser supplies focus trapping and background inertness.
  HTMLDialogElement.prototype.showModal = vi.fn(function(this: HTMLDialogElement) {this.open = true;});
  HTMLDialogElement.prototype.close = vi.fn(function(this: HTMLDialogElement) {this.open = false;});
});
afterEach(() => {cleanup(); vi.restoreAllMocks();});

test("opens a named modal focused on safe cancellation and restores prior focus", () => {
  const prior = document.createElement("button"); document.body.append(prior); prior.focus();
  const cancel = vi.fn(), discard = vi.fn();
  const {unmount} = render(<UnsavedCloseDialog error="" closing={false} onCancel={cancel} onDiscard={discard}/>);
  const dialog = screen.getByRole("dialog", {name: "未保存の内容を破棄して終了しますか？"});
  expect(HTMLDialogElement.prototype.showModal).toHaveBeenCalledOnce();
  expect(document.activeElement).toBe(screen.getByText("編集に戻る"));
  fireEvent.click(dialog); expect(discard).not.toHaveBeenCalled(); expect(cancel).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("編集に戻る")); expect(cancel).toHaveBeenCalledOnce();
  unmount(); expect(document.activeElement).toBe(prior); prior.remove();
});

test("Escape cancels rather than discards and modal keyboard events do not reach map shortcuts", () => {
  const cancel = vi.fn(), discard = vi.fn(), key = vi.fn();
  render(<UnsavedCloseDialog error="" closing={false} onCancel={cancel} onDiscard={discard}/>);
  window.addEventListener("keydown", key);
  for (const value of ["Escape", "+", "-", "z"]) fireEvent.keyDown(screen.getByText("編集に戻る"), {key:value});
  expect(key).not.toHaveBeenCalled(); window.removeEventListener("keydown", key);
  fireEvent(screen.getByRole("dialog"), new Event("cancel", {bubbles:false, cancelable:true}));
  expect(cancel).toHaveBeenCalledOnce(); expect(discard).not.toHaveBeenCalled();
});

test("discard is explicit; closing disables actions; failure remains inside dialog", () => {
  const cancel = vi.fn(), discard = vi.fn();
  const {rerender} = render(<UnsavedCloseDialog error="" closing={false} onCancel={cancel} onDiscard={discard}/>);
  fireEvent.click(screen.getByText("破棄して終了")); expect(discard).toHaveBeenCalledOnce();
  rerender(<UnsavedCloseDialog error="" closing onCancel={cancel} onDiscard={discard}/>);
  fireEvent.click(screen.getByText("終了中…")); fireEvent.click(screen.getByText("編集に戻る"));
  fireEvent(screen.getByRole("dialog"), new Event("cancel", {cancelable:true}));
  expect(discard).toHaveBeenCalledOnce(); expect(cancel).not.toHaveBeenCalled();
  rerender(<UnsavedCloseDialog error="終了できませんでした。計画は保持されています。" closing={false} onCancel={cancel} onDiscard={discard}/>);
  expect(screen.getByRole("alert").textContent).toContain("計画は保持");
  fireEvent.click(screen.getByText("編集に戻る")); expect(cancel).toHaveBeenCalledOnce();
});
