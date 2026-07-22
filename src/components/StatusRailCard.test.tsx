import { act, cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { EspnMeta, SlotState, SourceKind } from "../useSlotState";
import type { StatusState } from "../useStatusState";
import { StatusRailCard } from "./StatusRailCard";

// plan 084: `Crest` (StatusRailCard.tsx) calls `convertFileSrc` itself —
// the real tauri implementation isn't available under vitest/jsdom, so
// every test asserting on the crest <img> src needs this mocked. The fake
// prefix makes "the src actually went through convertFileSrc, not the
// raw path" assertable rather than assumed.
vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://converted${path}`,
}));

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
  origin: "football",
  expanded: true,
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  subtitle: null,
  details: [],
  queueTotal: 3,
  queueDone: 0,
  ttlMs: 8000,
  remainingMs: 8000,
};

const RED_CARD: SlotState = {
  state: "showing",
  id: "n2",
  title: "Red Card",
  body: "Chelsea down to 10",
  eventType: "match_state",
  priority: "high",
  signal: "red_card",
  origin: "football",
  expanded: true,
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  subtitle: null,
  details: [],
  queueTotal: 3,
  queueDone: 1,
  ttlMs: 8000,
  remainingMs: 6000,
};

const CMUX_NEEDS_INPUT: SlotState = {
  state: "showing",
  id: "n3",
  title: "Claude Code needs input",
  body: "Workspace command is waiting",
  eventType: "generic",
  priority: "high",
  signal: "generic",
  origin: "cmux",
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
  origin: "football",
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
  ttlMs: 8000,
  remainingMs: 8000,
};

const NEWS: SlotState = {
  state: "showing",
  id: "news-1",
  title: "Parliament passes the landmark digital rights bill",
  body: "The measure passed after a late-night vote.",
  eventType: "news_item",
  priority: "low",
  signal: "generic",
  origin: "news",
  expanded: true,
  source: "NDTV",
  category: "politics",
  publishedAtMs: 2_000_000_000_000 - 5 * 60_000,
  link: "https://example.com/digital-rights",
  subtitle: null,
  details: [],
  queueTotal: 2,
  queueDone: 1,
  ttlMs: 8000,
  remainingMs: 6000,
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
  origin: "cmux",
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
  ttlMs: 8000,
  remainingMs: 8000,
};

// plan 082: a weather ALERT card — the rust core attaches wx-condition/
// wx-is-day marker pairs (plan 035's details channel, reused) so the
// frontend can derive mood/glyph art. Weather art is keyed off those
// markers, not `origin` (plan 096 put `origin` on the wire, but the
// weather mood/glyph derivation predates that and has no reason to move
// off the markers now).
const WEATHER_ALERT: SlotState = {
  state: "showing",
  id: "wx-1",
  title: "Rain expected soon",
  body: "75% chance of rain within ~30 min",
  eventType: "generic",
  priority: "medium",
  signal: "generic",
  origin: "weather",
  expanded: false,
  source: null,
  category: null,
  publishedAtMs: null,
  link: null,
  subtitle: null,
  details: [
    { label: "wx-condition", value: "Rain" },
    { label: "wx-is-day", value: "1" },
  ],
  queueTotal: 1,
  queueDone: 0,
  ttlMs: 8000,
  remainingMs: 8000,
};

// plan 084: the structured espn meta (POST-083 contract) — a base fixture
// plus a small helper to build a showing slot around it, one event/state
// at a time, so each scorecard test only overrides what it's about.
const ESPN_BASE: EspnMeta = {
  league: "UCL",
  homeAbbrev: "ARS",
  awayAbbrev: "PSG",
  homeScore: 1,
  awayScore: 1,
  clock: "78'",
  homeCards: [0, 0],
  awayCards: [0, 0],
  homeCrest: null,
  awayCrest: null,
};

function liveSlot(overrides: Partial<Extract<SlotState, { state: "showing" }>> = {}): SlotState {
  return {
    state: "showing",
    id: "match-live-1",
    title: "UCL: ARS 1–1 PSG",
    body: "Goal — K. Havertz 78'",
    eventType: "score_update",
    priority: "high",
    signal: "goal",
    origin: "football",
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
    espn: ESPN_BASE,
    ...overrides,
  };
}

