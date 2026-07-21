import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { emitTo, resetHandlers } from "./test-support/tauriEventMock";
import type { SlotState } from "./useSlotState";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

const emit = (payload: SlotState) => act(() => emitTo("slot-state", payload));

describe("App", () => {
  beforeEach(() => {
    resetHandlers();
  });

  // this project's vitest config doesn't set `test.globals`, so RTL's
  // auto-cleanup (hooked off a global `afterEach`) never registers.
  afterEach(cleanup);

  it("renders the idle pill without notification content when the slot is empty", () => {
    const { container } = render(<App />);
    expect(container.querySelector(".rail-card.idle")).not.toBeNull();
    expect(container.querySelector(".title")).toBeNull();
    expect(container.querySelector(".body")).toBeNull();
  });

  it("renders title, body, and the priority class when showing", async () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "GOAL",
      body: "1-0",
      eventType: "score_update",
      priority: "high",
      signal: "goal",
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
      remainingMs: 8000,
    });
    expect(await screen.findByText("GOAL")).toBeTruthy();
    // plan 078: the collapsed manifest stays mounted (aria-hidden), so the
    // body text also appears in its Message cell — assert on the compact
    // view's copy specifically.
    expect(container.querySelector(".compact .body")?.textContent).toBe("1-0");
    expect(container.querySelector(".rail-card.high")).not.toBeNull();
  });

  it("applies the expanded class only when expanded is true", async () => {
    const { container } = render(<App />);
    emit({
      state: "showing",
      id: "n1",
      title: "t",
      body: "b",
      eventType: "generic",
      priority: "medium",
      signal: "generic",
      expanded: true,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 8000,
    });
    await screen.findByText("t");
    expect(container.querySelector(".rail-card.expanded")).not.toBeNull();
  });

  it("does not apply the expanded class when expanded is false", async () => {
    const { container } = render(<App />);
    emit({
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
      remainingMs: 8000,
    });
    await screen.findByText("t");
    expect(container.querySelector(".rail-card.expanded")).toBeNull();
  });

  it("keeps a single card element mounted through empty, showing, and empty states", async () => {
    const { container } = render(<App />);
    const card = container.querySelector(".rail-card");

    expect(card).not.toBeNull();
    expect(card?.classList.contains("idle")).toBe(true);

    emit({
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
      remainingMs: 8000,
    });
    await screen.findByText("t");
    expect(container.querySelector(".rail-card")).toBe(card);
    expect(card?.classList.contains("idle")).toBe(false);

    emit({ state: "empty" });
    // the outer card's "idle" class flips synchronously with the state
    // change, but the old title/body only leave the DOM once their exit
    // animation finishes — wait for that too, not just the class.
    await vi.waitFor(() => {
      expect(card?.classList.contains("idle")).toBe(true);
      expect(container.querySelector(".title")).toBeNull();
    });
    expect(container.querySelector(".rail-card")).toBe(card);
    expect(container.querySelector(".body")).toBeNull();
  });

  // plan 085: the resting-state render choice rides the same appearance
  // channel as scale/radius/opacity — seeded at boot, hot-updated live.
  describe("resting_state (plan 085)", () => {
    afterEach(() => {
      delete window.__NOTCHTAP_APPEARANCE__;
    });

    it("renders nothing while idle when the boot seed carries resting_state: notch", () => {
      window.__NOTCHTAP_APPEARANCE__ = {
        scale: 1,
        radius: 16,
        opacity: 0.9,
        resting_state: "notch",
      };
      const { container } = render(<App />);
      expect(container.querySelector(".rail-card")).toBeNull();
    });

    it("falls back to the rail when the seed omits resting_state", () => {
      window.__NOTCHTAP_APPEARANCE__ = { scale: 1, radius: 16, opacity: 0.9 };
      const { container } = render(<App />);
      expect(container.querySelector(".rail-card.idle")).not.toBeNull();
    });

    it("hot-applies a live appearance-changed event without a reload", async () => {
      const { container } = render(<App />);
      expect(container.querySelector(".rail-card.idle")).not.toBeNull();

      act(() =>
        emitTo("appearance-changed", {
          scale: 1,
          radius: 16,
          opacity: 0.9,
          resting_state: "notch",
        }),
      );
      await vi.waitFor(() => {
        expect(container.querySelector(".rail-card")).toBeNull();
      });

      // and back — the toggle isn't a one-way ratchet
      act(() =>
        emitTo("appearance-changed", {
          scale: 1,
          radius: 16,
          opacity: 0.9,
          resting_state: "rail",
        }),
      );
      await vi.waitFor(() => {
        expect(container.querySelector(".rail-card.idle")).not.toBeNull();
      });
    });
  });
});
