import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App, { BOARD_SURFACE_MOTION, RAIL_SURFACE_MOTION } from "./App";
import { BOARD_SUMMON_MS, NOTCHTAP_EASE, SURFACE_SWAP_MS } from "./animationTiming";
import { emitTo, resetHandlers } from "./test-support/tauriEventMock";
import type { AgentState } from "./useAgentState";
import type { SlotState } from "./useSlotState";

vi.mock("@tauri-apps/api/event", () => import("./test-support/tauriEventMock"));

const emit = (payload: SlotState) => act(() => emitTo("slot-state", payload));
const emitAgentState = (payload: AgentState) => act(() => emitTo("agent-state", payload));
// Every gate off, nothing queued — only `paused` is under test here, and
// `useStatusState`'s validator rejects a partial payload whole, so the
// full shape has to be supplied.
const emitStatus = (paused: boolean) =>
  act(() =>
    emitTo("status-state", {
      paused,
      waiting: 0,
      football: { enabled: false, live: null },
      news: { enabled: false },
      weather: { enabled: false, current: null },
      media: { enabled: false, current: null },
    }),
  );

const SHOWING: SlotState = {
  state: "showing",
  id: "n1",
  title: "t",
  body: "b",
  eventType: "generic",
  priority: "medium",
  signal: "generic",
  origin: "manual",
  agentRuntime: null,
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
};

