import { act, cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SlotState } from "../useSlotState";
import type { StatusState } from "../useStatusState";
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
  subtitle: null,
  details: [],
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
  subtitle: null,
  details: [],
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
  subtitle: null,
  details: [],
  queueTotal: 1,
  queueDone: 0,
};

// plan 042: a live-match card — the rust core attaches Clock + per-side
// Cards detail pairs when `espn_live_card` is on.
const LIVE_MATCH: SlotState = {
  state: "showing",
  id: "match-1",
  title: "UCL: ARS 1–1 PSG",
  body: "Yellow Card — B. Saka 54'",
  eventType: "match_state",
  priority: "high",
  signal: "yellow_card",
  expanded: false,
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  subtitle: null,
  details: [
    { label: "Clock", value: "54'" },
    { label: "Cards", value: "ARS 4Y0R · PSG 2Y0R" },
  ],
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
  subtitle: null,
  details: [],
  queueTotal: 2,
  queueDone: 1,
};

// plan 035: a generic (cmux/claude-relay) card carrying a subtitle and
// detail pairs — the manifest renders each as its own cell (Layout A).
const CMUX_RICH: SlotState = {
  state: "showing",
  id: "n5",
  title: "Claude Code needs input",
  body: "A permission prompt is waiting",
  eventType: "generic",
  priority: "high",
  signal: "generic",
  expanded: true,
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  subtitle: "Permission request",
  details: [
    { label: "Tool", value: "Bash" },
    { label: "Command", value: "git push origin master" },
    { label: "Project", value: "/Users/x/proj" },
  ],
  queueTotal: 1,
  queueDone: 0,
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

  // plan 034: the widened idle card (`.rail-card.idle.status`) applies
  // only while the status rail actually has chips — an active status with
  // every gate off and an empty queue keeps the narrow plain-clock card.
  it("widens the idle card only while the status rail is active", () => {
    const active: StatusState = {
      paused: false,
      waiting: 0,
      football: { enabled: false, live: null },
      news: { enabled: true },
      weather: { enabled: false, current: null },
    };
    const inactive: StatusState = {
      paused: false,
      waiting: 0,
      football: { enabled: false, live: null },
      news: { enabled: false },
      weather: { enabled: false, current: null },
    };

    const { container, rerender } = render(
      <StatusRailCard slot={{ state: "empty" }} status={active} />,
    );
    expect(container.querySelector(".rail-card.idle.status")).not.toBeNull();

    rerender(<StatusRailCard slot={{ state: "empty" }} status={inactive} />);
    expect(container.querySelector(".rail-card.idle")).not.toBeNull();
    expect(container.querySelector(".rail-card.idle.status")).toBeNull();

    // no status prop at all (settings preview, older hosts): narrow card
    rerender(<StatusRailCard slot={{ state: "empty" }} />);
    expect(container.querySelector(".rail-card.idle.status")).toBeNull();
  });

  it("never carries the status width class onto a showing card", () => {
    const active: StatusState = {
      paused: false,
      waiting: 1,
      football: { enabled: true, live: { label: "MTL 0–0 TOR", minute: "12'" } },
      news: { enabled: true },
      weather: { enabled: false, current: null },
    };
    const { container } = render(<StatusRailCard slot={GOAL} status={active} />);
    expect(container.querySelector(".rail-card.status")).toBeNull();
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

  it("renders the news masthead, headline, category, age, published time, and category shader classes", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={NEWS} />);

    expect(container.querySelector(".masthead")?.textContent).toContain("NDTV");
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
    expect(screen.getByText("Summary").classList.contains("manifest-label")).toBe(true);
    expect(container.querySelector(".manifest-inner.news")).toBeNull();
    expect(container.querySelector(".manifest-meta")?.textContent).toContain("NDTV");
    expect(container.querySelector(".manifest-meta")?.textContent).toContain("published 08:58");
    expect(container.querySelector(".manifest-meta")?.textContent).toContain("Politics");
    expect(container.querySelector(".manifest-footer")?.textContent).toContain(
      "⌃⇧O read · ⌃⇧N collapse",
    );

    now.mockRestore();
  });

  it("omits category, age, and published-time meta when all news metadata is null", () => {
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

    // plan 078: the manifest stays mounted (aria-hidden) when collapsed, so
    // "RSS" also appears in its meta row — assert on the masthead
    // specifically, and on the collapsed mechanism for the collapse control
    // that used to be absent from the DOM entirely.
    expect(container.querySelector(".masthead")?.textContent).toContain("RSS");
    // hotkey key-cap styling: the hint's "⌃⇧N" is now a <kbd> child, so
    // its text is split across elements — match on the container's
    // normalized textContent rather than an exact getByText.
    expect(container.querySelector(".compact-hint")?.textContent).toBe("⌃⇧N more");
    expect(container.querySelector(".manifest-wrap")?.getAttribute("aria-hidden")).toBe("true");
    expect(container.querySelector(".pill.category")).toBeNull();
    expect(container.querySelector(".pill.age")).toBeNull();
    expect(container.querySelector(".pub-meta")).toBeNull();
    expect(container.querySelector(".rail-card.news-shade.cat-generic")).not.toBeNull();
  });

  it("renders the published-time meta in the compact news card when publishedAtMs is set", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={{ ...NEWS, expanded: false }} />);

    const pubMeta = container.querySelector(".pub-meta");
    expect(pubMeta).not.toBeNull();
    expect(pubMeta?.textContent).toBe("published 08:58");
    // the meta is the last child of the pills row, pushed right by auto margin
    const pills = container.querySelector(".pills");
    expect(pills?.lastElementChild?.classList.contains("pub-meta")).toBe(true);

    now.mockRestore();
  });

  it("renders the expanded news manifest as a full-width summary with an inline meta row", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={{ ...NEWS, link: null }} />);

    expect(container.querySelector(".manifest-inner.news")).toBeNull();
    expect(container.querySelector(".manifest-block")).not.toBeNull();
    expect(screen.getByText("Summary").classList.contains("manifest-label")).toBe(true);
    expect(screen.getByText(NEWS.body).classList.contains("manifest-text")).toBe(true);

    const meta = container.querySelector(".manifest-meta") as HTMLElement;
    expect(meta).not.toBeNull();
    expect(within(meta).getByText("NDTV")?.tagName).toBe("B");
    expect(meta.textContent).toContain("published 08:58");
    expect(meta.textContent).toContain("Politics");
    expect(meta.textContent?.split("·").length).toBeGreaterThanOrEqual(2);

    const footer = container.querySelector(".manifest-footer") as HTMLElement;
    expect(footer).not.toBeNull();
    expect(footer.textContent).toContain("⌃⇧N collapse");
    expect(footer.textContent).not.toContain("⌃⇧O read");

    now.mockRestore();
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

  // plan 035 (Layout A): subtitle and each detail pair render as ordinary
  // manifest cells in the generic branch — exercised through StatusRailCard,
  // the real render path, not a bare <Manifest>.
  it("renders the subtitle and each detail pair as manifest cells when expanded", () => {
    render(<StatusRailCard slot={CMUX_RICH} />);

    expect(screen.getByText("Subtitle")).toBeTruthy();
    expect(screen.getByText("Permission request")).toBeTruthy();
    expect(screen.getByText("Tool")).toBeTruthy();
    expect(screen.getByText("Bash")).toBeTruthy();
    expect(screen.getByText("Command")).toBeTruthy();
    // detail values are plain text (not markdown) — the literal string shows.
    expect(screen.getByText("git push origin master")).toBeTruthy();
    expect(screen.getByText("Project")).toBeTruthy();
    expect(screen.getByText("/Users/x/proj")).toBeTruthy();
  });

  // plan 042 changed the collapsed contract: detail pairs now render below
  // the body (a live-match card's Clock/Cards must be readable without
  // expanding). the subtitle stays expanded-only.
  // plan 078: the manifest stays mounted (aria-hidden, zero-height) when
  // collapsed, so "subtitle hidden" is now asserted via the wrapper's
  // aria-hidden, and the detail-pair assertions are scoped to the compact
  // view (the manifest cells carry the same text).
  it("hides the subtitle but shows detail pairs when collapsed", () => {
    const { container } = render(<StatusRailCard slot={{ ...CMUX_RICH, expanded: false }} />);

    const wrap = container.querySelector(".manifest-wrap") as HTMLElement;
    expect(wrap.getAttribute("aria-hidden")).toBe("true");
    expect(within(wrap).getByText("Subtitle")).toBeTruthy();
    const compact = container.querySelector(".compact") as HTMLElement;
    expect(within(compact).queryByText("Subtitle")).toBeNull();
    expect(within(compact).getByText("Tool")).toBeTruthy();
    expect(within(compact).getByText("Bash")).toBeTruthy();
    expect(within(compact).getByText("Project")).toBeTruthy();
  });

  // plan 078 (Step 6): the collapsed manifest's aria-hidden replaces the
  // DOM removal AnimatePresence used to provide — collapsed content must
  // stay out of the accessibility tree, expanded content must not.
  it("carries aria-hidden on the manifest wrapper only while collapsed", () => {
    const { container, rerender } = render(
      <StatusRailCard slot={{ ...CMUX_RICH, expanded: false }} />,
    );
    expect(container.querySelector(".manifest-wrap")?.getAttribute("aria-hidden")).toBe("true");

    rerender(<StatusRailCard slot={CMUX_RICH} />);
    expect(container.querySelector(".manifest-wrap")?.getAttribute("aria-hidden")).toBe("false");
  });

  // plan 042: the whole point — Clock + per-side Cards readable without
  // expanding. (Scoped to the compact view: plan 078 keeps the collapsed
  // manifest mounted, so its cells carry the same labels.)
  it("renders Clock and per-side Cards in the collapsed view for a live-match card", () => {
    const { container } = render(<StatusRailCard slot={LIVE_MATCH} />);
    const compact = container.querySelector(".compact") as HTMLElement;

    expect(within(compact).getByText("Clock")).toBeTruthy();
    expect(within(compact).getByText("54'")).toBeTruthy();
    expect(within(compact).getByText("Cards")).toBeTruthy();
    expect(within(compact).getByText("ARS 4Y0R · PSG 2Y0R")).toBeTruthy();
  });

  it("renders no detail lines when collapsed with empty details (unchanged behavior)", () => {
    const { container } = render(
      <StatusRailCard slot={{ ...GOAL, expanded: false, details: [] }} />,
    );

    expect(screen.getByText("GOAL")).toBeTruthy();
    // plan 078: body text also renders in the mounted-but-collapsed
    // manifest's Message cell — the "no detail lines" contract is about
    // the compact view, so assert there.
    const compact = container.querySelector(".compact") as HTMLElement;
    expect(within(compact).getByText("Arsenal 2-0")).toBeTruthy();
    expect(compact.querySelector(".detail-label")).toBeNull();
    expect(compact.querySelector(".detail-value")).toBeNull();
  });

  it("renders a detail value that contains an '=' verbatim (first-'=' split is CLI-side only)", () => {
    render(
      <StatusRailCard
        slot={{
          ...CMUX_RICH,
          details: [{ label: "Command", value: "FOO=bar make build" }],
        }}
      />,
    );
    expect(screen.getByText("FOO=bar make build")).toBeTruthy();
  });

  // plan 085: resting_state "notch" — the cheap half of plan 079 item 17.
  // Idle must render zero app-drawn pixels: no idle content, and no
  // `.rail-card` shell at all (not even an empty one), since the shell
  // itself carries background/shadow/priority-accent styling.
  describe("resting_state: notch (plan 085)", () => {
    it("renders nothing while idle", () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} restingState="notch" />,
      );
      expect(container.querySelector(".rail-card")).toBeNull();
      expect(container.querySelector(".rail-card.idle")).toBeNull();
      expect(container.querySelector(".idle-view")).toBeNull();
      expect(container.innerHTML).toBe("");
    });

    it("renders the card normally while showing (promotions unaffected)", () => {
      const { container } = render(<StatusRailCard slot={GOAL} restingState="notch" />);
      expect(container.querySelector(".rail-card.high")).not.toBeNull();
      expect(screen.getByText("GOAL")).toBeTruthy();
    });

    it("hides the card once a showing item finishes rotating out to idle", async () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} restingState="notch" />);
      expect(container.querySelector(".rail-card")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} restingState="notch" />);
      // the outgoing card keeps playing its normal exit animation for the
      // same 220ms window as "rail" mode — it must not vanish abruptly.
      expect(container.querySelector(".rail-card")).not.toBeNull();

      await vi.waitFor(() => {
        expect(container.querySelector(".rail-card")).toBeNull();
      });
    });
  });

  // plan 085: explicit regression pin for the default/unset cases — the
  // idle rail must render byte-identically to before this plan.
  describe("resting_state: rail (default) and unset (plan 085 regression pin)", () => {
    it('renders today\'s idle clock/status rail when restingState is "rail"', () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} restingState="rail" />,
      );
      expect(container.querySelector(".rail-card.idle")).not.toBeNull();
      expect(container.querySelector(".idle-view")).not.toBeNull();
    });

    it("renders today's idle clock/status rail when restingState is omitted", () => {
      const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".rail-card.idle")).not.toBeNull();
      expect(container.querySelector(".idle-view")).not.toBeNull();
    });
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
