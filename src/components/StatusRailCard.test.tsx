import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { StatusRailCard } from "./StatusRailCard";
import type { SlotState } from "../useSlotState";

// lottie-web touches HTMLCanvasElement.getContext at import time, which
// jsdom doesn't implement — stub the player rather than pull in the
// native `canvas` package just for tests.
vi.mock("lottie-react", () => ({ default: () => null }));

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers —
// without this, DOM from one test's render leaks into the next and
// screen.getByText finds duplicates across tests.
afterEach(cleanup);

const GOAL: SlotState = {
  state: "showing",
  id: "n1",
  title: "GOAL",
  body: "Arsenal 2-0",
  eventType: "score_update",
  priority: "high",
  signal: "goal",
  expanded: true,
  source: null,
  category: null,
  publishedAtMs: null,
};

const RED_CARD: SlotState = {
  state: "showing",
  id: "n2",
  title: "Red Card",
  body: "Chelsea down to 10",
  eventType: "match_state",
  priority: "high",
  signal: "red_card",
  expanded: true,
  source: null,
  category: null,
  publishedAtMs: null,
};

const CMUX_NEEDS_INPUT: SlotState = {
  state: "showing",
  id: "n3",
  title: "Claude Code needs input",
  body: "Workspace command is waiting",
  eventType: "generic",
  priority: "high",
  signal: "generic",
  expanded: true,
  source: null,
  category: null,
  publishedAtMs: null,
};

const NEWS: SlotState = {
  state: "showing",
  id: "news-1",
  title: "Parliament passes the landmark digital rights bill",
  body: "The measure passed after a late-night vote.",
  eventType: "news_item",
  priority: "low",
  signal: "generic",
  expanded: true,
  source: "NDTV",
  category: "politics",
  publishedAtMs: 2_000_000_000_000 - 5 * 60_000,
};

describe("StatusRailCard", () => {
  describe("goal/red-card pulse", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("applies pulse-goal and mounts the celebration on a goal signal", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".rail-card.pulse-goal")).not.toBeNull();
      expect(container.querySelector(".goal-celebration")).not.toBeNull();
    });

    it("clears pulse-goal after its duration", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      act(() => {
        vi.advanceTimersByTime(620);
      });
      expect(container.querySelector(".pulse-goal")).toBeNull();
    });

    it("applies pulse-red (without mounting the goal celebration) on a red-card signal", () => {
      const { container } = render(<StatusRailCard slot={RED_CARD} />);
      expect(container.querySelector(".rail-card.pulse-red")).not.toBeNull();
      expect(container.querySelector(".goal-celebration")).toBeNull();
    });

    it("clears pulse-red after its duration", () => {
      const { container } = render(<StatusRailCard slot={RED_CARD} />);
      act(() => {
        vi.advanceTimersByTime(920);
      });
      expect(container.querySelector(".pulse-red")).toBeNull();
    });

    it("does not replay the pulse on an unrelated re-render of the same notification", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      act(() => {
        vi.advanceTimersByTime(620);
      });
      expect(container.querySelector(".pulse-goal")).toBeNull();

      // same id + signal, only `expanded` flips — must not replay the burst
      rerender(<StatusRailCard slot={{ ...GOAL, expanded: false }} />);
      expect(container.querySelector(".pulse-goal")).toBeNull();
    });
  });

  // The actual reason EventSignal exists: a High-priority alert from a
  // non-football source (cmux) must never be mistaken for a goal just
  // because both happen to be High priority.
  it("never plays the goal celebration or a pulse for a High-priority generic signal", () => {
    const { container } = render(<StatusRailCard slot={CMUX_NEEDS_INPUT} />);
    expect(container.querySelector(".pulse-goal")).toBeNull();
    expect(container.querySelector(".pulse-red")).toBeNull();
    expect(container.querySelector(".goal-celebration")).toBeNull();
  });

  it("renders the idle clock, not a card, when the slot is empty", () => {
    const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
    expect(container.querySelector(".rail-card.idle")).not.toBeNull();
    expect(container.querySelector(".idle-view")).not.toBeNull();
    expect(screen.queryByText("GOAL")).toBeNull();
  });

  it("renders the priority class and expanded class when showing", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    expect(container.querySelector(".rail-card.high.expanded")).not.toBeNull();
    expect(screen.getByText("GOAL")).toBeTruthy();
    // "Arsenal 2-0" legitimately appears twice while expanded — once in
    // the compact preview, once in the manifest's "Message" detail.
    expect(screen.getAllByText("Arsenal 2-0").length).toBe(2);
  });

  it("renders the news masthead, headline, category, age, and category shader classes", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={NEWS} />);

    expect(screen.getByText("NDTV · Wire")).toBeTruthy();
    expect(screen.getByText(NEWS.title).classList.contains("title")).toBe(true);
    expect(screen.getByText(NEWS.title).classList.contains("headline")).toBe(true);
    expect(screen.getAllByText("Politics").length).toBeGreaterThan(0);
    expect(screen.getByText("5m ago").classList.contains("pill")).toBe(true);
    expect(screen.getByText("5m ago").classList.contains("age")).toBe(true);
    expect(container.querySelector(".rail-card.low.news-shade.cat-politics")).not.toBeNull();
    expect(container.querySelector(".tier-code .lucide-newspaper")).not.toBeNull();
    expect(screen.getByText("Wire").classList.contains("stamp")).toBe(true);
    expect(screen.getByText("Summary")).toBeTruthy();
    expect(screen.getByText("Source / Published")).toBeTruthy();

    now.mockRestore();
  });

  it("omits category and age pills when news metadata is null", () => {
    const { container } = render(
      <StatusRailCard
        slot={{ ...NEWS, expanded: false, source: null, category: null, publishedAtMs: null }}
      />,
    );

    expect(screen.getByText("RSS · Wire")).toBeTruthy();
    expect(container.querySelector(".pill.category")).toBeNull();
    expect(container.querySelector(".pill.age")).toBeNull();
    expect(container.querySelector(".rail-card.news-shade.cat-generic")).not.toBeNull();
  });
});