function agentSession(id: string): AgentState["sessions"][number] {
  return {
    id,
    runtime: "codex",
    state: "waiting_for_permission",
    capabilities: [],
    summary: null,
    details: [],
    project: null,
    host: null,
    subagent: null,
    elapsedMs: 0,
    retentionRemainingMs: null,
    history: [],
  };
}

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
      agentRuntime: null,
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
      agentRuntime: null,
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
      agentRuntime: null,
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
      agentRuntime: null,
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

  // Plan 136 (v7 ticket 4 of 13, spec §6.1): the presentation precedence
  // machine's own integration coverage — App.tsx is `presentationMode`'s
  // one call site, so this is where "slot-occupied hides the board",
  // "board over idle", and "empty registry falls back to idle" actually
  // get exercised end to end, not just as a pure-function unit test.
  describe("Agent Board precedence (plan 136)", () => {
    it("an empty registry falls back to the existing idle rail, never mounting the board", () => {
      const { container } = render(<App />);
      emitAgentState({ revision: 1, capturedAtMs: Date.now(), sessions: [], adapterHealth: [] });
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      expect(container.querySelector('[data-testid="agent-board"]')).toBeNull();
    });

    it("shows the board over idle once at least one session exists", async () => {
      const { container } = render(<App />);
      emitAgentState({
        revision: 1,
        capturedAtMs: Date.now(),
        sessions: [agentSession("s1")],
        adapterHealth: [],
      });
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-board"]')).not.toBeNull();
      });
    });

    it("a Visible Notification hides the board even while sessions exist", async () => {
      const { container } = render(<App />);
      emitAgentState({
        revision: 1,
        capturedAtMs: Date.now(),
        sessions: [agentSession("s1")],
        adapterHealth: [],
      });
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-board"]')).not.toBeNull();
      });

      emit(SHOWING);
      await screen.findByText("t");
      expect(container.querySelector('[data-testid="agent-board"]')).toBeNull();
      expect(
        container.querySelector(".card-assembly.high, .card-assembly.medium, .card-assembly.low"),
      ).not.toBeNull();
    });

    // Operator feedback (2026-08-02): pausing notifications left the Agent
    // Board on screen, still ticking with live agent activity. Paused
    // quiets the WHOLE notch (CONTEXT.md's Paused), so the board falls
    // through to the idle rail until the engine resumes.
    it("hides the board while the engine is paused, and brings it back on resume", async () => {
      const { container } = render(<App />);
      emitAgentState({
        revision: 1,
        capturedAtMs: Date.now(),
        sessions: [agentSession("s1")],
        adapterHealth: [],
      });
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-board"]')).not.toBeNull();
      });

      emitStatus(true);
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-board"]')).toBeNull();
      });
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();

      emitStatus(false);
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-board"]')).not.toBeNull();
      });
    });

    // 2026-08-02 animation audit (finding #1): the summon's two fixes —
    // the surfaces stack in one grid cell instead of queueing in flow
    // during the overlap, and the Board branch (only the Board branch)
    // arrives with real entrance emphasis. Structure and exported consts
    // are pinned here, never mid-flight styles: jsdom runs no compositor,
    // same discipline as AgentBoard.test.tsx's own motion-vitals block.
    describe("surface swap (2026-08-02 animation audit)", () => {
      it("stacks both surfaces in one grid cell so an overlap never pushes either one down", async () => {
        const { container } = render(<App />);
        const stack = container.querySelector(".surface-stack") as HTMLElement | null;
        expect(stack).not.toBeNull();
        // the wrapper is the layout mechanism — a single-cell grid, so an
        // overlap resolves as max(height), not sum(height).
        expect(stack?.style.display).toBe("grid");
        // `.card-root` itself keeps its documented zero-geometry
        // `display: contents` scoping role (styles.css) — the stack is a
        // NEW child, not an amendment to that guarantee.
        expect(stack?.parentElement?.className).toBe("card-root");

        // both branches occupy the same cell, so neither is ever in the
        // other's flow.
        const railCell = container.querySelector(".card-assembly")?.parentElement;
        expect(railCell?.style.gridArea).toBe("1 / 1");

        emitAgentState({
          revision: 1,
          capturedAtMs: Date.now(),
          sessions: [agentSession("s1")],
          adapterHealth: [],
        });
        await vi.waitFor(() => {
          expect(container.querySelector('[data-testid="agent-board"]')).not.toBeNull();
        });
        const boardCell = container.querySelector('[data-testid="agent-board"]')?.parentElement;
        expect(boardCell?.style.gridArea).toBe("1 / 1");
        // the summon's scale grows out of the notch cutout, the one point
        // on this card that never moves.
        expect(boardCell?.style.transformOrigin).toBe("top center");
      });

      it("gives the board an entrance the routine rail swap does not have", () => {
        // The Board only appears when an agent is blocked on the operator,
        // so its arrival earns emphasis (opacity + scale + a small drop on
        // the house ease) while the rail keeps the plain crossfade.
        expect(BOARD_SURFACE_MOTION.initial).toEqual({ opacity: 0, scale: 0.97, y: -6 });
        expect(BOARD_SURFACE_MOTION.animate).toEqual({
          opacity: 1,
          scale: 1,
          y: 0,
          transition: { duration: BOARD_SUMMON_MS / 1000, ease: NOTCHTAP_EASE },
        });
        expect(RAIL_SURFACE_MOTION.initial).toEqual({ opacity: 0 });
        expect(RAIL_SURFACE_MOTION.animate).toEqual({ opacity: 1 });
      });

      it("keeps the board's dismissal quieter than its arrival (deliberate asymmetry)", () => {
        // Spatial-consistency's "mirror the exit path" rule is waived here
        // on purpose: an interruption should announce itself and then
        // leave without ceremony. The exit is opacity ONLY, on the shorter
        // shared surface-swap clock.
        expect(BOARD_SURFACE_MOTION.exit).toEqual({
          opacity: 0,
          transition: { duration: SURFACE_SWAP_MS / 1000, ease: NOTCHTAP_EASE },
        });
        expect(BOARD_SUMMON_MS).toBeGreaterThan(SURFACE_SWAP_MS);
      });
    });

    it("returns to the still-current board once the notification finishes", async () => {
      const { container } = render(<App />);
      emitAgentState({
        revision: 1,
        capturedAtMs: Date.now(),
        sessions: [agentSession("s1")],
        adapterHealth: [],
      });
      emit(SHOWING);
      await screen.findByText("t");
      expect(container.querySelector('[data-testid="agent-board"]')).toBeNull();

      emit({ state: "empty" });
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-board"]')).not.toBeNull();
      });
    });
  });
});
