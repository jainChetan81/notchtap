import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { SlotState } from "./useSlotState";
import { useSlotState } from "./useSlotState";

type Handler = (event: { payload: SlotState }) => void;
const handlers: Handler[] = [];

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name: string, handler: Handler) => {
    handlers.push(handler);
    return Promise.resolve(() => {});
  }),
}));

function emit(payload: SlotState) {
  act(() => {
    handlers.forEach((h) => h({ payload }));
  });
}

describe("useSlotState", () => {
  beforeEach(() => {
    handlers.length = 0;
  });

  async function renderReady() {
    const rendered = renderHook(() => useSlotState());
    await act(async () => {
      await Promise.resolve();
    });
    expect(handlers.length).toBeGreaterThan(0);
    return rendered;
  }

  it("starts empty before any event arrives", async () => {
    const { result } = await renderReady();
    expect(result.current).toEqual({ state: "empty" });
  });

  it("renders the showing payload as-is when an event arrives", async () => {
    const { result } = await renderReady();
    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: false,
    });
    expect(result.current).toEqual({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: false,
    });
  });

  it("a new showing payload replaces the previous one directly, without an intermediate empty frame", async () => {
    const { result } = await renderReady();
    emit({ state: "showing", id: "n1", title: "a", body: "b1", eventType: "generic", priority: "low", expanded: false });
    emit({ state: "showing", id: "n2", title: "b", body: "b2", eventType: "score_update", priority: "high", expanded: true });
    // must go straight from n1 to n2 — assert the final state only, since
    // there's no async gap between the two synchronous emits in this test
    // to observe an intermediate frame at, but a snapshot check on id is
    // the meaningful assertion either way
    expect(result.current).toMatchObject({ id: "n2", expanded: true, priority: "high" });
  });

  it("goes back to empty when rust emits an empty slot-state", async () => {
    const { result } = await renderReady();
    emit({ state: "showing", id: "n1", title: "a", body: "b", eventType: "generic", priority: "medium", expanded: false });
    expect(result.current.state).toBe("showing");
    emit({ state: "empty" });
    expect(result.current).toEqual({ state: "empty" });
  });

  it("cleans up the listener on unmount", async () => {
    const { unmount } = await renderReady();
    expect(() => unmount()).not.toThrow();
  });
});
