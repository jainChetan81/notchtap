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
const emitStatus = (paused: boolean) =>
  act(() =>
    emitTo("status-state", {
      paused,
      waiting: 0,
      agent: { activeSessions: 0 },
      football: { enabled: false, live: null },
      news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
    }),
  );

const emitAgentTabSession = () =>
  act(() =>
    emitTo("agent-state", {
      revision: 1,
      capturedAtMs: Date.now(),
      sessions: [],
      tabSessions: [agentSession("tab-s1")],
      adapterHealth: [],
    }),
  );

const emitHover = (hovered: boolean) => act(() => emitTo("hover-changed", { hovered }));
const emitTabSelection = (payload: unknown) => act(() => emitTo("tab-selection-changed", payload));

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
    await vi.waitFor(() => {
      expect(card?.classList.contains("idle")).toBe(true);
      expect(container.querySelector(".notif-title")).toBeNull();
    });
    expect(container.querySelector(".card-assembly")).toBe(card);
    expect(container.querySelector(".notif-body")).toBeNull();
  });

  describe("resting_state", () => {
    afterEach(() => {
      delete window.__NOTCHTAP_APPEARANCE__;
    });

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
      await vi.waitFor(() => {
        expect(container.querySelector(".card-assembly.bare")).not.toBeNull();
      });

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

  describe("HUD synthetic cutout vars", () => {
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

  });

  describe("Agent Board precedence", () => {
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

    describe("surface swap", () => {
      it("stacks both surfaces in one grid cell so an overlap never pushes either one down", async () => {
        const { container } = render(<App />);
        // SAFETY: App always renders the `.surface-stack` wrapper; the
        // null-guard on the next line protects the optional cast.
        const stack = container.querySelector(".surface-stack") as HTMLElement | null;
        expect(stack).not.toBeNull();
        expect(stack?.style.display).toBe("grid");
        expect(stack?.parentElement?.className).toBe("card-root");

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
        expect(boardCell?.style.transform).toBe("");
        expect(boardCell?.style.transformOrigin).toBe("");
      });

      it("never transforms the shell — the summon's emphasis is duration only", () => {
        expect(BOARD_SURFACE_MOTION.initial).toEqual({ opacity: 0 });
        expect(BOARD_SURFACE_MOTION.animate).toEqual({
          opacity: 1,
          transition: { duration: BOARD_SUMMON_MS / 1000, ease: NOTCHTAP_EASE },
        });
        expect(RAIL_SURFACE_MOTION.initial).toEqual({ opacity: 0 });
        expect(RAIL_SURFACE_MOTION.animate).toEqual({ opacity: 1 });
        // SAFETY: each BOARD_SURFACE_MOTION leg is an object of known animation
        // props, so the array cast to Record<string, unknown>[] is a safe
        // widening the property check below reads through.
        for (const leg of [
          BOARD_SURFACE_MOTION.initial,
          BOARD_SURFACE_MOTION.animate,
          BOARD_SURFACE_MOTION.exit,
        ] as Record<string, unknown>[]) {
          for (const banned of ["scale", "x", "y", "rotate", "transform"]) {
            expect(leg, `${banned} must not animate the shell`).not.toHaveProperty(banned);
          }
        }
      });

      it("keeps the board's dismissal quieter than its arrival (deliberate asymmetry)", () => {
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

  describe("tab selection seam", () => {
    it("mounts the selected tab's below-block once hovered", async () => {
      const { container } = render(<App />);
      emitAgentTabSession();
      emitHover(true);
      emitTabSelection({ selected: "agent" });

      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-below-block"]')).not.toBeNull();
      });
      await vi.waitFor(() => {
        expect(container.querySelector(".idle-peek")).toBeNull();
        expect(container.querySelectorAll(".below-block").length).toBe(1);
      });
    });

    it("is inert until the operator hovers — a selection alone opens nothing", async () => {
      const { container } = render(<App />);
      emitAgentTabSession();
      emitTabSelection({ selected: "agent" });

      await act(async () => {
        await Promise.resolve();
      });
      expect(container.querySelector('[data-testid="agent-below-block"]')).toBeNull();
    });

    it("drops back to the ambient peek when an unknown token follows a good one", async () => {
      const { container } = render(<App />);
      emitAgentTabSession();
      emitHover(true);
      emitTabSelection({ selected: "agent" });
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-below-block"]')).not.toBeNull();
      });

      emitTabSelection({ selected: 5 });
      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-below-block"]')).toBeNull();
      });
      expect(container.querySelector(".idle-peek")).not.toBeNull();
    });
  });
});
