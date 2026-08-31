import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { emitTo, listen, resetHandlers } from "./test-support/tauriEventMock";
import { useTabSelection } from "./useTabSelection";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

const emit = (payload: unknown) => act(() => emitTo("tab-selection-changed", payload));

describe("useTabSelection", () => {
  beforeEach(() => {
    resetHandlers();
    listen.mockClear();
  });

  async function renderReady() {
    const rendered = renderHook(() => useTabSelection());
    await act(async () => {
      await Promise.resolve();
    });
    expect(listen).toHaveBeenCalled();
    return rendered;
  }

  it("starts with nothing selected before any event arrives", async () => {
    const { result } = await renderReady();
    expect(result.current).toBeNull();
  });

  it("subscribes to the tab-selection-changed channel, not any other", async () => {
    await renderReady();
    expect(listen).toHaveBeenCalledWith("tab-selection-changed", expect.any(Function));
  });

  it.each(["agent", "football", "news"] as const)(
    "renders %s when rust selects it",
    async (tab) => {
      const { result } = await renderReady();
      emit({ selected: tab });
      expect(result.current).toBe(tab);
    },
  );

  it("a later transition replaces the previous selection directly", async () => {
    const { result } = await renderReady();
    emit({ selected: "agent" });
    emit({ selected: "news" });
    expect(result.current).toBe("news");
  });

  it("an explicit null deselects (the 'none' page), and is not treated as malformed", async () => {
    const { result } = await renderReady();
    emit({ selected: "football" });
    emit({ selected: null });
    expect(result.current).toBeNull();
  });

  it("an unknown tab token falls back to nothing selected rather than rendering a phantom tab", async () => {
    const { result } = await renderReady();
    emit({ selected: "football" });
    emit({ selected: "podcast" });
    expect(result.current).toBeNull();
  });

  it.each([
    ["a missing selected key", {}],
    ["a non-string, non-null selected", { selected: 3 }],
    ["a bare string instead of an object", "agent"],
    ["null", null],
    ["an array", ["agent"]],
  ])("%s falls back to nothing selected", async (_label, payload) => {
    const { result } = await renderReady();
    emit({ selected: "football" });
    emit(payload);
    expect(result.current).toBeNull();
  });

  it("cleans up the listener on unmount", async () => {
    const { unmount } = await renderReady();
    expect(() => unmount()).not.toThrow();
  });
});
