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
    eventType: "generic",
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

  it("removes an item whose wall-clock deadline has passed even if setTimeout never fired", async () => {
    // simulate system sleep / webview timer throttling: the happy-path
    // timers are stalled, but the 1s sweep sees the deadline is past.
    const now = Date.now();
    vi.setSystemTime(now);

    const { result } = await renderReady();
    emit({ id: "stale", ttlSecs: 1 });
    expect(result.current).toHaveLength(1);

    // jump past the full lifecycle deadline without advancing timers
    vi.setSystemTime(now + ENTER_DURATION_MS + 1000 + EXIT_DURATION_MS + 1);
    act(() => vi.advanceTimersByTime(1000));

    expect(result.current).toHaveLength(0);
  });

  it("defaults a missing eventType to generic", async () => {
    const { result } = await renderReady();
    act(() => {
      handlers.forEach((h) =>
        h({ payload: { id: "legacy", title: "t", body: "b", ttlSecs: 8 } as NotificationPayload }),
      );
    });
    expect(result.current[0].eventType).toBe("generic");
  });

  it("carries the eventType through to the returned state", async () => {
    const { result } = await renderReady();
    emit({ id: "goal", eventType: "score_update" });
    expect(result.current[0].eventType).toBe("score_update");
  });

  it("the deadline sweep removes several stale items in one pass", async () => {
    // two items whose deadlines have passed plus one long-ttl item: the 1s
    // sweep must remove both stale items and leave the fresh one.
    const now = Date.now();
    vi.setSystemTime(now);

    const { result } = await renderReady();
    emit({ id: "stale1", ttlSecs: 1 });
    emit({ id: "stale2", ttlSecs: 1 });
    emit({ id: "fresh", ttlSecs: 100 });
    expect(result.current).toHaveLength(3);

    vi.setSystemTime(now + ENTER_DURATION_MS + 1000 + EXIT_DURATION_MS + 1);
    act(() => vi.advanceTimersByTime(1000));

    expect(result.current.map((n) => n.id)).toEqual(["fresh"]);
  });

  it("the sweep does not remove an item before its deadline", async () => {
    // an item whose deadline is 1ms in the future when the sweep fires must
    // survive; once mocked time is advanced past the deadline the item is
    // removed.
    const now = Date.now();
    vi.setSystemTime(now);

    const { result } = await renderReady();
    emit({ id: "almost", ttlSecs: 1 });
    const deadline = now + ENTER_DURATION_MS + 1000 + EXIT_DURATION_MS;

    // advanceTimersByTime also advances the mocked system clock, so start
    // 1000ms before the desired sweep time to land 1ms before the deadline.
    vi.setSystemTime(deadline - 1001);
    act(() => vi.advanceTimersByTime(1000));
    expect(result.current.map((n) => n.id)).toEqual(["almost"]);

    // advance mocked time past the deadline and let the next sweep run
    vi.setSystemTime(deadline + 1);
    act(() => vi.advanceTimersByTime(1000));
    expect(result.current).toHaveLength(0);
  });

  it("a sweep firing while an item is already exiting does not double-remove or crash", async () => {
    // drive an item into its exit animation, then force a sweep whose
    // wall-clock time is past the item's deadline. the remove timer may
    // also fire, but the item must end up removed exactly and no error
    // thrown.
    const now = Date.now();
    vi.setSystemTime(now);

    const { result } = await renderReady();
    emit({ id: "exiting", ttlSecs: 1 });

    act(() => vi.advanceTimersByTime(ENTER_DURATION_MS));
    expect(result.current[0].phase).toBe("hold");
    act(() => vi.advanceTimersByTime(1000));
    expect(result.current[0].phase).toBe("exit");

    const deadline = now + ENTER_DURATION_MS + 1000 + EXIT_DURATION_MS;
    vi.setSystemTime(deadline + 1);
    act(() => vi.advanceTimersByTime(1000));

    expect(result.current).toHaveLength(0);
  });
});
