// @vitest-environment jsdom
import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { useUnsavedCloseGuard } from "./useUnsavedCloseGuard";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
vi.mock("@tauri-apps/api/core",()=>({isTauri:vi.fn(()=>false)}));
vi.mock("@tauri-apps/api/window",()=>({getCurrentWindow:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks();delete (window as unknown as Record<string,unknown>).__TAURI_INTERNALS__;});
test("browser unload is guarded only while there are unsaved changes",()=>{
  const {rerender,unmount}=renderHook(({unsaved})=>useUnsavedCloseGuard(unsaved),{initialProps:{unsaved:false}});
  let event=new Event("beforeunload",{cancelable:true});window.dispatchEvent(event);expect(event.defaultPrevented).toBe(false);
  rerender({unsaved:true});event=new Event("beforeunload",{cancelable:true});window.dispatchEvent(event);expect(event.defaultPrevented).toBe(true);
  rerender({unsaved:false});event=new Event("beforeunload",{cancelable:true});window.dispatchEvent(event);expect(event.defaultPrevented).toBe(false);
  unmount();event=new Event("beforeunload",{cancelable:true});window.dispatchEvent(event);expect(event.defaultPrevented).toBe(false);
});
test("native close is prevented until explicit discard; cancel preserves edits",async()=>{
  vi.mocked(isTauri).mockReturnValue(true);(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__={};
  let request:(event:{preventDefault:()=>void})=>void=()=>{};
  const destroy=vi.fn().mockResolvedValue(undefined),unlisten=vi.fn();
  vi.mocked(getCurrentWindow).mockReturnValue({destroy,onCloseRequested:vi.fn(async handler=>{request=handler;return unlisten;})} as unknown as ReturnType<typeof getCurrentWindow>);
  const {result,rerender,unmount}=renderHook(({unsaved})=>useUnsavedCloseGuard(unsaved),{initialProps:{unsaved:true}});
  await waitFor(()=>expect(getCurrentWindow).toHaveBeenCalled());
  const preventDefault=vi.fn();act(()=>request({preventDefault}));expect(preventDefault).toHaveBeenCalled();expect(result.current.confirmClose).toBe(true);expect(destroy).not.toHaveBeenCalled();
  act(()=>result.current.cancelClose());expect(result.current.confirmClose).toBe(false);expect(destroy).not.toHaveBeenCalled();
  act(()=>request({preventDefault}));await act(()=>result.current.discardAndClose());expect(destroy).toHaveBeenCalledOnce();
  rerender({unsaved:false});preventDefault.mockClear();act(()=>request({preventDefault}));expect(preventDefault).not.toHaveBeenCalled();
  unmount();expect(unlisten).toHaveBeenCalledOnce();
});
test("native registration and close failures are visible rather than claiming success",async()=>{
  vi.mocked(isTauri).mockReturnValue(true);(window as unknown as Record<string,unknown>).__TAURI_INTERNALS__={};
  vi.mocked(getCurrentWindow).mockReturnValue({onCloseRequested:vi.fn().mockRejectedValue(Error("denied")),destroy:vi.fn().mockRejectedValue(Error("denied"))} as unknown as ReturnType<typeof getCurrentWindow>);
  const {result}=renderHook(()=>useUnsavedCloseGuard(true));
  await waitFor(()=>expect(result.current.closeError).toMatch(/登録できません/));
  await act(()=>result.current.discardAndClose());expect(result.current.closeError).toMatch(/計画は保持/);
});
test("repeated discard is deduplicated and a failed close can be retried",async()=>{
  let rejectClose:(reason:Error)=>void=()=>{};
  const destroy=vi.fn().mockImplementationOnce(()=>new Promise((_,reject)=>{rejectClose=reject;})).mockResolvedValue(undefined);
  vi.mocked(getCurrentWindow).mockReturnValue({destroy} as unknown as ReturnType<typeof getCurrentWindow>);
  const {result}=renderHook(()=>useUnsavedCloseGuard(true));
  let pending:Promise<void>;
  act(()=>{pending=result.current.discardAndClose();});
  expect(result.current.closing).toBe(true);
  await act(()=>result.current.discardAndClose());expect(destroy).toHaveBeenCalledOnce();
  await act(async()=>{rejectClose(Error("failed"));await pending;});
  expect(result.current.closing).toBe(false);expect(result.current.closeError).toContain("計画は保持");
  await act(()=>result.current.discardAndClose());expect(destroy).toHaveBeenCalledTimes(2);expect(result.current.closeError).toBe("");
});
