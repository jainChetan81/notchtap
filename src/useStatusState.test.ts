import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { emitTo, listen, resetHandlers } from "./test-support/tauriEventMock";
import type { StatusState } from "./useStatusState";
import { useStatusState } from "./useStatusState";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

// deliberately keeps `unknown` — this file exercises malformed payloads
const emit = (payload: unknown) => act(() => emitTo("status-state", payload));

const FALLBACK: StatusState = {
  paused: false,
  waiting: 0,
  agent: { activeSessions: 0 },
  football: { enabled: false, live: null },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
};

const LIVE: StatusState = {
  paused: false,
  waiting: 3,
  agent: { activeSessions: 0 },
  football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
  news: { enabled: true, chargeFraction: 0, chargeCount: 0, isCharged: false },
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
      agent: { activeSessions: 0 },
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
      agent: { activeSessions: 0 },
      football: { enabled: "yes", live: null },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      agent: { activeSessions: 0 },
      football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: 45 } },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      agent: { activeSessions: 0 },
      football: { enabled: true, live: "Arsenal 2–0 Chelsea" },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  // --- the wire fields added to the validator ---
  //
  // The four clauses below (`agent.activeSessions`, `news.chargeFraction`
  // with its `[0, 1]` range, `news.chargeCount`, `news.isCharged`) shipped
  // with fixture coverage only. They matter more than the count suggests:
  // this validator is all-or-nothing, so a single bad field from rust
  // blanks the WHOLE status rail back to FALLBACK — every icon dark, no
  // console error, nothing naming the field that did it. These tests name
  // the fields.

  it("ignores a payload with no agent block at all", () => {
    const { agent: _agent, ...missingAgent } = LIVE;
    window.__NOTCHTAP_STATUS_STATE__ = missingAgent;
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    // ...and one that is present but not an object.
    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, agent: 3 };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("ignores a payload with a negative, fractional, or non-number agent session count", () => {
    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, agent: { activeSessions: -1 } };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, agent: { activeSessions: 1.5 } };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = { ...LIVE, agent: { activeSessions: "2" } };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("ignores a news charge fraction outside [0, 1] — the file's only range check", () => {
    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      news: { enabled: true, chargeFraction: 1.4, chargeCount: 0, isCharged: false },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      news: { enabled: true, chargeFraction: -0.1, chargeCount: 0, isCharged: false },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      news: { enabled: true, chargeFraction: "0.5", chargeCount: 0, isCharged: false },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("accepts both ends of the charge fraction — the range is inclusive", () => {
    for (const chargeFraction of [0, 1]) {
      const payload: StatusState = {
        ...LIVE,
        news: { enabled: true, chargeFraction, chargeCount: 2, isCharged: chargeFraction === 1 },
      };
      window.__NOTCHTAP_STATUS_STATE__ = payload;
      expect(renderHook(() => useStatusState()).result.current).toEqual(payload);
    }
  });

  it("ignores a negative news charge count or a non-boolean charged flag", () => {
    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      news: { enabled: true, chargeFraction: 0.5, chargeCount: -3, isCharged: false },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);

    window.__NOTCHTAP_STATUS_STATE__ = {
      ...LIVE,
      news: { enabled: true, chargeFraction: 0.5, chargeCount: 0, isCharged: "yes" },
    };
    expect(renderHook(() => useStatusState()).result.current).toEqual(FALLBACK);
  });

  it("accepts the live=null all-clear shape rust sends when nothing is in-play", () => {
    const allClear: StatusState = {
      paused: false,
      waiting: 0,
      agent: { activeSessions: 0 },
      football: { enabled: false, live: null },
      news: { enabled: true, chargeFraction: 0, chargeCount: 0, isCharged: false },
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
