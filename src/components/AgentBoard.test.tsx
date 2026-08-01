import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DISCLOSURE_SPRING, NOTCHTAP_EASE } from "../animationTiming";
import type { AgentSessionView } from "../useAgentState";
import { AgentBoard, HERO_SWAP_TRANSITION, nowTickIntervalMs, ROW_TRANSITION } from "./AgentBoard";
import { MAX_VISIBLE_DETAIL_PAIRS } from "./NotificationBody";

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

  // Plan 169: the hero now renders through NotificationBody.tsx's shared
  // template — the state drives a prose `.title.headline` (there is no
  // more standalone `.agent-board-runtime`/`.agent-board-state-pill`
  // pair), runtime + project become the subtitle row, summary becomes
  // the notif-body. `waiting_for_permission` also maps to "high"
  // priority (step 6's mapping) — the NEW `--accent`/Stamp channel on
  // `.card-assembly`, a separate paint channel from the `--agent-accent`
  // one `.agent-waiting` below already drives.
  // Plan 169 fidelity pass (2026-08-02): title/subtitle are pinned to the
  // mock's own strings (`prototype/agent-board.html`, proposal section) —
  // per-state prose plus a `runtime · project` subtitle, replacing the
  // old `"Codex — Needs approval"` / bare-project pair.
  it("waiting-for-permission: amber family, hero renders through the shared template (title/subtitle/body/priority)", () => {
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
    expect(getByText("Agent needs input")).toBeTruthy();
    expect(getByText("Codex · notchtap")).toBeTruthy();
    expect(getByText("Approval needed to run a command")).toBeTruthy();
    expect(container.querySelector(".below-block.agent-waiting")).not.toBeNull();
    expect(container.querySelector(".card-assembly.high")).not.toBeNull();
  });

  it("working: blue/pulsing family, medium priority", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ state: "working" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(container.querySelector(".title.headline")?.textContent).toBe("Agent working");
    expect(container.querySelector(".below-block.agent-working")).not.toBeNull();
    expect(container.querySelector(".agent-dot.large.pulse")).not.toBeNull();
    expect(container.querySelector(".card-assembly.medium")).not.toBeNull();
  });

  it("failed: coral, non-pulsing family, high priority", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ state: "failed" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(container.querySelector(".title.headline")?.textContent).toBe("Agent session failed");
    expect(container.querySelector(".below-block.agent-failed")).not.toBeNull();
    expect(container.querySelector(".agent-dot.large.pulse")).toBeNull();
    expect(container.querySelector(".card-assembly.high")).not.toBeNull();
  });

  it("completed: green, non-pulsing family, low priority", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ state: "completed" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(container.querySelector(".title.headline")?.textContent).toBe("Agent turn completed");
    expect(container.querySelector(".below-block.agent-completed")).not.toBeNull();
    expect(container.querySelector(".agent-dot.large.pulse")).toBeNull();
    expect(container.querySelector(".card-assembly.low")).not.toBeNull();
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

  // Plan 169: project is the hero's subtitle row now (`.notif-subtitle-row`,
  // NotificationBody.tsx's shared template) — the old standalone
  // `.agent-board-project` line is gone.
  // Plan 169 fidelity pass: the subtitle is `runtime · project` and the
  // row ALWAYS renders for the hero — the runtime name lives only here
  // now (the title is per-state prose), so a session with no project
  // must still say which runtime it is, not drop the row.
  it("falls back to the runtime alone in the subtitle when a session has no project metadata", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ project: null })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    const subtitle = container.querySelector(".agent-board-primary .notif-subtitle-row");
    expect(subtitle).not.toBeNull();
    expect(subtitle?.textContent).toBe("Codex");
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

  // Plan 169 fidelity pass (2026-08-02): the board's below-block also
  // carries the SHIPPED runtime wash (`agent-origin` — card-chrome.css's
  // corner radial off `--cat-deep`, plus the runtime-coloured hairline),
  // which the mock's hero draws and the board never applied. Paired with
  // the `src-<runtime>` class above, which is what actually supplies the
  // `--cat`/`--cat-deep` pair that rule reads.
  it("the hero's below-block carries the runtime wash class", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ runtime: "claude-code", state: "working" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const board = container.querySelector(".below-block");
    expect(board?.classList.contains("agent-origin")).toBe(true);
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

  // Plan 169: the old bespoke `.agent-board-primary-head` is gone — the
  // runtime tick glyph now lives in the hero's shared masthead.
  it("renders a runtime tick glyph on the hero's masthead", () => {
    const { container } = render(
      <AgentBoard sessions={[session()]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(
      container.querySelector(".agent-board-primary .masthead .agent-runtime-tick"),
    ).not.toBeNull();
  });
});

// Plan 169: the hero's fact-pill assembly — `session.details` (the same
// capability-dependent facts `ExpandedAgentRow` already renders) plus a
// synthesized elapsed-in-state fact for starting/completed/stale (the
// Target table's "session"/"duration"/"last seen" examples), and the
// Target table's own "(danger tone)" marking on exactly two states
// (waiting_for_permission, failed). Covers the three states the earlier
// per-state describe block didn't (waiting_for_input, starting, stale),
// so all seven states have hero-render coverage somewhere in this file.
describe("AgentBoard hero fact pills (plan 169)", () => {
  // `liveElapsedMs` (AgentBoard.tsx) adds `Date.now() - capturedAtMs` on
  // top of the fixture's own `elapsedMs` — with a real wall clock and the
  // tiny fixed `CAPTURED_AT_MS` epoch every other test in this file uses,
  // that diff is enormous, not zero. Pinning the system clock to exactly
  // `CAPTURED_AT_MS` makes the diff 0, so the synthesized elapsed fact
  // pills below assert the fixture's own `elapsedMs` value directly.
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(CAPTURED_AT_MS);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("waiting-for-input: high priority, no fact pills when the session carries no details", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ state: "waiting_for_input" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector(".title.headline")?.textContent).toBe("Agent needs input");
    expect(container.querySelector(".card-assembly.high")).not.toBeNull();
    expect(container.querySelector(".agent-board-primary .detail-facts")).toBeNull();
  });

  it("starting: medium priority, synthesized 'Session' elapsed fact pill", () => {
    const { getByText, container } = render(
      <AgentBoard
        sessions={[session({ state: "starting", elapsedMs: 2_000 })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector(".card-assembly.medium")).not.toBeNull();
    expect(getByText("Session")).toBeTruthy();
    expect(getByText("2s")).toBeTruthy();
  });

  it("completed: low priority, synthesized 'Duration' elapsed fact pill", () => {
    const { getByText, container } = render(
      <AgentBoard
        sessions={[session({ state: "completed", elapsedMs: 5_000 })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector(".card-assembly.low")).not.toBeNull();
    expect(getByText("Duration")).toBeTruthy();
    expect(getByText("5s")).toBeTruthy();
  });

  it("stale: low priority, synthesized 'Last seen … ago' elapsed fact pill", () => {
    const { getByText, container } = render(
      <AgentBoard
        sessions={[session({ state: "stale", elapsedMs: 840_000 })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector(".card-assembly.low")).not.toBeNull();
    expect(getByText("Last seen")).toBeTruthy();
    expect(getByText("14m ago")).toBeTruthy();
  });

  it("waiting-for-permission: declared details (Tool/Bash) render as a danger-toned fact pill", () => {
    const { getByText, container } = render(
      <AgentBoard
        sessions={[
          session({
            state: "waiting_for_permission",
            details: [{ label: "Tool", value: "Bash" }],
          }),
        ]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(getByText("Tool")).toBeTruthy();
    expect(getByText("Bash")).toBeTruthy();
    const pill = container.querySelector(".agent-board-primary .fact-pill");
    expect(pill?.classList.contains("tone-danger")).toBe(true);
  });

  it("failed: declared details (Exit code/1) render as a danger-toned fact pill", () => {
    const { getByText, container } = render(
      <AgentBoard
        sessions={[session({ state: "failed", details: [{ label: "Exit code", value: "1" }] })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(getByText("Exit code")).toBeTruthy();
    const pill = container.querySelector(".agent-board-primary .fact-pill");
    expect(pill?.classList.contains("tone-danger")).toBe(true);
    // Plan 169 fidelity pass: a nonzero exit also earns the mock's
    // `ERROR` tag (`.fp-tag`) on that same pill.
    expect(pill?.querySelector(".fp-tag")?.textContent).toBe("error");
    expect(pill?.textContent).toBe("Exit code1error");
  });

  // Plan 169 fidelity pass: the tag is derived from the DATA, never from
  // the state alone — a failed session reporting a clean exit code (or a
  // non-numeric one) gets no `ERROR` tag.
  it("failed: a zero exit code carries no error tag", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ state: "failed", details: [{ label: "Exit code", value: "0" }] })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const pill = container.querySelector(".agent-board-primary .fact-pill");
    expect(pill?.classList.contains("tone-danger")).toBe(true);
    expect(pill?.querySelector(".fp-tag")).toBeNull();
  });

  // Plan 169 fidelity pass: the mock's `Tool rm DESTRUCTIVE` pill — a
  // declared `Risk` detail whose value reads destructive/blocked folds
  // into the `Tool` pill as its tag instead of standing as its own pill.
  it("waiting-for-permission: a destructive Risk detail folds into the Tool pill as a tag", () => {
    const { container } = render(
      <AgentBoard
        sessions={[
          session({
            state: "waiting_for_permission",
            details: [
              { label: "Tool", value: "rm" },
              { label: "Risk", value: "destructive" },
            ],
          }),
        ]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const pills = container.querySelectorAll(".agent-board-primary .fact-pill");
    expect(pills).toHaveLength(1);
    expect(pills[0].classList.contains("tone-danger")).toBe(true);
    expect(pills[0].querySelector(".fp-tag")?.textContent).toBe("destructive");
    expect(pills[0].textContent).toBe("Toolrmdestructive");
  });

  // The same guard from the other direction: a risk the table doesn't
  // flag stays an ordinary pill of its own, nothing is invented.
  it("waiting-for-permission: an unflagged Risk value stays its own untagged pill", () => {
    const { container } = render(
      <AgentBoard
        sessions={[
          session({
            state: "waiting_for_permission",
            details: [
              { label: "Tool", value: "read" },
              { label: "Risk", value: "read-only" },
            ],
          }),
        ]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const pills = container.querySelectorAll(".agent-board-primary .fact-pill");
    expect(pills).toHaveLength(2);
    expect(container.querySelector(".agent-board-primary .fp-tag")).toBeNull();
  });

  // Plan 169 fidelity pass: every non-danger state's pills are
  // `tone-accent` (the mock's own fixtures), not the neutral pill the
  // generic branch uses.
  it("working: declared details (Progress/63%) render as an accent-toned fact pill", () => {
    const { getByText, container } = render(
      <AgentBoard
        sessions={[session({ state: "working", details: [{ label: "Progress", value: "63%" }] })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(getByText("Progress")).toBeTruthy();
    expect(getByText("63%")).toBeTruthy();
    const pill = container.querySelector(".agent-board-primary .fact-pill");
    expect(pill?.classList.contains("tone-danger")).toBe(false);
    expect(pill?.classList.contains("tone-accent")).toBe(true);
    expect(pill?.querySelector(".fp-tag")).toBeNull();
  });

  it("stale: the synthesized elapsed pill is accent-toned too", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ state: "stale", elapsedMs: 840_000 })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const pill = container.querySelector(".agent-board-primary .fact-pill");
    expect(pill?.classList.contains("tone-accent")).toBe(true);
  });

  // Overflow safety: the hero's facts are capped at the SAME
  // MAX_VISIBLE_DETAIL_PAIRS limit the generic branch's own pills
  // respect (NotificationBody.tsx) — a session with more declared
  // details than the cap must never grow the card past a knowable
  // height (see manifest.css's `.detail-facts`/`.fact-pill` truncation
  // rules, verified statically per the plan's overflow-check note).
  it("caps the hero's fact pills at MAX_VISIBLE_DETAIL_PAIRS even with more declared details", () => {
    const { container } = render(
      <AgentBoard
        sessions={[
          session({
            state: "working",
            details: Array.from({ length: MAX_VISIBLE_DETAIL_PAIRS + 3 }, (_, i) => ({
              label: `Detail${i}`,
              value: `v${i}`,
            })),
          }),
        ]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelectorAll(".agent-board-primary .fact-pill")).toHaveLength(
      MAX_VISIBLE_DETAIL_PAIRS,
    );
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

  // Operator feedback (2026-08-02): `sessions[0]` is the HERO in both the
  // resting and the expanded state — only `sessions[1..]` become expanded
  // rows. Every row-level assertion below therefore puts a filler primary
  // ahead of the session actually under test, so that session is a ROW.
  // The filler carries no summary/project/host/subagent/details of its
  // own, so it can never satisfy a row assertion by accident.
  function withHero(...rows: AgentSessionView[]): AgentSessionView[] {
    return [session({ id: "hero-filler", runtime: "opencode" }), ...rows];
  }

  it("renders every retained non-primary session (8+) in the given order, none collapsed", () => {
    const sessions = manySessions(9);
    const { container } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    const rows = container.querySelectorAll('[data-testid="agent-expanded-row"]');
    // 9 sessions = 1 hero + 8 rows
    expect(rows).toHaveLength(8);
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
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
    const sessions = withHero(
      session({
        id: "a",
        history: [
          { state: "starting", elapsedMs: 60_000 },
          { state: "working", elapsedMs: 30_000 },
          { state: "waiting_for_permission", elapsedMs: 1_000 },
        ],
      }),
    );
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
      <AgentBoard
        sessions={withHero(session({ history: [] }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    fireEvent.mouseEnter(getByTestId("agent-expanded-row"));
    expect(queryByTestId("agent-expanded-history")).toBeNull();
  });

  it("capability-dependent detail cells are omitted cleanly when a session has none", () => {
    const { container } = render(
      <AgentBoard
        sessions={withHero(session({ details: [] }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-expanded-row-details")).toBeNull();
  });

  it("renders declared detail cells when present", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={withHero(session({ details: [{ label: "Tool", value: "Bash" }] }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("Tool")).toBeTruthy();
    expect(getByText("Bash")).toBeTruthy();
  });

  // Operator feedback (2026-07-27, then 2026-08-02): the hero and the
  // expanded list's first row both used to render `sessions[0]`, so at
  // N=1 the same session appeared twice. Hiding the hero while expanded
  // fixed the duplicate but broke something worse — hovering a
  // one-session Board swapped its big hero card for one skinny list row,
  // i.e. hover made the card SMALLER. The contract now: the hero stays
  // mounted in both states and the list carries `sessions[1..]` only, so
  // each session still renders exactly once at every N.
  it("keeps the hero mounted while expanded, with no row at all for a one-session board", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ id: "only", summary: "Investigating a flaky test" })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
    expect(container.querySelectorAll('[data-testid="agent-expanded-row"]')).toHaveLength(0);
    expect(container.textContent?.match(/Investigating a flaky test/g)).toHaveLength(1);
  });

  it("keeps the hero mounted while expanded on a 3-session board, with the other two as rows", () => {
    const { container } = render(
      <AgentBoard sessions={manySessions(3)} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
    expect(container.querySelectorAll('[data-testid="agent-expanded-row"]')).toHaveLength(2);
  });

  it("renders each session exactly once across a larger expanded board", () => {
    const sessions = manySessions(5);
    const { container } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
    // 5 sessions = 1 hero + 4 rows; the primary is never also a row.
    expect(container.querySelectorAll('[data-testid="agent-expanded-row"]')).toHaveLength(4);
    const rowRuntimes = Array.from(
      container.querySelectorAll('[data-testid="agent-expanded-row"] .agent-row-runtime'),
    ).map((node) => node.textContent);
    expect(rowRuntimes).toEqual(["Codex", "Kimi", "OpenCode", "Claude Code"]);
  });

  it("keeps the hero mounted across an expanded -> resting flip (hover never swaps the hero out)", () => {
    const { container, rerender } = render(
      <AgentBoard
        sessions={[session({ id: "a" }), session({ id: "b" })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
    rerender(
      <AgentBoard
        sessions={[session({ id: "a" }), session({ id: "b" })]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded={false}
      />,
    );
    // The hero never unmounts; only the rows below it swap shape, and
    // that swap is behind `AnimatePresence mode="wait"`'s exit-then-enter
    // animation (async, a real spring — same reason the history-disclosure
    // test above asserts closing INTENT rather than an immediate DOM
    // state), so only the hero's presence is asserted here.
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
  });

  // Plan 146 follow-up: richer expanded-row detail — project.cwd (home-
  // abbreviated, and only when it says more than project.name already
  // does), host.name, and a terminal-only "clears in" retention hint.
  it("renders an abbreviated cwd distinct from the project name", () => {
    const { getByText, queryByText } = render(
      <AgentBoard
        sessions={withHero(
          session({ project: { name: "notchtap", cwd: "/Users/chetanjain/code/notchtap" } }),
        )}
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
        sessions={withHero(session({ project: { name: "notchtap", cwd: "notchtap" } }))}
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
        sessions={withHero(session({ project: { name: "notchtap", cwd: null } }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-expanded-row-meta")).toBeNull();
  });

  it("renders host.name when present", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={withHero(session({ host: { name: "chetans-mac-mini", bundleId: null } }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("chetans-mac-mini")).toBeTruthy();
  });

  it("omits host metadata cleanly when absent", () => {
    const { container } = render(
      <AgentBoard
        sessions={withHero(session({ host: null }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(container.querySelector(".agent-expanded-row-meta")).toBeNull();
  });

  it("shows a 'clears in' hint for terminal sessions with a retention countdown", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={withHero(session({ state: "completed", retentionRemainingMs: 125_000 }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("clears in 2m")).toBeTruthy();
  });

  it("omits the 'clears in' hint for non-terminal sessions (retentionRemainingMs null)", () => {
    const { container } = render(
      <AgentBoard
        sessions={withHero(session({ state: "working", retentionRemainingMs: null }))}
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
        sessions={withHero(session({ runtime: "opencode", state: "waiting_for_input" }))}
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
      <AgentBoard sessions={withHero(session())} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-expanded-row-head .agent-runtime-tick")).not.toBeNull();
  });

  // Plan 147 wave 2: the subagent meta chip — label preferred, id
  // fallback, state appended in parens when present, nothing rendered
  // when the session has no active subagent.
  it("renders a subagent chip with label when present", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={withHero(
          session({ subagent: { id: "sub-1", label: "Reviewer", state: "working" } }),
        )}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("subagent: Reviewer (working)")).toBeTruthy();
  });

  it("falls back to the subagent id when label is null", () => {
    const { getByText } = render(
      <AgentBoard
        sessions={withHero(session({ subagent: { id: "sub-1", label: null, state: null } }))}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
    );
    expect(getByText("subagent: sub-1")).toBeTruthy();
  });

  it("omits the subagent chip entirely when the session has no active subagent", () => {
    const { container } = render(
      <AgentBoard
        sessions={withHero(session({ subagent: null }))}
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
    // `sessions[0]` is the hero in BOTH states (operator feedback,
    // 2026-08-02), so the three rows under test are sessions 1..3.
    const sessions = [
      session({ id: "primary", runtime: "opencode" }),
      session({ id: "a", runtime: "claude-code" }),
      session({ id: "b", runtime: "codex" }),
      session({ id: "c", runtime: "kimi" }),
    ];
    const { container, rerender } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelectorAll('[data-testid="agent-expanded-row"]')).toHaveLength(3);

    rerender(
      <AgentBoard
        sessions={[sessions[0], sessions[1], sessions[3]]}
        capturedAtMs={CAPTURED_AT_MS}
        expanded
      />,
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

// Plan 149 (motion vitals): the four fixes this plan lands — a BOUNDED
// dot pulse that restarts on state change, an accent that morphs instead
// of snapping, a hero that swaps on IDENTITY change only, and a wall-clock
// tick that adapts to what `elapsedLabel` can actually render. These pin
// structure and const values, never mid-flight styles (jsdom runs no real
// spring — same discipline as the row-fluidity block above).
describe("AgentBoard motion vitals", () => {
  const AGENT_BOARD_TSX = readFileSync(
    fileURLToPath(new NodeURL("./AgentBoard.tsx", import.meta.url)),
    "utf8",
  );
  const AGENT_BOARD_CSS = readFileSync(
    fileURLToPath(new NodeURL("../overlay/agent-board.css", import.meta.url)),
    "utf8",
  );

  it("the dot's breathe animation is BOUNDED, never infinite (plan-105 precedent)", () => {
    // an `infinite` opacity loop on a `waiting_for_input` session that
    // persists for hours is the exact always-on pulse plan 105 removed
    // from the status dots. 4 iterations ≈ 8.8s per state change.
    expect(AGENT_BOARD_CSS).toMatch(
      /animation:\s*\n?\s*agent-dot-state-tick[^;]*agent-dot-breathe/,
    );
    expect(AGENT_BOARD_CSS).toMatch(/agent-dot-breathe 2\.2s ease-in-out 4;/);
    expect(AGENT_BOARD_CSS).not.toMatch(/agent-dot-breathe[^;]*infinite/);
  });

  it("the dot morphs its accent colour and the one-shot tick is scoped to .pulse only", () => {
    // base rule: colour morph for every state (including completed/
    // failed/stale); the scale tick lives under `.pulse` so quiet states
    // stay quiet.
    expect(AGENT_BOARD_CSS).toMatch(
      /\.card-root \.agent-dot \{[^}]*transition: background-color var\(--hover-ms, 160ms\) var\(--ease-notchtap\);/s,
    );
    expect(AGENT_BOARD_CSS).toMatch(/@keyframes agent-dot-state-tick/);
    expect(AGENT_BOARD_CSS).not.toMatch(/\.card-root \.agent-dot \{[^}]*agent-dot-state-tick/s);
  });

  it("every disclosure uses the shared DISCLOSURE_SPRING — no hand-copied spring literals remain", () => {
    expect(AGENT_BOARD_TSX).not.toMatch(/stiffness:/);
    expect(AGENT_BOARD_TSX).not.toMatch(/opacity: \{ duration/);
    // three sites: the per-row history disclosure, the expanded list, the
    // resting rows block.
    expect(AGENT_BOARD_TSX.match(/transition=\{DISCLOSURE_SPRING\}/g)).toHaveLength(3);
    // ...and it is genuinely the exported token, not a look-alike.
    expect(DISCLOSURE_SPRING).toEqual({ type: "spring", stiffness: 480, damping: 37 });
  });

  it("HERO_SWAP_TRANSITION is a short house-eased tween (no spring overshoot on a text block)", () => {
    expect(HERO_SWAP_TRANSITION).toEqual({ duration: 0.16, ease: NOTCHTAP_EASE });
  });

  it("a state change on the SAME session keeps the hero mounted but remounts its dot", () => {
    const { container, rerender } = render(
      <AgentBoard
        sessions={[session({ id: "a", state: "working" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const heroBefore = container.querySelector(".agent-board-primary");
    const dotBefore = container.querySelector(".agent-board-primary .agent-dot");
    expect(heroBefore).not.toBeNull();
    expect(dotBefore?.classList.contains("pulse")).toBe(true);

    rerender(
      <AgentBoard
        sessions={[session({ id: "a", state: "completed" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );

    // the hero block itself is keyed on `primary.id` ONLY — a state change
    // must morph in place, not replay the whole swap.
    expect(container.querySelector(".agent-board-primary")).toBe(heroBefore);
    // the DOT, though, is keyed on the state, so it genuinely remounts —
    // that remount is what restarts the bounded pulse/tick.
    const dotAfter = container.querySelector(".agent-board-primary .agent-dot");
    expect(dotAfter).not.toBe(dotBefore);
    expect(dotAfter?.classList.contains("pulse")).toBe(false);
  });

  it("a compact row's dot also remounts on state change (bounded pulse restart)", () => {
    const rows = (state: AgentSessionView["state"]) => [
      session({ id: "primary" }),
      session({ id: "b", runtime: "kimi", state }),
    ];
    const { container, rerender } = render(
      <AgentBoard sessions={rows("working")} capturedAtMs={CAPTURED_AT_MS} />,
    );
    const dotBefore = container.querySelector(".agent-row .agent-dot");
    rerender(<AgentBoard sessions={rows("waiting_for_input")} capturedAtMs={CAPTURED_AT_MS} />);
    const dotAfter = container.querySelector(".agent-row .agent-dot");
    expect(dotAfter).not.toBe(dotBefore);
    // both states pulse, so the class is unchanged — only the remount
    // (and the CSS colour morph) marks the change.
    expect(dotAfter?.classList.contains("pulse")).toBe(true);
  });

  it("a DIFFERENT session becoming primary swaps the hero (identity change, not a state change)", async () => {
    const { container, rerender } = render(
      <AgentBoard
        sessions={[session({ id: "a", runtime: "claude-code" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    const heroBefore = container.querySelector(".agent-board-primary");
    // Plan 169: runtime is folded into the hero's shared template now
    // (there is no more standalone `.agent-board-runtime`); the fidelity
    // pass moved it specifically into the subtitle row, since the title
    // is per-state prose that never names the runtime.
    expect(heroBefore?.querySelector(".notif-subtitle")?.textContent).toContain("Claude Code");

    rerender(
      <AgentBoard
        sessions={[session({ id: "b", runtime: "opencode" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );

    // `mode="wait"` means the outgoing hero holds the slot until its exit
    // completes, so the new content arrives asynchronously — the swap is
    // a real animation, not a same-frame content replacement.
    await waitFor(() => {
      const heroAfter = container.querySelector(".agent-board-primary");
      expect(heroAfter?.querySelector(".notif-subtitle")?.textContent).toContain("OpenCode");
      expect(heroAfter).not.toBe(heroBefore);
    });
  });

  it("nowTickIntervalMs: fast while any session is inside elapsedLabel's second-granular window", () => {
    const now = CAPTURED_AT_MS;
    expect(nowTickIntervalMs([session({ elapsedMs: 5_000 })], CAPTURED_AT_MS, now)).toBe(1000);
    // one slow session doesn't drag the board off the fast tick
    expect(
      nowTickIntervalMs(
        [session({ id: "a", elapsedMs: 900_000 }), session({ id: "b", elapsedMs: 1_000 })],
        CAPTURED_AT_MS,
        now,
      ),
    ).toBe(1000);
  });

  it("nowTickIntervalMs: slow once every session is past the 60s minute boundary", () => {
    const now = CAPTURED_AT_MS;
    expect(
      nowTickIntervalMs(
        [session({ id: "a", elapsedMs: 60_000 }), session({ id: "b", elapsedMs: 3_600_000 })],
        CAPTURED_AT_MS,
        now,
      ),
    ).toBe(15_000);
    // and the LIVE elapsed is what counts, not the wire snapshot: a 59s
    // snapshot captured 5s ago is already past the boundary.
    expect(
      nowTickIntervalMs([session({ elapsedMs: 59_000 })], CAPTURED_AT_MS, CAPTURED_AT_MS + 5_000),
    ).toBe(15_000);
  });

  it("the board subscribes at the slow rate when nothing is second-granular", () => {
    const setInterval = vi.spyOn(window, "setInterval");
    try {
      render(
        <AgentBoard
          sessions={[session({ id: "a", elapsedMs: 300_000 })]}
          capturedAtMs={Date.now()}
        />,
      );
      const delays = setInterval.mock.calls.map((call) => call[1]);
      expect(delays).toContain(15_000);
      expect(delays).not.toContain(1000);
    } finally {
      setInterval.mockRestore();
    }
  });

  it("the board subscribes at the fast rate while a session is fresh", () => {
    const setInterval = vi.spyOn(window, "setInterval");
    try {
      render(
        <AgentBoard
          sessions={[session({ id: "a", elapsedMs: 2_000 })]}
          capturedAtMs={Date.now()}
        />,
      );
      expect(setInterval.mock.calls.map((call) => call[1])).toContain(1000);
    } finally {
      setInterval.mockRestore();
    }
  });
});
