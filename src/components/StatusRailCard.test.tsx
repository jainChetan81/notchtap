import { afterEach, describe, expect, it, vi } from "vitest";
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

// jsdom has no AnimationEvent constructor at all, and fireEvent.animationEnd
// silently drops non-standard init properties (confirmed: it produces a
// plain Event with animationName === undefined) — so simulate a real
// browser's animationend by patching the property directly onto a plain
// Event before dispatch, which React's synthetic event reads through fine.
function fireAnimationEnd(el: HTMLElement, animationName: string) {
  const event = new Event("animationend", { bubbles: true });
  Object.defineProperty(event, "animationName", { value: animationName });
  act(() => {
    el.dispatchEvent(event);
  });
}

const GOAL: SlotState = {
  state: "showing",
  id: "n1",
  title: "GOAL",
  body: "Arsenal 2-0",
  eventType: "score_update",
  priority: "high",
  signal: "goal",
  expanded: true,
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
};

describe("StatusRailCard", () => {
  describe("goal/red-card pulse", () => {
    it("applies pulse-goal and mounts the celebration on a goal signal", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".rail-card.pulse-goal")).not.toBeNull();
      expect(container.querySelector(".goal-celebration")).not.toBeNull();
    });

    // The pulse clears on the CSS animation's own animationend, not a
    // JS-side timer — there's no duration to keep in sync with styles.css
    // this way. jsdom never runs the animation itself, so tests simulate
    // its natural completion.
    it("clears pulse-goal when its animation ends", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      const card = container.querySelector(".rail-card") as HTMLElement;
      fireAnimationEnd(card, "goal-overshoot");
      expect(container.querySelector(".pulse-goal")).toBeNull();
    });

    it("applies pulse-red (without mounting the goal celebration) on a red-card signal", () => {
      const { container } = render(<StatusRailCard slot={RED_CARD} />);
      expect(container.querySelector(".rail-card.pulse-red")).not.toBeNull();
      expect(container.querySelector(".goal-celebration")).toBeNull();
    });

    it("clears pulse-red when its animation ends", () => {
      const { container } = render(<StatusRailCard slot={RED_CARD} />);
      const card = container.querySelector(".rail-card") as HTMLElement;
      fireAnimationEnd(card, "red-alert");
      expect(container.querySelector(".pulse-red")).toBeNull();
    });

    it("ignores an unrelated animation ending on the same element", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      const card = container.querySelector(".rail-card") as HTMLElement;
      fireAnimationEnd(card, "goal-burst");
      expect(container.querySelector(".pulse-goal")).not.toBeNull();
    });

    it("does not replay the pulse on an unrelated re-render of the same notification", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      const card = container.querySelector(".rail-card") as HTMLElement;
      fireAnimationEnd(card, "goal-overshoot");
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

  // The idle clock re-renders every 30s (useClock) — a live region there
  // would re-announce the time to assistive tech on every tick, which
  // isn't what an arrival-alert live region is for.
  it("is not a live region while idle, and becomes one while showing", () => {
    const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
    const card = container.querySelector(".rail-card") as HTMLElement;
    expect(card.getAttribute("role")).toBeNull();
    expect(card.getAttribute("aria-live")).toBeNull();

    rerender(<StatusRailCard slot={GOAL} />);
    expect(card.getAttribute("role")).toBe("status");
    expect(card.getAttribute("aria-live")).toBe("polite");
  });

  it("renders the priority class and expanded class when showing", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    expect(container.querySelector(".rail-card.high.expanded")).not.toBeNull();
    expect(screen.getByText("GOAL")).toBeTruthy();
    // "Arsenal 2-0" legitimately appears twice while expanded — once in
    // the compact preview, once in the manifest's "Message" detail.
    expect(screen.getAllByText("Arsenal 2-0").length).toBe(2);
  });
});
