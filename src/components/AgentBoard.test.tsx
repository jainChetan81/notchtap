import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DISCLOSURE_SPRING, NOTCHTAP_EASE } from "../animationTiming";
import type { AgentSessionView } from "../useAgentState";
import { AgentBoard, HERO_SWAP_TRANSITION, nowTickIntervalMs, ROW_TRANSITION } from "./AgentBoard";
import { MAX_VISIBLE_DETAIL_PAIRS } from "./NotificationBody";

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

describe("AgentBoard resting render", () => {
  it("renders nothing when there are zero sessions (defense in depth)", () => {
    const { container } = render(<AgentBoard sessions={[]} capturedAtMs={CAPTURED_AT_MS} />);
    expect(container.querySelector('[data-testid="agent-board"]')).toBeNull();
  });

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

  it("renders 3+ sessions as individual rows, never a +N collapse, in the given (Rust) order", () => {
    const sessions = [
      session({ id: "a", runtime: "claude-code", state: "waiting_for_permission" }),
      session({ id: "b", runtime: "codex", state: "failed" }),
      session({ id: "c", runtime: "kimi", state: "working" }),
      session({ id: "d", runtime: "opencode", state: "completed" }),
    ];
    const { container } = render(<AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} />);
    const rows = container.querySelectorAll(".agent-row");
    expect(rows).toHaveLength(3);
    const rowRuntimes = Array.from(rows).map(
      (row) => row.querySelector(".agent-row-runtime")?.textContent,
    );
    expect(rowRuntimes).toEqual(["Codex", "Kimi", "OpenCode"]);
    expect(container.textContent).not.toMatch(/\+\d/);
  });

  it("falls back to the runtime alone in the subtitle when a session has no project metadata", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ project: null })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    const subtitle = container.querySelector(".agent-board-primary .notif-subtitle-row");
    expect(subtitle).not.toBeNull();
    expect(subtitle?.textContent).toBe("Codex");
  });

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

  it("renders a runtime tick glyph on the hero's masthead", () => {
    const { container } = render(
      <AgentBoard sessions={[session()]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    expect(
      container.querySelector(".agent-board-primary .masthead .agent-runtime-tick"),
    ).not.toBeNull();
  });
});

