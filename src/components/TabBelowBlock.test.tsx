import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { AgentSessionView } from "../useAgentState";
import type { NowPlayingSummary, StatusState } from "../useStatusState";
import { TabBelowBlock, tabBelowBlockHandles } from "./TabBelowBlock";

afterEach(cleanup);

const CAPTURED_AT_MS = 1_000_000;
const NOW_MS = CAPTURED_AT_MS + 5_000;

const QUIET: StatusState = {
  paused: false,
  waiting: 0,
  agent: { activeSessions: 0 },
  football: { enabled: false, live: null },
  news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
  weather: { enabled: false, current: null },
  media: { enabled: false, current: null },
};

const TRACK: NowPlayingSummary = {
  title: "Midnight City",
  artist: "M83",
  album: "Hurry Up, We're Dreaming",
  playing: true,
  elapsedMs: 1500,
  durationMs: 243_000,
  capturedAtMs: CAPTURED_AT_MS,
  appBundleId: "app.zen-browser.zen",
};

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

function renderTab(
  selected: Parameters<typeof TabBelowBlock>[0]["selected"],
  status: StatusState = QUIET,
  sessions: AgentSessionView[] = [],
) {
  return render(
    <TabBelowBlock
      selected={selected}
      status={status}
      agentSessions={sessions}
      agentCapturedAtMs={CAPTURED_AT_MS}
      nowMs={NOW_MS}
    />,
  );
}

describe("tabBelowBlockHandles (plan 171, slice K)", () => {
  it("claims exactly the three tabs with their own below-block component", () => {
    expect(tabBelowBlockHandles("agent")).toBe(true);
    expect(tabBelowBlockHandles("music")).toBe(true);
    expect(tabBelowBlockHandles("news")).toBe(true);
  });

  // football/weather are served by IdleHoverPeek's own shipped rendering
  // instead (spec §7's weather bullet, §11's untouched-mechanism rule) —
  // see TabBelowBlock.tsx's header comment for the full split.
  it("does not claim football or weather, which reuse the shipped hover peek", () => {
    expect(tabBelowBlockHandles("football")).toBe(false);
    expect(tabBelowBlockHandles("weather")).toBe(false);
  });

  it("does not claim the no-selection state", () => {
    expect(tabBelowBlockHandles(null)).toBe(false);
  });
});

describe("TabBelowBlock (plan 171, slice K)", () => {
  // spec §7's "none" page: this must fall out of the conditional, not be
  // a case built for it.
  it("renders nothing with no selection", () => {
    const { container } = renderTab(null);
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing for the two tabs the hover peek serves", () => {
    expect(renderTab("football").container.firstChild).toBeNull();
    cleanup();
    expect(renderTab("weather").container.firstChild).toBeNull();
  });

  describe("agent", () => {
    it("mounts the agent below-block for the viewed session", () => {
      const { container } = renderTab("agent", { ...QUIET, agent: { activeSessions: 1 } }, [
        session(),
      ]);
      expect(container.querySelector('[data-testid="agent-below-block"]')).not.toBeNull();
    });

    it("defaults to the first session — nothing on the wire moves the viewed index yet", () => {
      const { container } = renderTab("agent", { ...QUIET, agent: { activeSessions: 2 } }, [
        session({ id: "a", runtime: "codex" }),
        session({ id: "b", runtime: "claude-code" }),
      ]);
      // AgentBelowBlock keys the runtime wash off the VIEWED session, so
      // the rendered class is how "which one is viewed" is observable.
      expect(container.querySelector('[data-testid="agent-below-block"]')?.className).toContain(
        "src-codex",
      );
    });

    it("honours an explicit viewed index, so the eventual prefix wiring is a caller change only", () => {
      const { container } = render(
        <TabBelowBlock
          selected="agent"
          status={{ ...QUIET, agent: { activeSessions: 2 } }}
          agentSessions={[session({ id: "a" }), session({ id: "b", runtime: "claude-code" })]}
          agentCapturedAtMs={CAPTURED_AT_MS}
          nowMs={NOW_MS}
          viewedSessionIndex={1}
        />,
      );
      expect(container.querySelector('[data-testid="agent-below-block"]')?.className).toContain(
        "src-claude-code",
      );
    });

    it("renders nothing when the agent source has gone quiet", () => {
      const { container } = renderTab("agent", QUIET, []);
      expect(container.firstChild).toBeNull();
    });
  });

  describe("music", () => {
    it("mounts the media below-block off the now-playing snapshot", () => {
      const { container } = renderTab("music", {
        ...QUIET,
        media: { enabled: true, current: TRACK },
      });
      expect(container.querySelector('[data-testid="media-below-block"]')).not.toBeNull();
      expect(screen.getByText("Midnight City")).toBeTruthy();
    });

    it("renders nothing when nothing is playing", () => {
      const { container } = renderTab("music", QUIET);
      expect(container.firstChild).toBeNull();
    });

    // spec §10: transport dispatch is rust's job. The buttons exist for
    // the press feedback and the accessible name; clicking one must not
    // throw, and there is no invoke() anywhere behind it.
    it("renders working transport buttons whose handler is a safe no-op", () => {
      renderTab("music", { ...QUIET, media: { enabled: true, current: TRACK } });
      const next = screen.getByLabelText("Next track");
      expect(() => next.click()).not.toThrow();
    });

    it("stays compact by default — spec §2 decision 6 forbids auto-expanding", () => {
      const { container } = renderTab("music", {
        ...QUIET,
        media: { enabled: true, current: TRACK },
      });
      expect(container.querySelector(".media-scrub")).toBeNull();
    });
  });

  describe("news", () => {
    // Flagged gap, not a bug: `StatusState.news` carries the charge cycle
    // only — no story content exists on the wire at this commit, so
    // NewsBelowBlock's own "zero stories renders nothing" floor is what
    // mounts. The charge itself is still visible, on the news GLYPH.
    it("renders nothing today, because no story wire exists yet", () => {
      const { container } = renderTab("news", {
        ...QUIET,
        news: { enabled: true, chargeFraction: 1, chargeCount: 4, isCharged: true },
      });
      expect(container.firstChild).toBeNull();
    });
  });

  it("tolerates a missing status wire entirely (settings preview / older callers)", () => {
    const { container } = render(
      <TabBelowBlock
        selected="music"
        status={undefined}
        agentSessions={[]}
        agentCapturedAtMs={CAPTURED_AT_MS}
      />,
    );
    expect(container.firstChild).toBeNull();
  });
});
