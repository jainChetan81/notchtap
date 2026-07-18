import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SlotState } from "../useSlotState";
import { StatusRailCard } from "./StatusRailCard";

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
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  queueTotal: 3,
  queueDone: 0,
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
  link: null,
  queueTotal: 3,
  queueDone: 1,
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
  link: null,
  queueTotal: 1,
  queueDone: 0,
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
  link: "https://example.com/digital-rights",
  queueTotal: 2,
  queueDone: 1,
};

describe("StatusRailCard", () => {
  describe("goal/red-card pulse", () => {
    // The celebration is plan 023's pure-CSS confetti burst + ring on
    // `.rail-card.pulse-goal`'s ::after/::before PLUS plan 032's mounted
    // three-ring ripple (.cele-ripple). Signal-keying coverage: the pulse
    // class lands and the ripple mounts — goal-signal only.
    it("applies pulse-goal (CSS burst + mounted three-ring ripple) on a goal signal", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".rail-card.pulse-goal")).not.toBeNull();
      expect(container.querySelectorAll(".cele-ripple span")).toHaveLength(3);
    });

    // The ripple rides the pulse state, so the same animationend that
    // retires the burst unmounts the rings — no separate cleanup path.
    it("unmounts the ripple when the goal pulse's animation ends", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      const card = container.querySelector(".rail-card") as HTMLElement;
      expect(container.querySelector(".cele-ripple")).not.toBeNull();
      fireAnimationEnd(card, "goal-overshoot");
      expect(container.querySelector(".cele-ripple")).toBeNull();
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

    it("applies pulse-red (and never the goal burst) on a red-card signal", () => {
      const { container } = render(<StatusRailCard slot={RED_CARD} />);
      expect(container.querySelector(".rail-card.pulse-red")).not.toBeNull();
      expect(container.querySelector(".rail-card.pulse-goal")).toBeNull();
      expect(container.querySelector(".cele-ripple")).toBeNull();
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
  it("never plays a goal or red-card pulse for a High-priority generic signal", () => {
    const { container } = render(<StatusRailCard slot={CMUX_NEEDS_INPUT} />);
    expect(container.querySelector(".pulse-goal")).toBeNull();
    expect(container.querySelector(".pulse-red")).toBeNull();
    expect(container.querySelector(".cele-ripple")).toBeNull();
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

  it("renders the news masthead, headline, category, age, and category shader classes", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={NEWS} />);

    expect(screen.getByText("NDTV")).toBeTruthy();
    expect(screen.queryByText("NDTV · Wire")).toBeNull();
    expect(screen.getByText(NEWS.title).classList.contains("title")).toBe(true);
    expect(screen.getByText(NEWS.title).classList.contains("headline")).toBe(true);
    expect(screen.getAllByText("Politics").length).toBeGreaterThan(0);
    expect(screen.getByText("5m ago").classList.contains("pill")).toBe(true);
    expect(screen.getByText("5m ago").classList.contains("age")).toBe(true);
    expect(container.querySelector(".rail-card.low.news-shade.cat-politics")).not.toBeNull();
    // plan 032 deleted the tier chip — priority reads from the CSS-only
    // .compact::before accent edge (a pseudo-element, not assertable in
    // jsdom), so the coverage here is absence: no chip markup, no code text.
    expect(container.querySelector(".tier-code")).toBeNull();
    expect(screen.queryByText("L1")).toBeNull();
    expect(screen.getByText("Wire").classList.contains("stamp")).toBe(true);
    expect(screen.getByText("Summary")).toBeTruthy();
    expect(screen.getByText("Source / Published")).toBeTruthy();
    expect(screen.getByText("Category / Control").nextElementSibling?.textContent).toContain(
      "⌃⇧O read · ⌃⇧N collapse",
    );

    now.mockRestore();
  });

  it("omits category and age pills when news metadata is null", () => {
    const { container } = render(
      <StatusRailCard
        slot={{
          ...NEWS,
          expanded: false,
          source: null,
          category: null,
          publishedAtMs: null,
          link: null,
        }}
      />,
    );

    expect(screen.getByText("RSS")).toBeTruthy();
    expect(screen.getByText("⌃⇧N more").classList.contains("compact-hint")).toBe(true);
    expect(screen.queryByText("⌃⇧N collapse")).toBeNull();
    expect(container.querySelector(".pill.category")).toBeNull();
    expect(container.querySelector(".pill.age")).toBeNull();
    expect(container.querySelector(".rail-card.news-shade.cat-generic")).not.toBeNull();
  });

  it("shows only the collapse control in an expanded manifest without a link", () => {
    render(<StatusRailCard slot={{ ...CMUX_NEEDS_INPUT, link: null }} />);

    const control = screen.getByText("Source / Control").nextElementSibling;
    expect(control?.textContent).toContain("⌃⇧N collapse");
    expect(control?.textContent).not.toContain("⌃⇧O read");
    expect(screen.queryByText("⌃⇧N more")).toBeNull();
  });

  it("shows the read and collapse controls in a non-news manifest with a link", () => {
    render(
      <StatusRailCard
        slot={{
          ...CMUX_NEEDS_INPUT,
          link: "https://example.com/local-notification",
        }}
      />,
    );

    expect(screen.getByText("Source / Control").nextElementSibling?.textContent).toContain(
      "⌃⇧O read · ⌃⇧N collapse",
    );
  });

  // plan 033: the track is now the queue slider — an enqueue while the
  // card is visible re-renders the track (the rust core re-emits the slot
  // state with new queueTotal/queueDone), but the card itself must not
  // re-animate: the motion key is the item id, unchanged by a
  // waiting-count change.
  it("updates the queue slider on a waiting-count change without remounting the card", () => {
    const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
    const trackBefore = container.querySelector(".track");
    expect(trackBefore?.querySelectorAll("span")).toHaveLength(3);
    expect(trackBefore?.querySelectorAll("span.cur")).toHaveLength(1);
    expect(trackBefore?.querySelectorAll("span.done")).toHaveLength(0);

    // same id, deeper queue — the track re-renders in place
    rerender(<StatusRailCard slot={{ ...GOAL, queueTotal: 5, queueDone: 1 }} />);
    const trackAfter = container.querySelector(".track");
    expect(trackAfter).toBe(trackBefore);
    expect(trackAfter?.querySelectorAll("span")).toHaveLength(5);
    expect(trackAfter?.querySelectorAll("span.done")).toHaveLength(1);
    expect(trackAfter?.querySelectorAll("span.cur")).toHaveLength(1);
  });
});
