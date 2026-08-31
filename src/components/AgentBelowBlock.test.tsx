import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { AgentSessionView } from "../useAgentState";
import { AgentBelowBlock, cycleSessionIndex } from "./AgentBelowBlock";

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

describe("cycleSessionIndex", () => {
  it("advances to the next index", () => {
    expect(cycleSessionIndex(0, 3, "next")).toBe(1);
  });

  it("wraps from the last index back to the first on next", () => {
    expect(cycleSessionIndex(2, 3, "next")).toBe(0);
  });

  it("moves to the previous index", () => {
    expect(cycleSessionIndex(2, 3, "previous")).toBe(1);
  });

  it("wraps from the first index back to the last on previous", () => {
    expect(cycleSessionIndex(0, 3, "previous")).toBe(2);
  });

  it("returns 0 for a non-positive total rather than dividing by zero", () => {
    expect(cycleSessionIndex(0, 0, "next")).toBe(0);
  });

  it("is a no-op cycle (returns the same index) for a single-session total", () => {
    expect(cycleSessionIndex(0, 1, "next")).toBe(0);
    expect(cycleSessionIndex(0, 1, "previous")).toBe(0);
  });
});

describe("AgentBelowBlock", () => {
  it("renders nothing when there are zero sessions", () => {
    const { container } = render(
      <AgentBelowBlock
        sessions={[]}
        viewedIndex={0}
        capturedAtMs={CAPTURED_AT_MS}
        nowMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector('[data-testid="agent-below-block"]')).toBeNull();
  });

  it("renders the viewed session's hero, not necessarily sessions[0]", () => {
    const { container } = render(
      <AgentBelowBlock
        sessions={[
          session({ id: "a", runtime: "codex", project: { name: "alpha", cwd: null } }),
          session({ id: "b", runtime: "claude-code", project: { name: "beta", cwd: null } }),
        ]}
        viewedIndex={1}
        capturedAtMs={CAPTURED_AT_MS}
        nowMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector(".notif-subtitle")?.textContent).toContain("beta");
  });

  it("clamps an out-of-range viewedIndex instead of crashing on a stale index", () => {
    const { container } = render(
      <AgentBelowBlock
        sessions={[session({ id: "a" }), session({ id: "b" })]}
        viewedIndex={99}
        capturedAtMs={CAPTURED_AT_MS}
        nowMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.querySelector('[data-testid="agent-below-block"]')).not.toBeNull();
    const segments = Array.from(container.querySelectorAll(".ttl-bar > *"));
    expect(segments[1].className).toBe("ttl-fill");
  });
});
