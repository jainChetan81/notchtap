import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { emitTo, listen, resetHandlers } from "./test-support/tauriEventMock";
import type { StatusState } from "./useStatusState";
import { statusRailActive, useStatusState } from "./useStatusState";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

// deliberately keeps `unknown` — this file exercises malformed payloads
const emit = (payload: unknown) => act(() => emitTo("status-state", payload));

const FALLBACK: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: false, current: null },
};

const LIVE: StatusState = {
  paused: false,
  waiting: 3,
  football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
  news: { enabled: true },
  weather: { enabled: false, current: null },
};

describe("useStatusState", () => {
  beforeEach(() => {
    resetHandlers();
    listen.mockClear();
    delete window.__NOTCHTAP_STATUS_STATE__;
  });

  async function renderReady() {
    const rendered = renderHook(() => useStatusState());
    await act(async () => {
      await Promise.resolve();
    });
    expect(listen).toHaveBeenCalled();
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

  it("ignores a payload with a missing weather gate or a malformed weather summary", () => {
    const { weather: _weather, ...missingWeather } = LIVE;
    window.__NOTCHTAP_STATUS_STATE__ = missingWeather;
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      weather: { enabled: "yes", current: null },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      weather: { enabled: true, current: { tempDisplay: 27, condition: "Cloudy" } },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("accepts the live=null all-clear shape rust sends when nothing is in-play", () => {
    const allClear: StatusState = {
      paused: false,
      waiting: 0,
      football: { enabled: false, live: null },
      news: { enabled: true },
      weather: { enabled: false, current: null },
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
        weather: { enabled: false, current: null },
      }),
    ).toBe(true);
    expect(
      statusRailActive({
        paused: false,
        waiting: 2,
        football: { enabled: false, live: null },
        news: { enabled: false },
        weather: { enabled: false, current: null },
      }),
    ).toBe(true);
    // a weather gate on (or a current reading) must show the rail too
    expect(
      statusRailActive({
        ...FALLBACK,
        weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy" } },
      }),
    ).toBe(true);
    expect(statusRailActive({ ...FALLBACK, paused: true })).toBe(true);
  });
});
