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
      agent: { activeSessions: 0 },
      football: { enabled: false, live: null },
      news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
    }),
  );

// an agent session on the UNGATED tab list only —
// `sessions` stays empty so `presentationMode` keeps the idle rail (not
// the Agent Board) mounted, while the agent tab's below-block has real
// content to render. The seam test needs content to distinguish "the
// selection arrived" from "the selection arrived and rendered nothing".
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
    // the collapsed manifest stays mounted (aria-hidden), so the
    // body text also appears in its Message cell — assert on the compact
    // view's copy specifically.
    // the generic branch's body class renamed `.body` ->
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
    // the generic branch's title/body classes renamed
    // `.title`/`.body` -> `.notif-title`/`.notif-body`.
    await vi.waitFor(() => {
      expect(card?.classList.contains("idle")).toBe(true);
      expect(container.querySelector(".notif-title")).toBeNull();
    });
    expect(container.querySelector(".card-assembly")).toBe(card);
    expect(container.querySelector(".notif-body")).toBeNull();
  });

  // the resting-state render choice rides the same appearance
  // channel as scale/radius/opacity — seeded at boot, hot-updated live.
  describe("resting_state", () => {
    afterEach(() => {
      delete window.__NOTCHTAP_APPEARANCE__;
    });

    // the shell still mounts
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
      // bare, not absent — see the boot-seed test above.
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

  // the HUD synthetic cutout vars — a notchless mac gets no
  // measured cutout from rust (mode is "hud", width/height read null),
  // so App.tsx now falls through to the fixed HUD_CUTOUT_WIDTH_PX/
  // HUD_CUTOUT_HEIGHT_PX constants instead of leaving the CSS vars unset
  // (the pre-091 behavior, when only width existed and only in notch
  // mode). Notch mode with a real measurement is unaffected — the
  // measured value always wins over the synthetic fallback.
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

  // the presentation precedence
  // machine's own integration coverage — App.tsx is `presentationMode`'s
  // one call site, so this is where "slot-occupied hides the board",
  // "board over idle", and "empty registry falls back to idle" actually
  // get exercised end to end, not just as a pure-function unit test.
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

    // Paused quiets the WHOLE notch, so the Agent Board falls through to
    // the idle rail until the engine resumes — it never stays on screen
    // ticking with live agent activity.
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

    // The surfaces stack in one grid cell instead of queueing in flow
    // during the overlap, and the Board branch (only the Board branch)
    // arrives on a longer clock — with NO transform on the shell, since
    // that would move/resize the synthetic notch cutout. Structure and
    // exported consts are pinned here, never mid-flight styles: jsdom
    // runs no compositor, same discipline as AgentBoard.test.tsx's own
    // motion-vitals block.
    describe("surface swap", () => {
      it("stacks both surfaces in one grid cell so an overlap never pushes either one down", async () => {
        const { container } = render(<App />);
        // SAFETY: App always renders the `.surface-stack` wrapper; the
        // null-guard on the next line protects the optional cast.
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
        // ...and nothing else: the surface cell carries no transform of
        // its own (see the next test).
        expect(boardCell?.style.transform).toBe("");
        expect(boardCell?.style.transformOrigin).toBe("");
      });

      it("never transforms the shell — the summon's emphasis is duration only", () => {
        // A scale/drop entrance on this wrapper would animate the
        // synthetic notch cutout along with the rest of the shell, and
        // that cutout has to read as fixed hardware (card-chrome.css's
        // `transform-origin` doc). Both surfaces are opacity-only; the
        // Board's arrival earns its emphasis from the LONGER clock.
        expect(BOARD_SURFACE_MOTION.initial).toEqual({ opacity: 0 });
        expect(BOARD_SURFACE_MOTION.animate).toEqual({
          opacity: 1,
          transition: { duration: BOARD_SUMMON_MS / 1000, ease: NOTCHTAP_EASE },
        });
        expect(RAIL_SURFACE_MOTION.initial).toEqual({ opacity: 0 });
        expect(RAIL_SURFACE_MOTION.animate).toEqual({ opacity: 1 });
        // no transform-family key anywhere in the board's three legs.
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

  // `useTabSelection`'s ONE production call site.
  // `useTabSelection.test.ts` proves the hook validates and stores; this
  // proves App.tsx actually subscribes to the right channel and threads
  // the result down to `StatusRailCard`'s `selectedTab` prop. Neither of
  // those wiring mistakes is loud: drop the prop or mistype the channel
  // name and the overlay degrades to "clicking an icon does nothing" with
  // the whole suite still green, because every layer's own failure mode
  // is the silent "nothing is selected" page.
  //
  // Three events are needed to reach the seam, and all three are real
  // rust-emitted channels, not test scaffolding: an agent-state wire with
  // a session on it (the below-block renders nothing without content),
  // the hover that opens the tab pull at all (`tabPullOpen = !showing &&
  // hovered`, StatusRailCard.tsx), and the selection itself.
  describe("tab selection seam", () => {
    it("mounts the selected tab's below-block once hovered", async () => {
      const { container } = render(<App />);
      emitAgentTabSession();
      emitHover(true);
      emitTabSelection({ selected: "agent" });

      await vi.waitFor(() => {
        expect(container.querySelector('[data-testid="agent-below-block"]')).not.toBeNull();
      });
      // ...and the ambient peek yields to it, leaving exactly one
      // `.below-block` under the shell once the swap settles — the
      // invariant `card-chrome.css`'s `:not(:has(.below-block))` rounding
      // law depends on (StatusRailCard.tsx's `peekOpen` doc). Waited on,
      // not asserted synchronously: the peek is an AnimatePresence child,
      // so it is still playing its exit collapse at the instant the new
      // card mounts. The overlap is the crossfade, by design.
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

    it("ignores an unknown tab token instead of rendering a broken page", async () => {
      const { container } = render(<App />);
      emitAgentTabSession();
      emitHover(true);
      emitTabSelection({ selected: "definitely-not-a-tab" });

      await act(async () => {
        await Promise.resolve();
      });
      // The coercion this pins is SILENT by design (`useTabSelection`'s
      // closed-set check against `TAB_ORDER` returns "nothing selected"
      // for anything it does not recognise, never an error) — which is
      // also why `tabWireParity.test.ts` exists: rust-side drift in the
      // wire tokens would land here and read as a working app that
      // simply never selects anything.
      expect(container.querySelector('[data-testid="agent-below-block"]')).toBeNull();
      // ...and the shipped ambient peek is what fills the gap, unchanged.
      expect(container.querySelector(".idle-peek")).not.toBeNull();
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
