// @vitest-environment jsdom
import React from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
import { DISPLAY_SETTINGS_KEY } from "./displaySettings";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const world = {
  next_id: "3", players: [{ id: "1", name: "Player" }],
  companies: [{ id: "2", owner: "1", name: "Railway" }],
  stations: [], tracks: [], routes: [], services: [], alignments: [], station_placements: [],
};
const renamed = { ...world, companies: [{ ...world.companies[0], name: "オフライン鉄道" }] };
let saved: unknown;
let fetchAttempt: ReturnType<typeof vi.fn>;

beforeEach(() => {
  localStorage.removeItem(DISPLAY_SETTINGS_KEY);
  vi.spyOn(window.navigator, "onLine", "get").mockReturnValue(false);
  fetchAttempt = vi.fn(() => Promise.reject(new Error("network disconnected")));
  vi.stubGlobal("fetch", fetchAttempt);
  saved = world;
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockImplementation(async command => {
    if (command === "engine_info") return { name: "Rosenfu", version: "offline-test" };
    if (command === "world_snapshot") return world;
    if (command === "rename_company") return renamed;
    if (command === "save_world") { saved = renamed; return { saved: true }; }
    if (command === "load_world") return saved;
    throw new Error(`unexpected local command: ${command}`);
  });
});
afterEach(() => { cleanup(); localStorage.removeItem(DISPLAY_SETTINGS_KEY); vi.restoreAllMocks(); vi.resetAllMocks(); vi.unstubAllGlobals(); });

// jsdom does not load SVG images. Dispatch errors explicitly: this verifies
// application behavior, not the OS network stack, HTTP cache or native IPC.
async function failOnlineBackground() {
  render(<App />);
  await screen.findByText(/offline-test/);
  const tiles = Array.from(document.querySelectorAll("image[data-tile-id]"));
  expect(tiles.length).toBeGreaterThan(0);
  for (const tile of tiles) fireEvent.error(tile);
  await screen.findByText(/枚の読み込みに失敗しました/);
}

test("offline startup and failed online imagery do not block local world commands", async () => {
  await failOnlineBackground();
  expect(navigator.onLine).toBe(false);
  expect(screen.getByText(/エンジン接続済み/)).toBeTruthy();
  expect((screen.getByRole("button", { name: "保存" }) as HTMLButtonElement).disabled).toBe(false);
  expect((screen.getByRole("button", { name: "読込" }) as HTMLButtonElement).disabled).toBe(false);
  expect(fetchAttempt).not.toHaveBeenCalled();
});

test("manual offline fallback removes remote tiles and stays local through pan and zoom", async () => {
  await failOnlineBackground();
  fireEvent.click(screen.getByRole("button", { name: "サンプル背景に戻す" }));
  expect(screen.getByRole("img", { name: "駅配置用サンプル地図" })).toBeTruthy();
  expect(document.querySelectorAll("image")).toHaveLength(0);
  fireEvent.click(screen.getByRole("button", { name: "東へ移動" }));
  fireEvent.click(screen.getByRole("button", { name: "拡大" }));
  expect(document.querySelectorAll("image")).toHaveLength(0);
  expect(screen.queryByText(/枚の読み込みに失敗しました/)).toBeNull();
  expect(fetchAttempt).not.toHaveBeenCalled();
});

test("background failure does not block editing, saving and reloading local state", async () => {
  await failOnlineBackground();
  fireEvent.click(screen.getByRole("tab", { name: "設定" }));
  fireEvent.change(screen.getByLabelText("会社名"), { target: { value: "オフライン鉄道" } });
  fireEvent.click(screen.getByRole("button", { name: "設定変更" }));
  await screen.findByText("会社名を変更しました。");
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText("ワールドを保存しました。");
  fireEvent.click(screen.getByRole("button", { name: "読込" }));
  await screen.findByText("保存したワールドを読み込みました。");
  expect((screen.getByLabelText("会社名") as HTMLInputElement).value).toBe("オフライン鉄道");
  expect(invoke).toHaveBeenCalledWith("save_world");
  expect(invoke).toHaveBeenCalledWith("load_world");
  expect(fetchAttempt).not.toHaveBeenCalled();
});
