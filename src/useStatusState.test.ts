import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StatusState } from "./useStatusState";
import { statusRailActive, useStatusState } from "./useStatusState";

type Handler = (event: { payload: unknown }) => void;
const handlers: Handler[] = [];

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((_name: string, handler: Handler) => {
    handlers.push(handler);
    return Promise.resolve(() => {});
  }),
}));

function emit(payload: unknown) {
  act(() => {
    handlers.forEach((h) => {
      h({ payload });
    });
  });
}

const FALLBACK: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
};

const LIVE: StatusState = {
  paused: false,
  waiting: 3,
  football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
  news: { enabled: true },
};

describe("useStatusState", () => {
  beforeEach(() => {
    handlers.length = 0;
    delete window.__NOTCHTAP_STATUS_STATE__;
  });

  async function renderReady() {
    const rendered = renderHook(() => useStatusState());
    await act(async () => {
      await Promise.resolve();
    });
    expect(handlers.length).toBeGreaterThan(0);
    return rendered;
  }

  it("starts at the all-gates-off fallback before any event arrives", async () => {
    const { result } = await renderReady();
    expect(result.current).toEqual(FALLBACK);
  });

  it("renders a valid payload as-is when an event arrives", async () => {
    const { result } = await renderReady();
    emit(LIVE);
    expect(result.current).toEqual(LIVE);
  });

  it("a new payload replaces the previous one directly", async () => {
    const { result } = await renderReady();
    emit(LIVE);
    emit({ ...LIVE, waiting: 2, football: { enabled: true, live: null } });
    expect(result.current).toEqual({
      ...LIVE,
      waiting: 2,
      football: { enabled: true, live: null },
    });
  });

  it("an invalid payload delivered via the event falls back instead of rendering broken fields", async () => {
    const { result } = await renderReady();
    emit(LIVE);
    expect(result.current).toEqual(LIVE);
    // same contract as the slot-state hook: live event payloads run
    // through the validator too — a partial object falls back whole.
    emit({ paused: false, waiting: 1 });
    expect(result.current).toEqual(FALLBACK);
  });

  it("cleans up the listener on unmount", async () => {
    const { unmount } = await renderReady();
    expect(() => unmount()).not.toThrow();
  });

  // --- startup race shield (mirrors the slot-state hook's global-seed tests) ---

  it("reads the eval-planted global as initial state (late-mount side of the race shield)", () => {
    window.__NOTCHTAP_STATUS_STATE__ = LIVE;
    const { result } = renderHook(() => useStatusState());
    expect(result.current).toEqual(LIVE);
  });

  it("ignores garbage in the global rather than rendering a broken rail", () => {
    window.__NOTCHTAP_STATUS_STATE__ = { not: "a status state" };
    const { result } = renderHook(() => useStatusState());
    expect(result.current).toEqual(FALLBACK);
  });

  it("ignores a payload with a non-boolean paused or a missing news gate", () => {
    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, paused: "no" };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    const { news: _news, ...missingNews } = LIVE;
    window.__NOTCHTAP_STATUS_STATE__ = missingNews;
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("ignores a payload with a missing, fractional, negative, or non-number waiting count", () => {
    const { waiting: _waiting, ...missing } = LIVE;
    window.__NOTCHTAP_STATUS_STATE__ = missing;
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, waiting: 2.5 };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, waiting: -1 };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, waiting: "3" };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("ignores a payload with a non-boolean football gate or a malformed live match", () => {
    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      football: { enabled: "yes", live: null },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: 45 } },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      football: { enabled: true, live: "Arsenal 2–0 Chelsea" },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("accepts the live=null all-clear shape rust sends when nothing is in-play", () => {
    const allClear: StatusState = {
      paused: false,
      waiting: 0,
      football: { enabled: false, live: null },
      news: { enabled: true },
    };
    window.__NOTCHTAP_STATUS_STATE__ = allClear;
    const { result } = renderHook(() => useStatusState());
    expect(result.current).toEqual(allClear);
  });

  it("updates from the status-state event even when a valid global was also planted", async () => {
    window.__NOTCHTAP_STATUS_STATE__ = FALLBACK;
    const { result } = await renderReady();
    emit(LIVE);
    expect(result.current).toEqual(LIVE);
  });
});

describe("statusRailActive", () => {
  it("is false only when every gate is off, nothing is waiting, and not paused", () => {
    expect(statusRailActive(FALLBACK)).toBe(false);
  });

  it("is true for a live match, a source gate on, a backlog, or a paused engine", () => {
    expect(statusRailActive(LIVE)).toBe(true);
    // the manual all-clear check (espn off, rss on, idle) must show the rail
    expect(
      statusRailActive({
        paused: false,
        waiting: 0,
        football: { enabled: false, live: null },
        news: { enabled: true },
      }),
    ).toBe(true);
    expect(
      statusRailActive({
        paused: false,
        waiting: 2,
        football: { enabled: false, live: null },
        news: { enabled: false },
      }),
    ).toBe(true);
    expect(statusRailActive({ ...FALLBACK, paused: true })).toBe(true);
  });
});
