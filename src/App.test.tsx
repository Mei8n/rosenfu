// @vitest-environment jsdom
import React from "react";
import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { App } from "./App";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
test("browser preview does not claim an engine connection", () => {
  vi.mocked(isTauri).mockReturnValue(false);
  render(<App />);
  expect(screen.getByText(/ブラウザでの画面確認/)).toBeTruthy();
  expect(invoke).not.toHaveBeenCalled();
});
test("desktop displays the actual engine response", async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke).mockResolvedValue({
    name: "Rosenfu",
    version: "test-version",
  });
  render(<App />);
  expect(await screen.findByText(/test-version/)).toBeTruthy();
  expect(invoke).toHaveBeenCalledWith("engine_info");
});
test("connection failure can be retried", async () => {
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(invoke)
    .mockRejectedValueOnce(new Error("unavailable"))
    .mockResolvedValueOnce({ name: "Rosenfu", version: "recovered" });
  render(<App />);
  fireEvent.click(await screen.findByRole("button", { name: "再試行" }));
  expect(await screen.findByText(/recovered/)).toBeTruthy();
});
