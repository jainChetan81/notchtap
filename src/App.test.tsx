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
    expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
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
      origin: "football",
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
    // plan 092: the generic branch's body class renamed `.body` ->
    // `.notif-body` (header/subtitle/body restructure).
    expect(container.querySelector(".compact .notif-body")?.textContent).toBe("1-0");
    expect(container.querySelector(".card-assembly.high")).not.toBeNull();
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
      origin: "manual",
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
    expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();
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
      origin: "manual",
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
    expect(container.querySelector(".card-assembly.expanded")).toBeNull();
  });

  it("keeps a single card element mounted through empty, showing, and empty states", async () => {
    const { container } = render(<App />);
    const card = container.querySelector(".card-assembly");

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
      origin: "manual",
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
    expect(container.querySelector(".card-assembly")).toBe(card);
    expect(card?.classList.contains("idle")).toBe(false);

    emit({ state: "empty" });
    // the outer card's "idle" class flips synchronously with the state
    // change, but the old title/body only leave the DOM once their exit
    // animation finishes — wait for that too, not just the class.
    // plan 092: the generic branch's title/body classes renamed
    // `.title`/`.body` -> `.notif-title`/`.notif-body`.
    await vi.waitFor(() => {
      expect(card?.classList.contains("idle")).toBe(true);
      expect(container.querySelector(".notif-title")).toBeNull();
    });
    expect(container.querySelector(".card-assembly")).toBe(card);
    expect(container.querySelector(".notif-body")).toBeNull();
  });

  // plan 085: the resting-state render choice rides the same appearance
  // channel as scale/radius/opacity — seeded at boot, hot-updated live.
  describe("resting_state (plan 085)", () => {
    afterEach(() => {
      delete window.__NOTCHTAP_APPEARANCE__;
    });

    // plan 105 (Step C, fixing the plan-085 bug): the shell still mounts
    // (bare) so it stays hoverable — see StatusRailCard.test.tsx's own
    // "resting_state: notch" suite for the full behavior contract. This
    // pin only checks the wiring from the boot seed through to the bare
    // render, not the whole contract.
    it("renders bare (no painted chrome) while idle when the boot seed carries resting_state: notch", () => {
      window.__NOTCHTAP_APPEARANCE__ = {
        scale: 1,
        radius: 16,
        opacity: 0.9,
        resting_state: "notch",
      };
      const { container } = render(<App />);
      expect(container.querySelector(".card-assembly.bare")).not.toBeNull();
      expect(container.querySelector(".time-only")).toBeNull();
      expect(container.querySelector(".status-dots")).toBeNull();
      expect(container.querySelector(".below-block")).toBeNull();
    });

    it("falls back to the rail when the seed omits resting_state", () => {
      window.__NOTCHTAP_APPEARANCE__ = { scale: 1, radius: 16, opacity: 0.9 };
      const { container } = render(<App />);
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    });

    it("hot-applies a live appearance-changed event without a reload", async () => {
      const { container } = render(<App />);
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();

      act(() =>
        emitTo("appearance-changed", {
          scale: 1,
          radius: 16,
          opacity: 0.9,
          resting_state: "notch",
        }),
      );
      // plan 105 (Step C): bare, not absent — see the boot-seed test above.
      await vi.waitFor(() => {
        expect(container.querySelector(".card-assembly.bare")).not.toBeNull();
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
        expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      });
    });
  });

  // plan 091: the HUD synthetic cutout vars — a notchless mac gets no
  // measured cutout from rust (mode is "hud", width/height read null),
  // so App.tsx now falls through to the fixed HUD_CUTOUT_WIDTH_PX/
  // HUD_CUTOUT_HEIGHT_PX constants instead of leaving the CSS vars unset
  // (the pre-091 behavior, when only width existed and only in notch
  // mode). Notch mode with a real measurement is unaffected — the
  // measured value always wins over the synthetic fallback.
  describe("HUD synthetic cutout vars (plan 091)", () => {
    afterEach(() => {
      delete window.__NOTCHTAP_MODE__;
      delete window.__NOTCHTAP_CUTOUT_WIDTH__;
      delete window.__NOTCHTAP_CUTOUT_HEIGHT__;
      document.documentElement.style.removeProperty("--notchtap-cutout-width");
      document.documentElement.style.removeProperty("--notchtap-cutout-height");
    });

    it("sets the synthetic 200px/32px vars in hud mode (no measured cutout)", () => {
      window.__NOTCHTAP_MODE__ = "hud";
      render(<App />);
      expect(document.documentElement.style.getPropertyValue("--notchtap-cutout-width")).toBe(
        "200px",
      );
      expect(document.documentElement.style.getPropertyValue("--notchtap-cutout-height")).toBe(
        "32px",
      );
    });

    it("uses the measured cutout in notch mode, never the hud synthetic", () => {
      window.__NOTCHTAP_MODE__ = "notch";
      window.__NOTCHTAP_CUTOUT_WIDTH__ = 319;
      window.__NOTCHTAP_CUTOUT_HEIGHT__ = 32.5;
      render(<App />);
      expect(document.documentElement.style.getPropertyValue("--notchtap-cutout-width")).toBe(
        "319px",
      );
      expect(document.documentElement.style.getPropertyValue("--notchtap-cutout-height")).toBe(
        "32.5px",
      );
    });

    it("falls through to the hud synthetic vars if notch mode never got a measurement", () => {
      // presentation.rs's own hud/fallback shape: mode reported notch is
      // impossible without a measurement in practice, but this pins the
      // null-coalescing behavior directly regardless of which mode string
      // arrived, since App.tsx's fallback is keyed on `mode === "hud"`.
      window.__NOTCHTAP_MODE__ = "hud";
      window.__NOTCHTAP_CUTOUT_WIDTH__ = null;
      window.__NOTCHTAP_CUTOUT_HEIGHT__ = null;
      render(<App />);
      expect(document.documentElement.style.getPropertyValue("--notchtap-cutout-width")).toBe(
        "200px",
      );
      expect(document.documentElement.style.getPropertyValue("--notchtap-cutout-height")).toBe(
        "32px",
      );
    });
  });
});