describe("AgentBoard hero fact pills", () => {
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
    expect(pill?.querySelector(".fp-tag")?.textContent).toBe("error");
    expect(pill?.textContent).toBe("Exit code1error");
  });

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

  function withHero(...rows: AgentSessionView[]): AgentSessionView[] {
    return [session({ id: "hero-filler", runtime: "opencode" }), ...rows];
  }

  it("renders every retained non-primary session (8+) in the given order, none collapsed", () => {
    const sessions = manySessions(9);
    const { container } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    const rows = container.querySelectorAll('[data-testid="agent-expanded-row"]');
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

  it("renders each session exactly once across a larger expanded board", () => {
    const sessions = manySessions(5);
    const { container } = render(
      <AgentBoard sessions={sessions} capturedAtMs={CAPTURED_AT_MS} expanded />,
    );
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
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
    expect(container.querySelector(".agent-board-primary")).not.toBeNull();
  });

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

describe("AgentBoard row removal/insertion/reorder fluidity", () => {
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

describe("AgentBoard motion vitals", () => {
  const AGENT_BOARD_TSX = readFileSync(
    fileURLToPath(new NodeURL("./AgentBoard.tsx", import.meta.url)),
    "utf8",
  );
  const AGENT_BOARD_CSS = readFileSync(
    fileURLToPath(new NodeURL("../overlay/agent-board.css", import.meta.url)),
    "utf8",
  );

  it("the dot's breathe animation is BOUNDED, never infinite", () => {
    expect(AGENT_BOARD_CSS).toMatch(
      /animation:\s*\n?\s*agent-dot-state-tick[^;]*agent-dot-breathe/,
    );
    expect(AGENT_BOARD_CSS).toMatch(/agent-dot-breathe 2\.2s ease-in-out 4;/);
    expect(AGENT_BOARD_CSS).not.toMatch(/agent-dot-breathe[^;]*infinite/);
  });

  it("the dot morphs its accent colour and the one-shot tick is scoped to .pulse only", () => {
    expect(AGENT_BOARD_CSS).toMatch(
      /\.card-root \.agent-dot \{[^}]*transition: background-color var\(--reveal-ms, 260ms\) var\(--ease-notchtap\);/s,
    );
    expect(AGENT_BOARD_CSS).toMatch(/@keyframes agent-dot-state-tick/);
    expect(AGENT_BOARD_CSS).not.toMatch(/\.card-root \.agent-dot \{[^}]*agent-dot-state-tick/s);
  });

  it("every board-scoped accent consumer morphs on the same --reveal-ms clock", () => {
    const revealTransition = /var\(--reveal-ms, 260ms\) var\(--ease-notchtap\)/;
    for (const selector of [
      /\.card-root \.agent-board-primary \.compact::before \{([^}]*)\}/,
      /\.card-root \.agent-board-primary \.stamp \{([^}]*)\}/,
      /\.card-root \.agent-board-primary \.fact-pill \.fp-tag \{([^}]*)\}/,
      /\.card-root \.agent-row-state \{([^}]*)\}/,
    ]) {
      const rule = AGENT_BOARD_CSS.match(selector);
      expect(rule, `no rule matched ${selector}`).not.toBeNull();
      expect(rule?.[1]).toMatch(/transition:/);
      expect(rule?.[1]).toMatch(revealTransition);
    }
    expect(AGENT_BOARD_CSS).not.toMatch(/var\(--hover-ms/);
  });

  it("every disclosure uses the shared DISCLOSURE_SPRING — no hand-copied spring literals remain", () => {
    expect(AGENT_BOARD_TSX).not.toMatch(/stiffness:/);
    expect(AGENT_BOARD_TSX).not.toMatch(/opacity: \{ duration/);
    expect(AGENT_BOARD_TSX.match(/transition=\{DISCLOSURE_SPRING\}/g)).toHaveLength(3);
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

    expect(container.querySelector(".agent-board-primary")).toBe(heroBefore);
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
    expect(heroBefore?.querySelector(".notif-subtitle")?.textContent).toContain("Claude Code");

    rerender(
      <AgentBoard
        sessions={[session({ id: "b", runtime: "opencode" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );

    await waitFor(() => {
      const heroAfter = container.querySelector(".agent-board-primary");
      expect(heroAfter?.querySelector(".notif-subtitle")?.textContent).toContain("OpenCode");
      expect(heroAfter).not.toBe(heroBefore);
    });
  });

  it("nowTickIntervalMs: fast while any session is inside elapsedLabel's second-granular window", () => {
    const now = CAPTURED_AT_MS;
    expect(nowTickIntervalMs([session({ elapsedMs: 5_000 })], CAPTURED_AT_MS, now)).toBe(1000);
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
    expect(
      nowTickIntervalMs([session({ elapsedMs: 59_000 })], CAPTURED_AT_MS, CAPTURED_AT_MS + 5_000),
    ).toBe(15_000);
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

describe("AgentBoard resting content-hug", () => {
  const AGENT_BOARD_CSS = readFileSync(
    fileURLToPath(new NodeURL("../overlay/agent-board.css", import.meta.url)),
    "utf8",
  );

  it("a one-session board mounts neither block the bottom-inset rule looks for", () => {
    const { container } = render(
      <AgentBoard sessions={[session({ id: "solo" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    const board = container.querySelector(".agent-board");
    expect(board).not.toBeNull();
    expect(board?.querySelector(".agent-board-rows")).toBeNull();
    expect(board?.querySelector(".agent-board-expanded-list")).toBeNull();
  });

  it("a multi-session resting board still mounts the rows block that owes the inset", () => {
    const { container } = render(
      <AgentBoard
        sessions={[session({ id: "a" }), session({ id: "b" })]}
        capturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector(".agent-board .agent-board-rows")).not.toBeNull();
  });

  it("the hero-only board drops the container's bottom padding", () => {
    expect(AGENT_BOARD_CSS).toMatch(
      /\.card-root \.agent-board:not\(:has\(\.agent-board-rows, \.agent-board-expanded-list\)\) \{\s*padding-bottom: 0;/,
    );
  });

  it("the hero's fact pills lay out as a wrapping ROW with no TTL-bar clearance", () => {
    expect(AGENT_BOARD_CSS).toMatch(
      /\.card-root \.agent-board-primary \.detail-facts \{[^}]*flex-direction: row;[^}]*flex-wrap: wrap;[^}]*margin-bottom: 0;/s,
    );
  });

  it("the shell's permanent `expanded` class stops dragging in a taller compact", () => {
    expect(AGENT_BOARD_CSS).toMatch(
      /\.card-root \.agent-board-primary \.compact \{\s*min-height: 74px;/,
    );
  });
});

describe("AgentBoard resting<->expanded morph", () => {
  const AGENT_BOARD_TSX = readFileSync(
    fileURLToPath(new NodeURL("./AgentBoard.tsx", import.meta.url)),
    "utf8",
  );
  const AGENT_BOARD_CSS = readFileSync(
    fileURLToPath(new NodeURL("../overlay/agent-board.css", import.meta.url)),
    "utf8",
  );

  const twoSessions = [session({ id: "a" }), session({ id: "b" })];

  it("puts both branches in one grid cell so they overlap instead of stacking", () => {
    const { container, rerender } = render(
      <AgentBoard sessions={twoSessions} capturedAtMs={CAPTURED_AT_MS} />,
    );
    const swap = container.querySelector(".agent-board-swap");
    expect(swap).not.toBeNull();
    expect(swap?.parentElement?.classList.contains("agent-board")).toBe(true);

    // SAFETY: the two-session fixture renders `.agent-board-rows` beneath the
    // swap wrapper asserted non-null above; the optional cast stays null-safe.
    const resting = container.querySelector(".agent-board-rows") as HTMLElement | null;
    expect(resting?.parentElement).toBe(swap);
    expect(resting?.style.gridArea).toBe("1 / 1");
    expect(resting?.style.overflow).toBe("hidden");

    rerender(<AgentBoard sessions={twoSessions} capturedAtMs={CAPTURED_AT_MS} expanded />);
    // SAFETY: rerendering with `expanded` makes AgentBoard mount the
    // `.agent-board-expanded-list` surface in place of the rows block.
    const expandedList = container.querySelector(
      '[data-testid="agent-board-expanded-list"]',
    ) as HTMLElement | null;
    expect(expandedList?.parentElement).toBe(swap);
    expect(expandedList?.style.gridArea).toBe("1 / 1");
    expect(expandedList?.style.overflow).toBe("hidden");
    expect(container.querySelector(".agent-board-expanded-scroll")).not.toBeNull();
  });

  it('does not serialise the swap — `mode="wait"` is absent from this block', () => {
    expect(AGENT_BOARD_TSX.match(/<AnimatePresence[^>]*mode="wait"/g)).toHaveLength(1);
    expect(AGENT_BOARD_TSX).toMatch(
      /<div className="agent-board-swap">\s*<AnimatePresence initial=\{false\}>/,
    );
  });

  it("drives both branches' height off the SAME spring, so max() traces one curve", () => {
    for (const cls of ["agent-board-expanded-list", "agent-board-rows"]) {
      const branch = AGENT_BOARD_TSX.match(
        new RegExp(`className="${cls}"[\\s\\S]{0,400}?/>|className="${cls}"[\\s\\S]{0,400}?>`),
      );
      expect(branch, `no ${cls} branch found`).not.toBeNull();
      expect(branch?.[0]).toContain('animate={{ opacity: 1, height: "auto" }}');
      expect(branch?.[0]).toContain("exit={{ opacity: 0, height: 0 }}");
      expect(branch?.[0]).toContain("transition={DISCLOSURE_SPRING}");
    }
  });

  it("the wrapper is a top-aligned single-cell grid", () => {
    expect(AGENT_BOARD_CSS).toMatch(
      /\.card-root \.agent-board-swap \{\s*display: grid;\s*align-items: start;\s*\}/,
    );
  });

  it("an empty wrapper is removed from layout so it can't collect the board's flex gap", () => {
    expect(AGENT_BOARD_CSS).toMatch(
      /\.card-root \.agent-board:not\(:has\(\.agent-board-rows, \.agent-board-expanded-list\)\)\s*\.agent-board-swap \{\s*display: none;/,
    );
    const { container } = render(
      <AgentBoard sessions={[session({ id: "solo" })]} capturedAtMs={CAPTURED_AT_MS} />,
    );
    const swap = container.querySelector(".agent-board-swap");
    expect(swap).not.toBeNull();
    expect(swap?.childElementCount).toBe(0);
  });
});