describe("StatusRailCard", () => {
  describe("goal/red-card pulse", () => {
    // The celebration is plan 023's pure-CSS confetti burst + ring on
    // `.card-assembly.pulse-goal`'s ::after/::before PLUS plan 032's mounted
    // three-ring ripple (.cele-ripple). Signal-keying coverage: the pulse
    // class lands and the ripple mounts — goal-signal only.
    it("applies pulse-goal (CSS burst + mounted three-ring ripple) on a goal signal", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.pulse-goal")).not.toBeNull();
      expect(container.querySelectorAll(".cele-ripple span")).toHaveLength(3);
    });

    // The ripple rides the pulse state, so the same animationend that
    // retires the burst unmounts the rings — no separate cleanup path.
    it("unmounts the ripple when the goal pulse's animation ends", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      const card = container.querySelector(".card-assembly") as HTMLElement;
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
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "goal-overshoot");
      expect(container.querySelector(".pulse-goal")).toBeNull();
    });

    it("applies pulse-red (and never the goal burst) on a red-card signal", () => {
      const { container } = render(<StatusRailCard slot={RED_CARD} />);
      expect(container.querySelector(".card-assembly.pulse-red")).not.toBeNull();
      expect(container.querySelector(".card-assembly.pulse-goal")).toBeNull();
      expect(container.querySelector(".cele-ripple")).toBeNull();
    });

    it("clears pulse-red when its animation ends", () => {
      const { container } = render(<StatusRailCard slot={RED_CARD} />);
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "red-alert");
      expect(container.querySelector(".pulse-red")).toBeNull();
    });

    it("ignores an unrelated animation ending on the same element", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "goal-burst");
      expect(container.querySelector(".pulse-goal")).not.toBeNull();
    });

    it("does not replay the pulse on an unrelated re-render of the same notification", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      const card = container.querySelector(".card-assembly") as HTMLElement;
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
    expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    expect(container.querySelector(".time-only")).not.toBeNull();
    expect(container.querySelector(".status-dots")).not.toBeNull();
    expect(screen.queryByText("GOAL")).toBeNull();
  });

  // plan 091: plan 034's idle/idle-status width split (`.rail-card.idle`
  // vs `.rail-card.idle.status`) deliberately collapses — the new idle
  // has ONE width formula regardless of status-rail activity, because the
  // status dots replace the old chip rail entirely (no chips to widen
  // for). This is a regression PIN for that collapse, not a rewrite of
  // the old widening behavior: no "status" modifier class exists at all
  // anymore, active status or not, status prop present or not.
  it("never applies a width-modifier class for status-rail activity (plan 034's split is gone)", () => {
    const active: StatusState = {
      paused: false,
      waiting: 1,
      football: { enabled: true, live: { label: "MTL 0–0 TOR", minute: "12'" } },
      news: { enabled: true },
      weather: { enabled: false, current: null },
      media: { enabled: false, current: null },
    };
    const inactive: StatusState = {
      paused: false,
      waiting: 0,
      football: { enabled: false, live: null },
      news: { enabled: false },
      weather: { enabled: false, current: null },
      media: { enabled: false, current: null },
    };

    const { container, rerender } = render(
      <StatusRailCard slot={{ state: "empty" }} status={active} />,
    );
    expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    expect(container.querySelector(".card-assembly.status")).toBeNull();

    rerender(<StatusRailCard slot={{ state: "empty" }} status={inactive} />);
    expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    expect(container.querySelector(".card-assembly.status")).toBeNull();

    // no status prop at all (settings preview, older hosts): same, no
    // status-modifier class either way.
    rerender(<StatusRailCard slot={{ state: "empty" }} />);
    expect(container.querySelector(".card-assembly.status")).toBeNull();

    // and a showing card never carried it either, before or after.
    rerender(<StatusRailCard slot={GOAL} status={active} />);
    expect(container.querySelector(".card-assembly.status")).toBeNull();
  });

  // The idle clock re-renders every 30s (useClock) — a live region there
  // would re-announce the time to assistive tech on every tick, which
  // isn't what an arrival-alert live region is for.
  it("is not a live region while idle, and becomes one while showing", () => {
    const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
    const card = container.querySelector(".card-assembly") as HTMLElement;
    expect(card.getAttribute("role")).toBeNull();
    expect(card.getAttribute("aria-live")).toBeNull();

    rerender(<StatusRailCard slot={GOAL} />);
    expect(card.getAttribute("role")).toBe("status");
    expect(card.getAttribute("aria-live")).toBe("polite");
  });

  it("renders the priority class and expanded class when showing", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    expect(container.querySelector(".card-assembly.high.expanded")).not.toBeNull();
    expect(screen.getByText("GOAL")).toBeTruthy();
    // "Arsenal 2-0" legitimately appears twice while expanded — once in
    // the compact preview, once in the manifest's "Message" detail.
    expect(screen.getAllByText("Arsenal 2-0").length).toBe(2);
  });

  // plan 100: the TTL bar exists only in the "showing" state (no card, no
  // bar) and is the LAST child of the card-content block, after both the
  // compact content and the manifest wrap — it's the card's bottom edge
  // (absolutely positioned, styles.css), not static-flow content between
  // sections (that was plan 081's position; plan 100 moved it to fix the
  // double-border artifact against the manifest wrap's own border-top).
  it("renders no ttl-bar while idle", () => {
    const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
    expect(container.querySelector(".ttl-bar")).toBeNull();
  });

  it("renders the ttl-bar last, after the compact content and the manifest wrap, when showing", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    const cardContent = container.querySelector(".card-content") as HTMLElement;
    const children = Array.from(cardContent.children);
    const compactIndex = children.findIndex((el) => el.classList.contains("compact"));
    const manifestIndex = children.findIndex((el) => el.classList.contains("manifest-wrap"));
    const ttlBarIndex = children.findIndex((el) => el.classList.contains("ttl-bar"));
    expect(compactIndex).toBeGreaterThanOrEqual(0);
    expect(manifestIndex).toBeGreaterThan(compactIndex);
    expect(ttlBarIndex).toBeGreaterThan(manifestIndex);
    expect(ttlBarIndex).toBe(children.length - 1);
  });

  it("renders the news masthead, headline, category, age, published time, and category shader classes", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={NEWS} />);

    expect(container.querySelector(".masthead")?.textContent).toContain("NDTV");
    expect(screen.queryByText("NDTV · Wire")).toBeNull();
    expect(screen.getByText(NEWS.title).classList.contains("title")).toBe(true);
    expect(screen.getByText(NEWS.title).classList.contains("headline")).toBe(true);
    expect(screen.getAllByText("Politics").length).toBeGreaterThan(0);
    // plan 092 (item 10/Decision 5): the category pill chip-converged to
    // `.chip.chip-category`; the age pill moved OUT of the chip row
    // entirely into the plain `.notif-time-inline` slot (same ageLabel
    // computation/thresholds, new location — no longer chip-shaped).
    // "Politics" legitimately appears twice while expanded (compact chip +
    // manifest meta segment), so scope to the compact chip specifically.
    const categoryChip = container.querySelector(".chip-category") as HTMLElement;
    expect(categoryChip.textContent).toBe("Politics");
    expect(categoryChip.classList.contains("chip")).toBe(true);
    expect(screen.getByText("5m ago").classList.contains("notif-time-inline")).toBe(true);
    expect(screen.getByText("5m ago").classList.contains("chip")).toBe(false);
    // plan 091: priority accent (`low`) stays on the outer shell;
    // news-shade/category mood classes moved to `.below-block` — required
    // for the mood gradient to paint at all (a `.card-assembly::before`
    // pseudo-element there would paint BEHIND below-block's own opaque
    // fill in stacking order, making the gradient invisible), and matches
    // the locked prototype's own placement (`wx-block`/mood classes sit
    // on `.below-block`, never on `.card-wrap`).
    expect(container.querySelector(".card-assembly.low")).not.toBeNull();
    expect(container.querySelector(".below-block.news-shade.cat-politics")).not.toBeNull();
    // plan 032 deleted the tier chip — priority reads from the CSS-only
    // .compact::before accent edge (a pseudo-element, not assertable in
    // jsdom), so the coverage here is absence: no chip markup, no code text.
    expect(container.querySelector(".tier-code")).toBeNull();
    expect(screen.queryByText("L1")).toBeNull();
    expect(screen.getByText("Wire").classList.contains("stamp")).toBe(true);
    expect(screen.getByText("Summary").classList.contains("manifest-label")).toBe(true);
    expect(container.querySelector(".manifest-inner.news")).toBeNull();
    expect(container.querySelector(".manifest-meta")?.textContent).toContain("NDTV");
    expect(container.querySelector(".manifest-meta")?.textContent).toContain("published 03:28");
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
    expect(container.querySelector(".chip-category")).toBeNull();
    expect(container.querySelector(".notif-time-inline")).toBeNull();
    expect(container.querySelector(".pub-meta")).toBeNull();
    // plan 091: news-shade/category classes moved to `.below-block` (see
    // the comment on the same assertion above for why).
    expect(container.querySelector(".below-block.news-shade.cat-generic")).not.toBeNull();
  });

  // plan 110 (Step C): flips the OLD contract this test used to pin (a
  // `.pub-meta` "published HH:MM" node duplicating the relative age in
  // the same compact row). The compact row now shows exactly one time
  // expression — the relative age — and `.pub-meta` is gone entirely
  // (orphaned CSS removed from both styles.css and preview-overlay.css).
  // The expanded Manifest's own "published HH:MM" segment is untouched —
  // see the dedicated pin below.
  it("renders exactly one time expression (relative age) in the compact news card's meta row — no duplicate published time", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={{ ...NEWS, expanded: false }} />);

    const metaRow = container.querySelector(".notif-meta-row") as HTMLElement;
    expect(metaRow).not.toBeNull();
    const ageNode = metaRow.querySelector(".notif-time-inline");
    expect(ageNode).not.toBeNull();
    expect(ageNode?.textContent).toBe("5m ago");
    expect(container.querySelector(".pub-meta")).toBeNull();
    // the age node is the only time expression in the row — it's also
    // the row's last child now that .pub-meta is gone.
    expect(metaRow.lastElementChild).toBe(ageNode);

    now.mockRestore();
  });

  // plan 110 (Step C): the pin this plan's own comment promises —
  // Manifest's expanded "published HH:MM" segment must survive the
  // compact-row deletion above unchanged, so a future edit can't
  // "simplify" the compact fix into deleting the expanded rendering too.
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
    expect(meta.textContent).toContain("published 03:28");
    expect(meta.textContent).toContain("Politics");
    expect(meta.textContent?.split("·").length).toBeGreaterThanOrEqual(2);

    const footer = container.querySelector(".manifest-footer") as HTMLElement;
    expect(footer).not.toBeNull();
    expect(footer.textContent).toContain("⌃⇧N collapse");
    expect(footer.textContent).not.toContain("⌃⇧O read");

    now.mockRestore();
  });

  // plan 092 (Step 3): the generic manifest converged onto news's
  // full-width `.manifest-block` vocabulary — the old "Source / Control"
  // detail-label heading is gone; the source label now sits in
  // `.manifest-meta` (unlabeled, like news's) and the hotkey hint moved
  // to `.manifest-footer` (also like news's), so these two assertions now
  // read directly off `.manifest-footer`, mirroring the news manifest
  // test above exactly.
  it("shows only the collapse control in an expanded manifest without a link", () => {
    const { container } = render(<StatusRailCard slot={{ ...CMUX_NEEDS_INPUT, link: null }} />);

    const footer = container.querySelector(".manifest-footer") as HTMLElement;
    expect(footer.textContent).toContain("⌃⇧N collapse");
    expect(footer.textContent).not.toContain("⌃⇧O read");
    expect(screen.queryByText("⌃⇧N more")).toBeNull();
  });

  it("shows the read and collapse controls in a non-news manifest with a link", () => {
    const { container } = render(
      <StatusRailCard
        slot={{
          ...CMUX_NEEDS_INPUT,
          link: "https://example.com/local-notification",
        }}
      />,
    );

    const footer = container.querySelector(".manifest-footer") as HTMLElement;
    expect(footer.textContent).toContain("⌃⇧O read · ⌃⇧N collapse");
  });

  // plan 035 (Layout A): subtitle and each detail pair render as ordinary
  // manifest cells in the generic branch — exercised through StatusRailCard,
  // the real render path, not a bare <Manifest>.
  // plan 092: subtitle now ALSO renders in the compact header's
  // `.notif-subtitle-row` (Decision 1 — surfaced there for the first
  // time), so "Permission request" legitimately appears twice while
  // expanded — once in the header, once in the manifest's Subtitle cell.
  // Scope the manifest-cell assertions to the manifest specifically, like
  // the goal fixture's "Arsenal 2-0"-appears-twice test above does.
  it("renders the subtitle and each detail pair as manifest cells when expanded", () => {
    const { container } = render(<StatusRailCard slot={CMUX_RICH} />);
    const manifest = container.querySelector(".manifest") as HTMLElement;

    expect(within(manifest).getByText("Subtitle")).toBeTruthy();
    expect(within(manifest).getByText("Permission request")).toBeTruthy();
    expect(within(manifest).getByText("Tool")).toBeTruthy();
    expect(within(manifest).getByText("Bash")).toBeTruthy();
    expect(within(manifest).getByText("Command")).toBeTruthy();
    // detail values are plain text (not markdown) — the literal string shows.
    expect(within(manifest).getByText("git push origin master")).toBeTruthy();
    expect(within(manifest).getByText("Project")).toBeTruthy();
    expect(within(manifest).getByText("/Users/x/proj")).toBeTruthy();
    // and the compact header does carry the subtitle too (new in this plan).
    const compact = container.querySelector(".compact") as HTMLElement;
    expect(within(compact).getByText("Permission request")).toBeTruthy();
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

  // plan 096 (079 item 8, the wire addition + accent build): the cmux
  // accent — a chip (tint + glyph) in the header badge cluster, plus a
  // hairline on `.below-block`, gated strictly on the now-wire `origin`
  // field. Byte-absent for the other four SourceKind values; must never
  // touch the priority accent channel (origin and priority are orthogonal).
  describe("cmux accent (plan 096)", () => {
    // cast to the narrower "showing" branch before spreading: spreading a
    // `SlotState`-typed (union) variable and overriding one field produces
    // a spurious excess-property error against the `{ state: "empty" }`
    // branch — an existing TS quirk with union spreads, unrelated to this
    // plan's own logic.
    function genericSlot(origin: SourceKind): SlotState {
      return { ...(CMUX_NEEDS_INPUT as Extract<SlotState, { state: "showing" }>), origin };
    }

    it("renders the chip and the below-block hairline for origin: cmux", () => {
      const { container } = render(<StatusRailCard slot={genericSlot("cmux")} />);
      const chip = container.querySelector(".chip-cmux");
      expect(chip).not.toBeNull();
      expect(chip?.textContent).toBe("Agent");
      // lives inside the header badge cluster, alongside the Stamp — not
      // some separate unrelated location.
      expect(container.querySelector(".notif-header-badges .chip-cmux")).not.toBeNull();
      expect(container.querySelector(".below-block.cmux-origin")).not.toBeNull();
    });

    it("is byte-absent for every other origin", () => {
      for (const origin of ["football", "news", "manual", "weather"] as const) {
        const { container, unmount } = render(<StatusRailCard slot={genericSlot(origin)} />);
        expect(container.querySelector(".chip-cmux")).toBeNull();
        expect(container.querySelector(".below-block.cmux-origin")).toBeNull();
        expect(container.innerHTML).not.toContain("chip-cmux");
        expect(container.innerHTML).not.toContain("cmux-origin");
        unmount();
      }
    });

    // the sharpest STOP condition in the plan: the accent must never touch
    // the priority accent channel. Same priority (high), different origin
    // — the shell's class list (which is exactly where the priority accent
    // classes live, `.card-assembly.low/medium/high`) must be identical.
    it("never touches the priority accent channel — same priority, different origin, identical shell classes", () => {
      const { container: cmuxContainer } = render(<StatusRailCard slot={genericSlot("cmux")} />);
      const { container: manualContainer } = render(
        <StatusRailCard slot={genericSlot("manual")} />,
      );
      const cmuxShell = cmuxContainer.querySelector(".card-assembly") as HTMLElement;
      const manualShell = manualContainer.querySelector(".card-assembly") as HTMLElement;
      expect(cmuxShell.className).toBe(manualShell.className);
      expect(cmuxShell.className).not.toContain("cmux");
    });
  });

  // plan 085 (original) / plan 105 Step C (fix): resting_state "notch" —
  // the cheap half of plan 079 item 17. Idle must render zero app-drawn
  // pixels — but (105) the shell itself still mounts, bare, so hovering
  // can reveal the peek; only the painted chrome (clock, dots,
  // below-block) is gone. This replaces 085's original "no shell at all"
  // test — that promise was a dead end (nothing painted AND nothing
  // hoverable) — with the narrower "no painted chrome" contract 105
  // authorizes. See that plan's Maintenance notes.
  describe("resting_state: notch (plan 085; bare-hover fix: plan 105)", () => {
    it("renders bare — no below-block, no clock text, no status dots — while idle and not hovered", () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} restingState="notch" />,
      );
      const shell = container.querySelector(".card-assembly");
      expect(shell).not.toBeNull();
      expect(shell?.classList.contains("bare")).toBe(true);
      expect(container.querySelector(".below-block")).toBeNull();
      expect(container.querySelector(".time-only")).toBeNull();
      expect(container.querySelector(".status-dots")).toBeNull();
    });

    // plan 105's actual bug fix: hovering bare mode must reveal the peek —
    // the whole point is that the mode is no longer a dead end.
    it("reveals the idle-peek below-block when hovered, even though nothing painted while not hovered", () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} restingState="notch" hovered={true} />,
      );
      expect(container.querySelector(".card-assembly.bare")).not.toBeNull();
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
    });

    it("renders the card normally while showing (promotions unaffected)", () => {
      const { container } = render(<StatusRailCard slot={GOAL} restingState="notch" />);
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();
      expect(container.querySelector(".card-assembly.bare")).toBeNull();
      expect(screen.getByText("GOAL")).toBeTruthy();
    });

    // Step C's DOM-identity requirement: a showing card in notch mode must
    // render byte-identical content to the same card in rail mode — the
    // bare modifier only ever applies to the settled-idle state.
    it("renders identical DOM to rail mode while showing", () => {
      const { container: notchContainer } = render(
        <StatusRailCard slot={GOAL} restingState="notch" />,
      );
      const { container: railContainer } = render(
        <StatusRailCard slot={GOAL} restingState="rail" />,
      );
      expect(notchContainer.innerHTML).toBe(railContainer.innerHTML);
    });

    it("keeps the exit animation, then settles bare (not absent) once a showing item finishes rotating out to idle", async () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} restingState="notch" />);
      expect(container.querySelector(".card-assembly")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} restingState="notch" />);
      // the outgoing card keeps playing its normal exit animation for the
      // same 220ms window as "rail" mode — it must not vanish abruptly.
      expect(container.querySelector(".below-block")).not.toBeNull();

      await vi.waitFor(() => {
        const shell = container.querySelector(".card-assembly");
        expect(shell?.classList.contains("bare")).toBe(true);
        expect(container.querySelector(".below-block")).toBeNull();
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
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      expect(container.querySelector(".time-only")).not.toBeNull();
      expect(container.querySelector(".status-dots")).not.toBeNull();
    });

    it("renders today's idle clock/status rail when restingState is omitted", () => {
      const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      expect(container.querySelector(".time-only")).not.toBeNull();
      expect(container.querySelector(".status-dots")).not.toBeNull();
    });
  });

  // plan 082: weather ALERT cards get their mood+glyph art from the
  // wx-condition/wx-is-day marker pairs, and those markers must never
  // leak into the visible detail cells (collapsed or expanded) — the
  // marker-leak guard.
  describe("weather ALERT card art (plan 082)", () => {
    // plan 091: wx-card/mood/texture classes moved from the outer shell
    // to `.below-block` — required for the mood gradient to actually
    // paint (see the comment on the news-shade assertions above for the
    // stacking-order reason) and matching the locked prototype's own
    // placement. `classesOf` now reads `.below-block`'s className, not
    // the outer `.card-assembly`'s.
    function classesOf(container: HTMLElement): string[] {
      return (container.querySelector(".below-block")?.className ?? "").split(" ").filter(Boolean);
    }

    it("applies the mood class and renders the condition glyph for a weather alert", () => {
      const { container } = render(<StatusRailCard slot={WEATHER_ALERT} />);
      const classes = classesOf(container);
      expect(classes).toContain("wx-card");
      expect(classes).toContain("wx-rain"); // Rain + day (wx-is-day: "1")
      expect(classes).toContain("wx-rain-streaks");
      const icon = container.querySelector("img.wx-icon");
      expect(icon).not.toBeNull();
      expect(icon?.getAttribute("src")).toMatch(/rain.*\.svg/);
    });

    it("keys night (wx-is-day: 0) to the rainy-night mood, not the day mood", () => {
      const { container } = render(
        <StatusRailCard
          slot={{
            ...WEATHER_ALERT,
            details: [
              { label: "wx-condition", value: "Rain" },
              { label: "wx-is-day", value: "0" },
            ],
          }}
        />,
      );
      const classes = classesOf(container);
      expect(classes).toContain("wx-rainy-night");
      expect(classes).not.toContain("wx-rain");
    });

    it("never renders wx-condition/wx-is-day as visible detail cells while collapsed", () => {
      const { container } = render(<StatusRailCard slot={WEATHER_ALERT} />);
      const compact = container.querySelector(".compact") as HTMLElement;
      expect(within(compact).queryByText("wx-condition")).toBeNull();
      expect(within(compact).queryByText("wx-is-day")).toBeNull();
      expect(within(compact).queryByText("Rain")).toBeNull();
      expect(compact.querySelector(".detail-label")).toBeNull();
      expect(compact.querySelector(".detail-value")).toBeNull();
    });

    it("never renders wx-condition/wx-is-day as visible detail cells while expanded (Manifest)", () => {
      const { container } = render(<StatusRailCard slot={{ ...WEATHER_ALERT, expanded: true }} />);
      const manifest = container.querySelector(".manifest") as HTMLElement;
      expect(within(manifest).queryByText("wx-condition")).toBeNull();
      expect(within(manifest).queryByText("wx-is-day")).toBeNull();
      // the manifest's Message cell legitimately contains the alert body
      // text, so assert there's no *label* cell for either marker rather
      // than banning the word "Rain" outright.
      const labels = Array.from(manifest.querySelectorAll(".detail-label")).map(
        (el) => el.textContent,
      );
      expect(labels).not.toContain("wx-condition");
      expect(labels).not.toContain("wx-is-day");
    });

    it("falls back to the neutral overcast mood for an unrecognized condition word", () => {
      const { container } = render(
        <StatusRailCard
          slot={{
            ...WEATHER_ALERT,
            details: [
              { label: "wx-condition", value: "—" },
              { label: "wx-is-day", value: "1" },
            ],
          }}
        />,
      );
      // plan 091: mood classes live on `.below-block` now.
      const block = container.querySelector(".below-block");
      expect(block?.className).toContain("wx-overcast");
    });

    // Regression pin: a generic card WITHOUT wx markers (the existing
    // CMUX_RICH fixture) must render exactly as it did before this plan —
    // no wx-card class, no glyph image, and its own (non-wx) detail pairs
    // still render normally.
    it("renders a non-weather generic card byte-identically (no wx classes, no glyph, own details intact)", () => {
      const { container } = render(<StatusRailCard slot={CMUX_RICH} />);
      const block = container.querySelector(".below-block");
      expect(block?.className).not.toContain("wx-card");
      expect(container.querySelector("img.wx-icon")).toBeNull();
      const manifest = container.querySelector(".manifest") as HTMLElement;
      expect(within(manifest).getByText("Tool")).toBeTruthy();
      expect(within(manifest).getByText("Bash")).toBeTruthy();
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

  // plan 084: the recurring live-match scorecard — detected by the
  // structured `espn` block's presence (POST-083 contract), rendered
  // through a wholly different branch than the generic/news layouts
  // above (no Track, no TtlBar, no Manifest, no compact-hint).
  describe("live-match football scorecard (plan 084)", () => {
    it("renders the league chip, live-pill, clock, crests-as-abbrev, and score", () => {
      const { container } = render(<StatusRailCard slot={liveSlot()} />);
      expect(container.querySelector(".notif-block")).not.toBeNull();
      expect(screen.getByText("UCL")).toBeTruthy();
      const pill = container.querySelector(".chip-live");
      expect(pill?.textContent).toBe("Live");
      expect(pill?.classList.contains("break")).toBe(false);
      expect(pill?.classList.contains("final")).toBe(false);
      expect(pill?.querySelector(".live-dot")).not.toBeNull();
      expect(container.querySelector(".clock-pill")?.textContent).toBe("78'");
      const crests = container.querySelectorAll(".crest");
      expect(crests).toHaveLength(2);
      expect(crests[0].textContent).toBe("ARS");
      expect(crests[1].textContent).toBe("PSG");
      expect(container.querySelector(".score")?.textContent).toBe("1–1");
    });

    it("goal: tints the event line green, plays cele-goal (not pulse-goal), never the ripple", () => {
      const { container } = render(
        <StatusRailCard slot={liveSlot({ signal: "goal", body: "Goal — K. Havertz 78'" })} />,
      );
      const eventLine = container.querySelector(".event-line");
      expect(eventLine?.classList.contains("tint-goal")).toBe(true);
      expect(eventLine?.querySelector(".ev-ico.goal")).not.toBeNull();
      expect(eventLine?.textContent).toContain("Goal — K. Havertz 78'");
      expect(container.querySelector(".card-assembly.cele-goal")).not.toBeNull();
      expect(container.querySelector(".card-assembly.pulse-goal")).toBeNull();
      expect(container.querySelector(".cele-ripple")).toBeNull();
    });

    it("penalty scored: same cele-goal celebration family, ring icon", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({ signal: "goal", body: "Penalty - Scored — Mohamed Salah 44'" })}
        />,
      );
      const eventLine = container.querySelector(".event-line");
      expect(eventLine?.classList.contains("tint-goal")).toBe(true);
      expect(eventLine?.querySelector(".ev-ico.pen")).not.toBeNull();
      expect(container.querySelector(".card-assembly.cele-goal")).not.toBeNull();
    });

    it("own goal: score updates, hollow icon, NO tint and NO celebration", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({
            signal: "goal",
            body: "Own Goal — W. Saliba 12'",
            espn: { ...ESPN_BASE, homeScore: 0, awayScore: 1 },
          })}
        />,
      );
      expect(container.querySelector(".score")?.textContent).toBe("0–1");
      const eventLine = container.querySelector(".event-line");
      expect(eventLine?.className).toBe("event-line");
      expect(eventLine?.querySelector(".ev-ico.og")).not.toBeNull();
      expect(container.querySelector(".card-assembly.cele-goal")).toBeNull();
      expect(container.querySelector(".card-assembly.cele-yc")).toBeNull();
      expect(container.querySelector(".card-assembly.cele-rc")).toBeNull();
    });

    it("yellow card: amber tint, cele-yc, and the per-side cards line ticks up", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({
            signal: "yellow_card",
            body: "Yellow Card — B. Saka 54'",
            espn: { ...ESPN_BASE, homeCards: [1, 0], awayCards: [2, 0] },
          })}
        />,
      );
      const eventLine = container.querySelector(".event-line");
      expect(eventLine?.classList.contains("tint-yc")).toBe(true);
      expect(eventLine?.querySelector(".ev-ico.yc")).not.toBeNull();
      expect(container.querySelector(".card-assembly.cele-yc")).not.toBeNull();
      expect(container.querySelector(".cards-line")?.textContent).toBe("ARS 1Y0R · PSG 2Y0R");
    });

    it("red card: coral tint and cele-rc", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({
            signal: "red_card",
            body: "Red Card — M. Dembélé 71'",
            espn: { ...ESPN_BASE, homeCards: [1, 0], awayCards: [2, 1] },
          })}
        />,
      );
      const eventLine = container.querySelector(".event-line");
      expect(eventLine?.classList.contains("tint-rc")).toBe(true);
      expect(eventLine?.querySelector(".ev-ico.rc")).not.toBeNull();
      expect(container.querySelector(".card-assembly.cele-rc")).not.toBeNull();
      expect(container.querySelector(".card-assembly.pulse-red")).toBeNull();
    });

    it.each([
      ["foul", "foul", "Foul — D. Rice 62'"],
      ["offside", "off", "Offside — K. Mbappé 55'"],
      ["var_check", "var", "VAR check — possible penalty 67'"],
      ["substitution", "sub", "Substitution — L. Trossard for G. Martinelli 70'"],
    ] as const)(
      "%s: quiet event line — correct icon, no tint, no celebration",
      (signal, iconSuffix, body) => {
        const { container } = render(<StatusRailCard slot={liveSlot({ signal, body })} />);
        const eventLine = container.querySelector(".event-line");
        expect(eventLine?.className).toBe("event-line");
        expect(eventLine?.querySelector(`.ev-ico.${iconSuffix}`)).not.toBeNull();
        expect(eventLine?.textContent).toContain(body);
        expect(container.querySelector(".card-assembly.cele-goal")).toBeNull();
        expect(container.querySelector(".card-assembly.cele-yc")).toBeNull();
        expect(container.querySelector(".card-assembly.cele-rc")).toBeNull();
      },
    );

    it("clears cele-goal on its ring animation ending, and never both pulse and cele stack", () => {
      const { container } = render(<StatusRailCard slot={liveSlot({ signal: "goal" })} />);
      const card = container.querySelector(".card-assembly") as HTMLElement;
      expect(container.querySelector(".cele-goal")).not.toBeNull();
      fireAnimationEnd(card, "cele-ring");
      expect(container.querySelector(".cele-goal")).toBeNull();
    });

    it("clears cele-rc on the red-strobe animation ending", () => {
      const { container } = render(<StatusRailCard slot={liveSlot({ signal: "red_card" })} />);
      const card = container.querySelector(".card-assembly") as HTMLElement;
      expect(container.querySelector(".cele-rc")).not.toBeNull();
      fireAnimationEnd(card, "red-strobe");
      expect(container.querySelector(".cele-rc")).toBeNull();
    });

    it("half-time: Break pill and the HT clock, from the wire's own signal/clock", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({
            signal: "halftime",
            body: "half-time",
            espn: { ...ESPN_BASE, clock: "HT" },
          })}
        />,
      );
      const pill = container.querySelector(".chip-live");
      expect(pill?.textContent).toBe("Break");
      expect(pill?.classList.contains("break")).toBe(true);
      expect(pill?.querySelector(".live-dot")).not.toBeNull();
      expect(container.querySelector(".clock-pill")?.textContent).toBe("HT");
    });

    it("full-time: Final pill (no live-dot) and the FT clock", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({
            signal: "fulltime",
            body: "full-time",
            espn: { ...ESPN_BASE, clock: "FT" },
          })}
        />,
      );
      const pill = container.querySelector(".chip-live");
      expect(pill?.textContent).toBe("Final");
      expect(pill?.classList.contains("final")).toBe(true);
      expect(pill?.querySelector(".live-dot")).toBeNull();
      expect(container.querySelector(".clock-pill")?.textContent).toBe("FT");
    });

    // No "Soon" (pre-match) variant: there is no wire signal for it (see
    // StatusRailCard.tsx's live-match branch doc) — every wire EventSignal
    // maps to Live/Break/Final only. This is coverage-by-exhaustion, not a
    // single assertion: `livePillVariantFor` (lib/presentation.ts) is
    // exhaustive over the full EventSignal union with no "soon" arm, so a
    // wire signal can never resolve to a fourth variant this component
    // would need to render.

    it("omits the cards line on a clean match (no cards either side)", () => {
      const { container } = render(<StatusRailCard slot={liveSlot()} />);
      expect(container.querySelector(".cards-line")).toBeNull();
    });

    it("renders no Track (queue slider) on the live card, while a generic card in the same run still gets one", () => {
      const { container: liveContainer } = render(<StatusRailCard slot={liveSlot()} />);
      expect(liveContainer.querySelector(".track")).toBeNull();
      cleanup();

      const { container: genericContainer } = render(<StatusRailCard slot={GOAL} />);
      expect(genericContainer.querySelector(".track")).not.toBeNull();
    });

    it("renders no TtlBar and no Manifest on the live card", () => {
      const { container } = render(<StatusRailCard slot={liveSlot()} />);
      expect(container.querySelector(".ttl-bar")).toBeNull();
      expect(container.querySelector(".manifest")).toBeNull();
      expect(container.querySelector(".manifest-wrap")).toBeNull();
      expect(container.querySelector(".compact-hint")).toBeNull();
    });

    it("still renders the compact scorecard (not a bigger layout) when expanded arrives true", () => {
      const { container } = render(<StatusRailCard slot={liveSlot({ expanded: true })} />);
      expect(container.querySelector(".notif-block")).not.toBeNull();
      expect(container.querySelector(".manifest")).toBeNull();
    });

    describe("crest rendering", () => {
      it("renders an <img> with a convertFileSrc-converted src when a crest path is present", () => {
        const { container } = render(
          <StatusRailCard
            slot={liveSlot({
              espn: {
                ...ESPN_BASE,
                homeCrest: "/Users/x/.config/notchtap/crests/96.png",
              },
            })}
          />,
        );
        const img = container.querySelector(".crest img") as HTMLImageElement;
        expect(img).not.toBeNull();
        expect(img.getAttribute("src")).toBe(
          "asset://converted/Users/x/.config/notchtap/crests/96.png",
        );
      });

      it("falls back to the text-abbrev circle when the crest path is absent", () => {
        const { container } = render(<StatusRailCard slot={liveSlot({ espn: ESPN_BASE })} />);
        expect(container.querySelector(".crest img")).toBeNull();
        const crests = container.querySelectorAll(".crest");
        expect(crests[0].textContent).toBe("ARS");
        expect(crests[1].textContent).toBe("PSG");
      });

      it("falls back to the text-abbrev circle when the <img> itself errors (defense in depth)", () => {
        const { container } = render(
          <StatusRailCard
            slot={liveSlot({
              espn: { ...ESPN_BASE, homeCrest: "/Users/x/.config/notchtap/crests/96.png" },
            })}
          />,
        );
        const img = container.querySelector(".crest img") as HTMLImageElement;
        expect(img).not.toBeNull();
        act(() => {
          img.dispatchEvent(new Event("error"));
        });
        expect(container.querySelector(".crest img")).toBeNull();
        expect(container.querySelector(".crest")?.textContent).toBe("ARS");
      });
    });
  });

  // plan 087: the hover primitive's one frontend consumer — a diagnostic
  // `.hovered` class proving `hover-changed` reaches the webview. Plan
  // 093 builds the actual hover FEATURES (081/082/084/idle
  // expanded-on-hover) — see the "idle hover-expanded peek/reveal (plan
  // 093)" describe block below for those; this block only pins the
  // prop/class wiring plan 087 shipped.
  describe("hovered prop (plan 087)", () => {
    it("toggles the .hovered class when hovered is true", () => {
      const { container } = render(<StatusRailCard slot={GOAL} hovered={true} />);
      expect(container.querySelector(".card-assembly.hovered")).not.toBeNull();
    });

    it("renders byte-identically to today when hovered is omitted (regression pin)", () => {
      const withHovered = render(<StatusRailCard slot={GOAL} hovered={false} />);
      const withoutHovered = render(<StatusRailCard slot={GOAL} />);
      expect(withoutHovered.container.innerHTML).toBe(withHovered.container.innerHTML);
      expect(withoutHovered.container.querySelector(".card-assembly.hovered")).toBeNull();
    });
  });

  // plan 093: the idle hover-expanded state (079 items 9/17/18) and the
  // TTL hover-pause (081's deferred half), exercised through the whole
  // StatusRailCard tree rather than IdleHoverPeek/TtlBar in isolation —
  // IdleHoverPeek.test.tsx and TtlBar.test.tsx already cover each
  // component's own behavior in depth; these pin how StatusRailCard
  // actually wires `hovered`/`status` into both of them.
  describe("idle hover-expanded peek/reveal (plan 093)", () => {
    const WEATHER_STATUS: StatusState = {
      paused: false,
      waiting: 0,
      football: { enabled: false, live: null },
      news: { enabled: false },
      weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy", isDay: true } },
      media: { enabled: false, current: null },
    };

    it("mounts no below-block while idle and not hovered (flanks stay rounded — 091's shell untouched)", () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} status={WEATHER_STATUS} hovered={false} />,
      );
      expect(container.querySelector(".below-block")).toBeNull();
    });

    // 091's `:not(:has(.below-block))` rounding law is untouched by this
    // plan — this pins the DOM condition that law keys off (a real
    // `.below-block` becoming present), which is what makes "flanks
    // un-round while the peek is open" true without editing that rule.
    it("mounts a .below-block.idle-peek while idle and hovered", () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} status={WEATHER_STATUS} hovered={true} />,
      );
      const peek = container.querySelector(".below-block.idle-peek");
      expect(peek).not.toBeNull();
      expect(container.querySelector(".card-assembly:has(.below-block)")).not.toBeNull();
    });

    it("never mounts the idle peek while a card is showing, regardless of hovered", () => {
      const { container } = render(<StatusRailCard slot={GOAL} hovered={true} />);
      expect(container.querySelector(".idle-peek")).toBeNull();
    });

    it("passes hoverPaused through to the TtlBar while a card is showing and hovered", () => {
      const { container } = render(<StatusRailCard slot={GOAL} hovered={true} />);
      expect(container.querySelector(".ttl-bar")).not.toBeNull();
      // TtlBar.test.tsx pins the freeze mechanics themselves; this only
      // proves StatusRailCard doesn't lose the wire — a stale/omitted
      // hoverPaused would still render a ttl-bar, so absence of a crash
      // plus TtlBar's own hoverPaused tests together cover this.
    });
  });

  // plan 107 Step B: compact->idle as one directional state machine. The
  // outer shell's geometry (priority accent class + expanded width class)
  // used to key off the live `showing` flag alone, which snapped straight
  // to idle the instant a card started exiting — a visible "grows before
  // shrinking" race against the still-fading below-block content (delayed
  // by useDelayedSwap's 220ms exit window). Entrance was always correct
  // (it already read the live slot); only the exit direction needed
  // fixing. Uses fake timers, scoped to this describe block only, so the
  // 220ms delayed-swap boundary is directly steppable (same pattern as
  // useDelayedSwap.test.ts).
  describe("compact->idle geometry as one state machine (plan 107 Step B)", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    // Entrance contract: geometry keys off the live slot, so the shell
    // grows (priority class + expanded class) on the exact same render
    // the promotion arrives — no timer advance needed. This is today's
    // shipped width-leads-content entrance, pinned rather than changed.
    it("idle->showing promotion applies showing geometry (priority + expanded) on the same render the promotion arrives", () => {
      const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();

      rerender(<StatusRailCard slot={GOAL} />);
      // synchronous — zero timer advance — entrance never waits on the
      // delayed swap.
      expect(container.querySelector(".card-assembly.high.expanded")).not.toBeNull();
      expect(container.querySelector(".card-assembly.idle")).toBeNull();
    });

    // Exit contract: the shell must hold its showing-geometry class for
    // the whole 220ms delayed-swap window, then flip to `.idle` in the
    // same tick the swap completes — never before.
    it("showing->idle exit holds the showing-geometry class until the 220ms swap, then settles idle", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} />);
      // mid-exit, immediately after the live slot goes empty: the swap
      // timer hasn't fired yet, so geometry must still read the frozen
      // renderedSlot — high, not idle.
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();
      expect(container.querySelector(".card-assembly.idle")).toBeNull();

      act(() => vi.advanceTimersByTime(219));
      // still one tick short of the swap — geometry must not have moved.
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();
      expect(container.querySelector(".card-assembly.idle")).toBeNull();

      act(() => vi.advanceTimersByTime(1));
      // the swap has now completed — geometry settles idle in the same
      // tick, not late.
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      expect(container.querySelector(".card-assembly.high")).toBeNull();
    });

    // The sharpest part of the contract: "geometry" is not just the
    // showing/idle shell class, it also covers `expanded` — an expanded
    // card must not silently collapse to compact mid-exit, only settle
    // to idle once the swap actually completes. GOAL is expanded: true.
    it("an expanded card's showing->idle exit keeps the expanded class until the swap completes", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} />);
      // mid-exit: expanded must still be held, not dropped to compact.
      expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();

      act(() => vi.advanceTimersByTime(219));
      expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();

      act(() => vi.advanceTimersByTime(1));
      // swap complete — settles idle, expanded gone (idle has no concept
      // of expanded).
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    });
  });
});
