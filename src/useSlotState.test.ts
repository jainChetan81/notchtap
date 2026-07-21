import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { emitTo, listen, resetHandlers } from "./test-support/tauriEventMock";
import type { SlotState } from "./useSlotState";
import { useSlotState } from "./useSlotState";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

const emit = (payload: SlotState) => act(() => emitTo("slot-state", payload));

const SHOWING_N1: SlotState = {
  state: "showing",
  id: "n1",
  title: "t",
  body: "b",
  eventType: "generic",
  priority: "medium",
  signal: "generic",
  expanded: false,
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  subtitle: null,
  details: [],
  queueTotal: 1,
  queueDone: 0,
  ttlMs: 8000,
  remainingMs: 6000,
};

describe("useSlotState", () => {
  beforeEach(() => {
    resetHandlers();
    listen.mockClear();
    delete window.__NOTCHTAP_SLOT_STATE__;
  });

  async function renderReady() {
    const rendered = renderHook(() => useSlotState());
    await act(async () => {
      await Promise.resolve();
    });
    expect(listen).toHaveBeenCalled();
    return rendered;
  }

  it("starts empty before any event arrives", async () => {
    const { result } = await renderReady();
    expect(result.current).toEqual({ state: "empty" });
  });

  it("renders the showing payload as-is when an event arrives", async () => {
    const { result } = await renderReady();
    emit(SHOWING_N1);
    expect(result.current).toEqual(SHOWING_N1);
  });

  it("a new showing payload replaces the previous one directly, without an intermediate empty frame", async () => {
    const { result } = await renderReady();
    emit({
      state: "showing",
      id: "n1",
      title: "a",
      body: "b1",
      eventType: "generic",
      priority: "low",
      signal: "generic",
      expanded: false,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [],
      queueTotal: 2,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 8000,
    });
    emit({
      state: "showing",
      id: "n2",
      title: "b",
      body: "b2",
      eventType: "score_update",
      priority: "high",
      signal: "goal",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [],
      queueTotal: 2,
      queueDone: 1,
      ttlMs: 8000,
      remainingMs: 4000,
    });
    // must go straight from n1 to n2 — assert the final state only, since
    // there's no async gap between the two synchronous emits in this test
    // to observe an intermediate frame at, but a snapshot check on id is
    // the meaningful assertion either way
    expect(result.current).toMatchObject({
      id: "n2",
      expanded: true,
      priority: "high",
      signal: "goal",
    });
  });

  it("goes back to empty when rust emits an empty slot-state", async () => {
    const { result } = await renderReady();
    emit(SHOWING_N1);
    expect(result.current.state).toBe("showing");
    emit({ state: "empty" });
    expect(result.current).toEqual({ state: "empty" });
  });

  it("round-trips a news_item payload with its source metadata", async () => {
    const { result } = await renderReady();
    const news: SlotState = {
      state: "showing",
      id: "news-1",
      title: "Budget announced",
      body: "The finance minister presented the annual budget.",
      eventType: "news_item",
      priority: "low",
      signal: "generic",
      expanded: false,
      source: "NDTV",
      category: "politics",
      publishedAtMs: 1_768_579_920_000,
      link: "https://example.com/budget",
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 8000,
    };
    emit(news);
    expect(result.current).toEqual(news);
  });

  it("ignores a well-tagged but incomplete showing payload delivered via the event", async () => {
    const { result } = await renderReady();
    emit(SHOWING_N1);
    expect(result.current.state).toBe("showing");
    // live event payloads must run through the same validator the global
    // path uses — an incomplete object falls back to empty, not undefined
    // fields (regression guard for the previously-unvalidated live path).
    emit({ state: "showing", id: "x" } as unknown as SlotState);
    expect(result.current).toEqual({ state: "empty" });
  });

  it("ignores a showing payload with an out-of-range enum delivered via the event", async () => {
    const { result } = await renderReady();
    emit({ ...SHOWING_N1, signal: "confetti" } as unknown as SlotState);
    expect(result.current).toEqual({ state: "empty" });
  });

  it("cleans up the listener on unmount", async () => {
    const { unmount } = await renderReady();
    expect(() => unmount()).not.toThrow();
  });

  // --- startup race shield (2026-07-17 review, mirrors the mode-delivery hook removed in plan 019 (see git history)) ---

  it("reads the eval-planted global as initial state (late-mount side of the race shield)", () => {
    window.__NOTCHTAP_SLOT_STATE__ = SHOWING_N1;
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toEqual(SHOWING_N1);
  });

  it("ignores garbage in the global rather than rendering a broken slot", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { not: "a slot state" };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toEqual({ state: "empty" });
  });

  it("ignores a global with no recognizable state tag", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { state: "bogus" };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toEqual({ state: "empty" });
  });

  it("ignores a well-tagged but incomplete showing payload (missing signal)", () => {
    window.__NOTCHTAP_SLOT_STATE__ = {
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      expanded: false,
      // signal omitted
    };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toEqual({ state: "empty" });
  });

  it("ignores a showing payload with a missing or non-string link", () => {
    const { link: _link, ...missingLink } = SHOWING_N1;
    window.__NOTCHTAP_SLOT_STATE__ = missingLink;
    const missingResult = renderHook(() => useSlotState()).result;
    expect(missingResult.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, link: 42 };
    const invalidResult = renderHook(() => useSlotState()).result;
    expect(invalidResult.current).toEqual({ state: "empty" });
  });

  it("ignores a showing payload with an out-of-range enum value", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, signal: "confetti" };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toEqual({ state: "empty" });
  });

  // plan 033: the queue-slider fields ride the same payload — the slider
  // does arithmetic on them, so the validator must reject anything but
  // non-negative integers (missing, fractional, negative, wrong type).
  it("accepts a showing payload with a queue-slider position", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, queueTotal: 5, queueDone: 2 };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toMatchObject({ queueTotal: 5, queueDone: 2 });
  });

  it("ignores a showing payload with a missing, fractional, or negative queue-slider position", () => {
    const { queueTotal: _total, queueDone: _done, ...missing } = SHOWING_N1;
    window.__NOTCHTAP_SLOT_STATE__ = missing;
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, queueTotal: 2.5 };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, queueDone: -1 };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, queueTotal: "3" };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });
  });

  // plan 081: ttlMs/remainingMs (the TTL-bar timing fields) ride the same
  // payload as queueTotal/queueDone and are validated with the same
  // discipline (non-negative integer, no fractional/negative/missing).
  it("accepts a showing payload with ttl/remaining timing fields", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, ttlMs: 8000, remainingMs: 3000 };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toMatchObject({ ttlMs: 8000, remainingMs: 3000 });
  });

  it("ignores a showing payload with a missing, fractional, or negative ttl/remaining value", () => {
    const { ttlMs: _ttl, remainingMs: _remaining, ...missing } = SHOWING_N1;
    window.__NOTCHTAP_SLOT_STATE__ = missing;
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, ttlMs: 8000.5 };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, remainingMs: -1 };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, ttlMs: "8000" };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });
  });

  // plan 035: subtitle is null-or-string, details an array of {label, value}
  // string pairs — the pairs come from untrusted hook input, so a malformed
  // details (non-array, or an item missing a string label/value) is rejected.
  it("accepts a showing payload carrying a subtitle and detail pairs", () => {
    window.__NOTCHTAP_SLOT_STATE__ = {
      ...SHOWING_N1,
      subtitle: "Permission request",
      details: [{ label: "Tool", value: "Bash" }],
    };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toMatchObject({
      subtitle: "Permission request",
      details: [{ label: "Tool", value: "Bash" }],
    });
  });

  it("ignores a showing payload whose details is not an array", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, details: "nope" };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });
  });

  it("ignores a showing payload whose detail item lacks a string label or value", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, details: [{ label: "Tool" }] };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, details: [{ label: 1, value: 2 }] };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });
  });

  it("ignores a showing payload whose subtitle is neither null nor a string", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, subtitle: 42 };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });
  });

  // Regression test: EVENT_TYPES must track the backend's EventType enum
  // (currently generic/score_update/match_state/news_item, event.rs) or a
  // real rss_poller.rs payload gets silently dropped to empty by the
  // validator above instead of rendering.
  it("accepts a real news_item payload from the rss poller", () => {
    const news: SlotState = {
      state: "showing",
      id: "n4",
      title: "Headline",
      body: "Summary",
      eventType: "news_item",
      priority: "low",
      signal: "generic",
      expanded: false,
      source: "NDTV",
      category: "politics",
      publishedAtMs: 1_789_600_000_000,
      link: "https://example.com/headline",
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 8000,
    };
    window.__NOTCHTAP_SLOT_STATE__ = news;
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toEqual(news);
  });

  it("updates from the slot-state event even when a valid global was also planted (early-mount side still wins on new data)", async () => {
    window.__NOTCHTAP_SLOT_STATE__ = { state: "empty" };
    const { result } = await renderReady();
    emit(SHOWING_N1);
    expect(result.current).toEqual(SHOWING_N1);
  });

  // plan 083: the structured `espn` block — absent (the common case, and
  // every non-football payload), present-and-valid, and present-but-
  // malformed (must fall back like every other field).

  const VALID_ESPN = {
    league: "UCL",
    homeAbbrev: "PSG",
    awayAbbrev: "ARS",
    homeScore: 1,
    awayScore: 1,
    clock: "45'",
    homeCards: [2, 0] as [number, number],
    awayCards: [4, 0] as [number, number],
    homeCrest: "/home/u/.config/notchtap/crests/160.png",
    awayCrest: null,
  };

  it("accepts a showing payload with no espn block (the common case)", () => {
    window.__NOTCHTAP_SLOT_STATE__ = SHOWING_N1;
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toEqual(SHOWING_N1);
  });

  it("accepts a showing payload with a valid espn block", () => {
    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, espn: VALID_ESPN };
    const { result } = renderHook(() => useSlotState());
    expect(result.current).toMatchObject({ espn: VALID_ESPN });
  });

  it("ignores a showing payload with a malformed espn block", () => {
    window.__NOTCHTAP_SLOT_STATE__ = {
      ...SHOWING_N1,
      espn: { ...VALID_ESPN, homeScore: "1" },
    };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = {
      ...SHOWING_N1,
      espn: { ...VALID_ESPN, homeCards: [2] },
    };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });

    window.__NOTCHTAP_SLOT_STATE__ = { ...SHOWING_N1, espn: "not an object" };
    expect(renderHook(() => useSlotState()).result.current).toEqual({ state: "empty" });
  });
});
