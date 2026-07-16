import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { NotificationPayload } from "./useVisibleNotifications";
import {
  useVisibleNotifications,
  ENTER_DURATION_MS,
  EXIT_DURATION_MS,
} from "./useVisibleNotifications";

// capture the handler that the hook registers so tests can fire events
type Handler = (event: { payload: NotificationPayload }) => void;
const handlers: Handler[] = [];

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name: string, handler: Handler) => {
    handlers.push(handler);
    return Promise.resolve(() => {});
  }),
}));

function emit(payload: Partial<NotificationPayload> & { id: string }) {
  const full: NotificationPayload = {
    title: "title",
    body: "body",
    ttlSecs: 8,
    ...payload,
  };
  act(() => {
    handlers.forEach((h) => h({ payload: full }));
  });
}

describe("useVisibleNotifications", () => {
  beforeEach(() => {
    handlers.length = 0;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  async function renderReady() {
    const rendered = renderHook(() => useVisibleNotifications());
    // let the listen() promise resolve so the handler is registered
    await act(async () => {
      await Promise.resolve();
    });
    expect(handlers.length).toBeGreaterThan(0);
    return rendered;
  }

  it("adds an item in the enter phase when a promoted event arrives", async () => {
    const { result } = await renderReady();
    emit({ id: "n1" });
    expect(result.current).toHaveLength(1);
    expect(result.current[0].id).toBe("n1");
    expect(result.current[0].phase).toBe("enter");
  });

  it("moves enter → hold → exit → removed on the ttl clock", async () => {
    const { result } = await renderReady();
    emit({ id: "n1", ttlSecs: 8 });

    act(() => vi.advanceTimersByTime(ENTER_DURATION_MS));
    expect(result.current[0].phase).toBe("hold");

    act(() => vi.advanceTimersByTime(8000));
    expect(result.current[0].phase).toBe("exit");

    act(() => vi.advanceTimersByTime(EXIT_DURATION_MS));
    expect(result.current).toHaveLength(0);
  });

  it("renders every promoted item — rust owns the cap, not the frontend", async () => {
    // spec §8: this is a render-state list, not a second queue. if rust
    // promoted it, it renders; the frontend never re-enforces cap-3.
    const { result } = await renderReady();
    emit({ id: "n1" });
    emit({ id: "n2" });
    emit({ id: "n3" });
    expect(result.current.map((n) => n.id)).toEqual(["n1", "n2", "n3"]);
  });

  it("items with different ttls remove themselves independently", async () => {
    const { result } = await renderReady();
    emit({ id: "short", ttlSecs: 1 });
    emit({ id: "long", ttlSecs: 8 });

    act(() =>
      vi.advanceTimersByTime(ENTER_DURATION_MS + 1000 + EXIT_DURATION_MS),
    );
    expect(result.current.map((n) => n.id)).toEqual(["long"]);

    act(() => vi.advanceTimersByTime(7000 + EXIT_DURATION_MS));
    expect(result.current).toHaveLength(0);
  });

  it("cleans up timers and the listener on unmount", async () => {
    const { unmount } = await renderReady();
    emit({ id: "n1" });
    const clearSpy = vi.spyOn(window, "clearTimeout");
    unmount(); // effect cleanup runs synchronously
    expect(clearSpy).toHaveBeenCalled();
  });
});
