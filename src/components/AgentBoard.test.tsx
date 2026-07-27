import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { AgentSessionView } from "../useAgentState";
import { AgentBoard, ROW_TRANSITION } from "./AgentBoard";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (hooked off a global `afterEach`) never registers.
afterEach(cleanup);

const CAPTURED_AT_MS = 1_000_000;

function session(overrides: Partial<AgentSessionView> = {}): AgentSessionView {
  return {
    id: "hash-1",
    runtime: "codex",
    state: "working",
    capabilities: ["session_lifecycle"],
    summary: null,
    details: [],
    project: null,
    host: null,
    subagent: null,
    elapsedMs: 5_000,
    retentionRemainingMs: null,
    history: [],
    ...overrides,
  };
}

// Plan 136 (v7 ticket 4 of 13, spec §6.2 resting): resting render
// coverage for each of the four+one non-alarming state families, plus
// the "3+ sessions, never a +N collapse, Rust order preserved" contract.
describe("AgentBoard resting render", () => {
  it("renders nothing when there are zero sessions (defense in depth)", () => {
    const { container } = render(<AgentBoard sessions={[]} capturedAtMs={CAPTURED_AT_MS} />);
    expect(container.querySelector('[data-testid="agent-board"]')).toBeNull();
  });

  it("waiting-for-permission: amber family, runtime/project/summary/state all render", () => {
    const { container, getByText } = render(
      <AgentBoard
        sessions={[
          session({
            state: "waiting_for_permission",
            project: { name: "notchtap", cwd: "/repo" },
            summary: "Approval needed to run a command",
          }),
        ]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(getByText("Codex")).toBeTruthy();
    expect(getByText("Needs approval")).toBeTruthy();
    expect(getByText("notchtap")).toBeTruthy();
    expect(getByText("Approval needed to run a command")).toBeTruthy();
    expect(container.querySelector(".below-block.agent-waiting")).not.toBeNull();
  });

  it("working: blue/pulsing family", () => {
    const { container, getByText } = render(
      <AgentBoard sessions={[session({ state: "working" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(getByText("Working")).toBeTruthy();
    expect(container.querySelector(".below-block.agent-working")).not.toBeNull();
    expect(container.querySelector(".agent-dot.large.pulse")).not.toBeNull();
  });

  it("failed: coral, non-pulsing family", () => {
    const { container, getByText } = render(
      <AgentBoard sessions={[session({ state: "failed" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(getByText("Failed")).toBeTruthy();
    expect(container.querySelector(".below-block.agent-failed")).not.toBeNull();
    expect(container.querySelector(".agent-dot.large.pulse")).toBeNull();
  });

  it("completed: green, non-pulsing family", () => {
    const { container, getByText } = render(
      <AgentBoard sessions={[session({ state: "completed" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(getByText("Completed")).toBeTruthy();
    expect(container.querySelector(".below-block.agent-completed")).not.toBeNull();
    expect(container.querySelector(".agent-dot.large.pulse")).toBeNull();
  });

  it("renders 3+ sessions as individual rows, never a +N collapse, in the given (Rust) order", () => {
    const sessions = [
      session({ id: "a", runtime: "claude-code", state: "waiting_for_permission" }),
      session({ id: "b", runtime: "codex", state: "failed" }),
      session({ id: "c", runtime: "kimi", state: "working" }),
      session({ id: "d", runtime: "opencode", state: "completed" }),
    ];
    const { container } = render(<AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} />);
    // the primary card carries the FIRST session (a) — the remaining
    // three (b, c, d) are individual compact rows, in that exact order.
    const rows = container.querySelectorAll(".agent-row");
    expect(rows).toHaveLength(3);
    const rowRuntimes = Array.from(rows).map(
      (row) => row.querySelector(".agent-row-runtime")?.textContent,
    );
    expect(rowRuntimes).toEqual(["Codex", "Kimi", "OpenCode"]);
    // never a "+N" collapse anywhere in the rendered output
    expect(container.textContent).not.toMatch(/\+\d/);
  });

  it("omits the project line cleanly when a session has no project metadata", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ project: null })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(container.querySelector(".agent-board-project")).toBeNull();
  });

  // Plan 147 wave 2: state accents (agent-waiting/agent-working/...) and
  // runtime identity (src-claude-code/src-kimi/...) are two independent
  // paint channels that must coexist on the same row — never one
  // replacing the other.
  it("a compact row carries both the state class and the runtime class simultaneously", () => {
    const { container } = render(
      <AgentBoard
        sessions={[
          session({ id: "primary", state: "waiting_for_permission" }),
          session({ id: "b", runtime: "kimi", state: "working" }),
        ]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const row = container.querySelector(".agent-row");
    expect(row?.classList.contains("agent-working")).toBe(true);
    expect(row?.classList.contains("src-kimi")).toBe(true);
  });

  it("the hero block carries both the state class and the runtime class simultaneously", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ runtime: "claude-code", state: "failed" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const board = container.querySelector(".below-block");
    expect(board?.classList.contains("agent-failed")).toBe(true);
    expect(board?.classList.contains("src-claude-code")).toBe(true);
  });

  it("renders a runtime tick glyph on each compact row", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ id: "primary" }), session({ id: "b" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelectorAll(".agent-row .agent-runtime-tick")).toHaveLength(1);
  });

  it("renders a runtime tick glyph on the hero head", () => {
    const { container } = render(
      <AgentBoard sessions={[session()]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(container.querySelector(".agent-board-primary-head .agent-runtime-tick")).not.toBeNull();
  });
});

// Plan 142 (v7 ticket 10 of 13, spec §6.2 expanded): the hover-expanded
// state's own render coverage — every retained session in the given
// order, per-row history disclosure, capability-omitted cells, and a
// bounded scroll container present.
describe("AgentBoard expanded render", () => {
  function manySessions(count: number): AgentSessionView[] {
    return Array.from({ length: count }, (_, i) =>
      session({
        id: `s${i}`,
        runtime: (["claude-code", "codex", "kimi", "opencode"] as const)[i % 4],
        state: "working",
      }),
    );
  }

  it("renders every retained session (8+) in the given order, none collapsed", () => {
    const sessions = manySessions(9);
    const { container } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    const rows = container.querySelectorAll('[data-testid="agent-expanded-row"]');
    expect(rows).toHaveLength(9);
    expect(container.textContent).not.toMatch(/\+\d/);
  });

  it("provides a bounded, scrollable container for the expanded list", () => {
    const { container } = render(
      <AgentBoard sessions={manySessions(3)} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-board-expanded-scroll")).not.toBeNull();
  });

  it("resting (non-expanded) render never shows the expanded list or its rows", () => {
    const { container } = render(
      <AgentBoard sessions={manySessions(3)} capturedAtMs={CAPTURED_AT_MS} expanded={false} />,
    );
    expect(container.querySelector('[data-testid="agent-board-expanded-list"]')).toBeNull();
    expect(container.querySelectorAll('[data-testid="agent-expanded-row"]')).toHaveLength(0);
  });

  it("a row's transition history is hidden until that row is hovered, then discloses oldest first", () => {
    const sessions = [
      session({
        id: "a",
        history: [
          { state: "starting", elapsedMs: 60_000 },
          { state: "working", elapsedMs: 30_000 },
          { state: "waiting_for_permission", elapsedMs: 1_000 },
        ],
      }),
    ];
    const { container, getByTestId, queryByTestId } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(queryByTestId("agent-expanded-history")).toBeNull();

    const row = getByTestId("agent-expanded-row");
    fireEvent.mouseEnter(row);
    const entries = container.querySelectorAll(".agent-expanded-history-entry");
    expect(entries).toHaveLength(3);
    const states = Array.from(entries).map(
      (e) => e.querySelector(".agent-expanded-history-state")?.textContent,
    );
    expect(states).toEqual(["Starting", "Working", "Needs approval"]);

    // motion's exit animation is async (a real spring, not instant) —
    // assert the CLOSING intent (opacity/height animating toward 0)
    // rather than an immediate unmount, which only `AnimatePresence`'s
    // eventual (post-animation) removal would satisfy.
    fireEvent.mouseLeave(row);
    const closing = queryByTestId("agent-expanded-history");
    expect(closing === null || closing.getAttribute("style")?.includes("opacity: 0")).toBeTruthy();
  });

  it("a row with no transition history renders no history section even when hovered", () => {
    const { getByTestId, queryByTestId } = render(
      <AgentBoard sessions={[session({ history: [] })]} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    fireEvent.mouseEnter(getByTestId("agent-expanded-row"));
    expect(queryByTestId("agent-expanded-history")).toBeNull();
  });

  it("capability-dependent detail cells are omitted cleanly when a session has none", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ details: [] })]} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-expanded-row-details")).toBeNull();
  });

  it("renders declared detail cells when present", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={[session({ details: [{ label: "Tool", value: "Bash" }] })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("Tool")).toBeTruthy();
    expect(getByText("Bash")).toBeTruthy();
  });

  // Operator feedback (2026-07-27): with the OLD implementation the hero
  // block (resting's primary-session summary) rendered unconditionally,
  // so at N=1 the same session appeared twice — once as the hero, once
  // as the first (only) expanded row. `expanded` must replace the hero
  // entirely, not sit alongside it.
  it("hides the hero block while expanded, showing sessions[0] only once, as an expanded row", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ id: "only", summary: "Investigating a flaky test" })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-board-primary")).toBeNull();
    const rows = container.querySelectorAll('[data-testid="agent-expanded-row"]');
    expect(rows).toHaveLength(1);
    expect(container.textContent?.match(/Investigating a flaky test/g)).toHaveLength(1);
  });

  it("renders each session exactly once across a larger expanded board", () => {
    const sessions = manySessions(5);
    const { container } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-board-primary")).toBeNull();
    expect(container.querySelectorAll('[data-testid="agent-expanded-row"]')).toHaveLength(5);
  });

  it("shows the hero block again once expanded flips back to false (resting unchanged)", () => {
    const { container, rerender } = render(
      <AgentBoard
        sessions={[session({ id: "a" }), session({ id: "b" })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-board-primary")).toBeNull();
    rerender(
      <AgentBoard
        sessions={[session({ id: "a" }), session({ id: "b" })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded={false}
      />,
    );
    // The hero reappears synchronously; the compact `rest` row swap is
    // behind `AnimatePresence mode="wait"`'s exit-then-enter animation
    // (async, a real spring — same reason the history-disclosure test
    // above asserts closing INTENT rather than an immediate DOM state),
    // so only the hero's presence is asserted here.
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
  });

  // Plan 146 follow-up: richer expanded-row detail — project.cwd (home-
  // abbreviated, and only when it says more than project.name already
  // does), host.name, and a terminal-only "clears in" retention hint.
  it("renders an abbreviated cwd distinct from the project name", () => {
    const { getByText, queryByText } = render(
      <AgentBoard
        sessions={[
          session({ project: { name: "notchtap", cwd: "/Users/chetanjain/code/notchtap" } }),
        ]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("~/code/notchtap")).toBeTruthy();
    expect(queryByText("/Users/chetanjain/code/notchtap")).toBeNull();
  });

  it("omits the cwd line when it duplicates the project name", () => {
    const { container, queryByText } = render(
      <AgentBoard
        sessions={[session({ project: { name: "notchtap", cwd: "notchtap" } })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(queryByText("notchtap")).toBeTruthy();
    // no duplicate rendering of the (identical) cwd string as a second node
    expect(container.querySelectorAll(".agent-expanded-meta-item")).toHaveLength(0);
  });

  it("omits the cwd line entirely when project metadata has no cwd", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ project: { name: "notchtap", cwd: null } })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-expanded-row-meta")).toBeNull();
  });

  it("renders host.name when present", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={[session({ host: { name: "chetans-mac-mini", bundleId: null } })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("chetans-mac-mini")).toBeTruthy();
  });

  it("omits host metadata cleanly when absent", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ host: null })]} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-expanded-row-meta")).toBeNull();
  });

  it("shows a 'clears in' hint for terminal sessions with a retention countdown", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={[session({ state: "completed", retentionRemainingMs: 125_000 })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("clears in 2m")).toBeTruthy();
  });

  it("omits the 'clears in' hint for non-terminal sessions (retentionRemainingMs null)", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ state: "working", retentionRemainingMs: null })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-expanded-row-meta")).toBeNull();
  });

  // Plan 147 wave 2: expanded rows also carry both paint channels at
  // once (state accent + runtime identity), and get their own runtime
  // tick glyph in the row head.
  it("an expanded row carries both the state class and the runtime class simultaneously", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ runtime: "opencode", state: "waiting_for_input" })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    const row = container.querySelector('[data-testid="agent-expanded-row"]');
    expect(row?.classList.contains("agent-waiting")).toBe(true);
    expect(row?.classList.contains("src-opencode")).toBe(true);
  });

  it("renders a runtime tick glyph on the expanded row head", () => {
    const { container } = render(
      <AgentBoard sessions={[session()]} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-expanded-row-head .agent-runtime-tick")).not.toBeNull();
  });

  // Plan 147 wave 2: the subagent meta chip — label preferred, id
  // fallback, state appended in parens when present, nothing rendered
  // when the session has no active subagent.
  it("renders a subagent chip with label when present", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={[session({ subagent: { id: "sub-1", label: "Reviewer", state: "working" } })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("subagent: Reviewer (working)")).toBeTruthy();
  });

  it("falls back to the subagent id when label is null", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={[session({ subagent: { id: "sub-1", label: null, state: null } })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("subagent: sub-1")).toBeTruthy();
  });

  it("omits the subagent chip entirely when the session has no active subagent", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ subagent: null })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-expanded-row-meta")).toBeNull();
  });
});

// Plan 147 follow-up (operator feedback, 2026-07-27): removal (and by
// symmetry insertion/reorder) of a session row used to pop — the row
// unmounted instantly and siblings jumped into place. `AgentRow` and the
// expanded list's per-row wrapper now go through `AnimatePresence` with a
// shared `ROW_TRANSITION`, mirroring how the `agent-expanded-history`
// disclosure above already asserts CLOSING INTENT (a real, async spring)
// rather than an immediate unmount.
describe("AgentBoard row removal/insertion/reorder fluidity", () => {
  // Pins the one shared const so enter/exit/layout can never hand-copy-drift
  // apart from each other (CLAUDE.md's `dedup_eq` desynced-clocks failure
  // class, generalized to motion transitions) — critically damped
  // (`bounce: 0`, no overshoot) because rows carry no gesture momentum to
  // preserve on exit, per the apple-design "Designing Fluid Interfaces"
  // derivation cited on the const itself.
  it("ROW_TRANSITION is a critically damped spring (no overshoot) shared by enter/exit/layout", () => {
    expect(ROW_TRANSITION).toEqual({ type: "spring", bounce: 0, duration: 0.35 });
  });

  it("a removed resting row leaves its siblings' stable keys/content intact, and either unmounts or is visibly closing", () => {
    const sessions = [
      session({ id: "primary" }),
      session({ id: "a", runtime: "claude-code" }),
      session({ id: "b", runtime: "codex" }),
      session({ id: "c", runtime: "kimi" }),
    ];
    const { container, rerender } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(container.querySelectorAll(".agent-row")).toHaveLength(3);

    // remove "b" (a stale-eviction / retention-expiry / snapshot-drop
    // shaped update — the same session set minus one entry)
    rerender(
      <AgentBoard
        sessions={[sessions[0], sessions[1], sessions[3]]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );

    const rows = container.querySelectorAll(".agent-row");
    const runtimes = Array.from(rows).map(
      (row) => row.querySelector(".agent-row-runtime")?.textContent,
    );
    // "b" (Codex) is either already gone (AnimatePresence's exit completed
    // synchronously in this environment) or still present but rendered
    // through the motion-controlled wrapper (an inline `style` attribute —
    // jsdom doesn't run the actual spring, so the exact opacity/height
    // mid-flight isn't assertable, but a plain instantly-popped `<div>`
    // would carry no such style at all).
    if (runtimes.includes("Codex")) {
      expect(runtimes).toEqual(["Claude Code", "Codex", "Kimi"]);
      const exitingRow = Array.from(rows).find(
        (row) => row.querySelector(".agent-row-runtime")?.textContent === "Codex",
      );
      expect(exitingRow?.getAttribute("style")).toBeTruthy();
    } else {
      expect(runtimes).toEqual(["Claude Code", "Kimi"]);
    }
  });

  it("an inserted resting row is present with the new session's content (mirrors exit path — no instant pop-in check possible in jsdom, structure only)", () => {
    const sessions = [session({ id: "primary" }), session({ id: "a", runtime: "claude-code" })];
    const { container, rerender } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(container.querySelectorAll(".agent-row")).toHaveLength(1);

    rerender(
      <AgentBoard
        sessions={[...sessions, session({ id: "new", runtime: "opencode" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );

    const rows = container.querySelectorAll(".agent-row");
    const runtimes = Array.from(rows).map(
      (row) => row.querySelector(".agent-row-runtime")?.textContent,
    );
    expect(runtimes).toEqual(["Claude Code", "OpenCode"]);
  });

  it("a removed expanded row leaves its siblings' stable keys/content intact, and either unmounts or is visibly closing", () => {
    const sessions = [
      session({ id: "a", runtime: "claude-code" }),
      session({ id: "b", runtime: "codex" }),
      session({ id: "c", runtime: "kimi" }),
    ];
    const { container, rerender } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelectorAll('[data-testid="agent-expanded-row"]')).toHaveLength(3);

    rerender(
      <AgentBoard sessions={[sessions[0], sessions[2]]} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );

    const rows = container.querySelectorAll('[data-testid="agent-expanded-row"]');
    if (rows.length === 3) {
      const exitingRow = Array.from(rows).find(
        (row) => row.querySelector(".agent-row-runtime")?.textContent === "Codex",
      );
      // the motion-controlled `style` attribute lives on the row's
      // motion.div WRAPPER, not the `agent-expanded-row` element itself
      // (see `AgentBoard.tsx`'s `sessions.map` — the wrapper carries
      // `initial`/`animate`/`exit`, `ExpandedAgentRow` renders the content
      // inside it unchanged). jsdom doesn't run the actual spring, so only
      // presence of that style (motion-controlled, not an instant pop) is
      // assertable here.
      expect(exitingRow?.parentElement?.getAttribute("style")).toBeTruthy();
    } else {
      expect(rows).toHaveLength(2);
    }
  });

  it("reordered sessions (rank change) render in the new Rust-given order with stable per-session content", () => {
    const sessions = [
      session({ id: "primary" }),
      session({ id: "a", runtime: "claude-code" }),
      session({ id: "b", runtime: "codex" }),
    ];
    const { container, rerender } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(
      Array.from(container.querySelectorAll(".agent-row")).map(
        (row) => row.querySelector(".agent-row-runtime")?.textContent,
      ),
    ).toEqual(["Claude Code", "Codex"]);

    // rust re-ranked: "b" now outranks "a" among the `rest` sessions
    rerender(
      <AgentBoard
        sessions={[sessions[0], sessions[2], sessions[1]]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(
      Array.from(container.querySelectorAll(".agent-row")).map(
        (row) => row.querySelector(".agent-row-runtime")?.textContent,
      ),
    ).toEqual(["Codex", "Claude Code"]);
  });
});
