import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { emitTo, listen, resetHandlers } from "./test-support/tauriEventMock";
import { useAgentViewedSession } from "./useAgentViewedSession";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

const emit = (payload: unknown) => act(() => emitTo("agent-viewed-session-changed", payload));

describe("useAgentViewedSession", () => {
  beforeEach(() => {
    resetHandlers();
    listen.mockClear();
  });

  async function renderReady() {
    const rendered = renderHook(() => useAgentViewedSession());
    await act(async () => {
      await Promise.resolve();
    });
    expect(listen).toHaveBeenCalled();
    return rendered;
  }

  it("starts at 0 before any event arrives", async () => {
    const { result } = await renderReady();
    expect(result.current).toBe(0);
  });

  it("subscribes to the agent-viewed-session-changed channel, not any other", async () => {
    await renderReady();
    expect(listen).toHaveBeenCalledWith("agent-viewed-session-changed", expect.any(Function));
  });

  it("updates to the index rust sends", async () => {
    const { result } = await renderReady();
    emit({ index: 2 });
    expect(result.current).toBe(2);
  });

  it("a later transition replaces the previous index directly", async () => {
    const { result } = await renderReady();
    emit({ index: 1 });
    emit({ index: 3 });
    expect(result.current).toBe(3);
  });

  it.each([
    ["a negative index", { index: -1 }],
    ["a non-integer index", { index: 1.5 }],
    ["a missing index key", {}],
    ["a string index", { index: "0" }],
    ["null", null],
  ])("%s is ignored, state stays at the default 0", async (_label, payload) => {
    const { result } = await renderReady();
    emit(payload);
    expect(result.current).toBe(0);
  });

  it("a malformed event after a valid one is ignored, state stays at the last valid value", async () => {
    const { result } = await renderReady();
    emit({ index: 2 });
    emit({ index: -1 });
    expect(result.current).toBe(2);
  });

  it("logs and swallows a listener registration failure, rather than throwing", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    listen.mockRejectedValueOnce(new Error("registration failed"));
    renderHook(() => useAgentViewedSession());
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(consoleError).toHaveBeenCalledWith(
      "agent-viewed-session-changed listener failed to register",
      expect.any(Error),
    );
    consoleError.mockRestore();
  });

  it("cleans up the listener on unmount", async () => {
    const { unmount } = await renderReady();
    expect(() => unmount()).not.toThrow();
  });
});
