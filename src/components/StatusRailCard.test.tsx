import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { act, cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  CONTENT_EXIT_MS,
  EXPAND_MS,
  INTERRUPT_EASE,
  INTERRUPT_EXIT_MS,
  NOTCHTAP_EASE,
  ROTATION_EXIT_MS,
} from "../animationTiming";
import type { AgentSessionView } from "../useAgentState";
import type { EspnMeta, SlotState, SourceKind } from "../useSlotState";
import type { StatusState } from "../useStatusState";
import { contentExitVariants, StatusRailCard } from "./StatusRailCard";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://converted${path}`,
}));

afterEach(cleanup);

function fireAnimationEnd(el: HTMLElement, animationName: string) {
  const event = new Event("animationend", { bubbles: true });
  Object.defineProperty(event, "animationName", { value: animationName });
  act(() => {
    el.dispatchEvent(event);
  });
}

async function flushFrame() {
  await act(async () => {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => resolve());
    });
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
  agentRuntime: null,
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
  agentRuntime: null,
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

const AGENT_NEEDS_INPUT: SlotState = {
  state: "showing",
  id: "n3",
  title: "Claude Code needs input",
  body: "Workspace command is waiting",
  eventType: "generic",
  priority: "high",
  signal: "generic",
  origin: "agent",
  agentRuntime: null,
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

const LIVE_MATCH: SlotState = {
  state: "showing",
  id: "match-1",
  title: "UCL: ARS 1–1 PSG",
  body: "Yellow Card — B. Saka 54'",
  eventType: "match_state",
  priority: "high",
  signal: "yellow_card",
  origin: "football",
  agentRuntime: null,
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
  agentRuntime: null,
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

const AGENT_RICH: SlotState = {
  state: "showing",
  id: "n5",
  title: "Claude Code needs input",
  body: "A permission prompt is waiting",
  eventType: "generic",
  priority: "high",
  signal: "generic",
  origin: "agent",
  agentRuntime: null,
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
    agentRuntime: null,
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
    it("applies pulse-goal (CSS burst + mounted three-ring ripple) on a goal signal", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.pulse-goal")).not.toBeNull();
      expect(container.querySelectorAll(".cele-ripple span")).toHaveLength(3);
    });

    it("keeps the ripple mounted until the THIRD ripple-out ends", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      // SAFETY: test helper guarantees element exists for this selector.
      const card = container.querySelector(".card-assembly") as HTMLElement;
      expect(container.querySelector(".cele-ripple")).not.toBeNull();

      fireAnimationEnd(card, "goal-overshoot");
      expect(container.querySelector(".cele-ripple")).not.toBeNull();

      fireAnimationEnd(card, "ripple-out");
      expect(container.querySelector(".cele-ripple")).not.toBeNull();
      fireAnimationEnd(card, "ripple-out");
      expect(container.querySelector(".cele-ripple")).not.toBeNull();
      fireAnimationEnd(card, "ripple-out");
      expect(container.querySelector(".cele-ripple")).toBeNull();
    });

    it("clears pulse-goal when its last ripple ring ends", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      // SAFETY: test helper guarantees element exists for this selector.
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "ripple-out");
      fireAnimationEnd(card, "ripple-out");
      expect(container.querySelector(".pulse-goal")).not.toBeNull();
      fireAnimationEnd(card, "ripple-out");
      expect(container.querySelector(".pulse-goal")).toBeNull();
    });

    it("replays the goal pulse for a second goal arriving during the first", async () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.pulse-goal")).not.toBeNull();

      rerender(<StatusRailCard slot={{ ...GOAL, id: "n2", body: "Arsenal 3-0" }} />);
      expect(container.querySelector(".card-assembly.pulse-goal")).toBeNull();
      expect(container.querySelector(".cele-ripple")).toBeNull();

      await flushFrame();
      expect(container.querySelector(".card-assembly.pulse-goal")).not.toBeNull();
      expect(container.querySelectorAll(".cele-ripple span")).toHaveLength(3);

      // SAFETY: test helper guarantees element exists for this selector.
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "ripple-out");
      fireAnimationEnd(card, "ripple-out");
      expect(container.querySelector(".pulse-goal")).not.toBeNull();
      fireAnimationEnd(card, "ripple-out");
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
      // SAFETY: test helper guarantees element exists for this selector.
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "red-alert");
      expect(container.querySelector(".pulse-red")).toBeNull();
    });

    it("ignores an unrelated animation ending on the same element", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      // SAFETY: test helper guarantees element exists for this selector.
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "goal-burst");
      expect(container.querySelector(".pulse-goal")).not.toBeNull();
    });

    it("does not replay the pulse on an unrelated re-render of the same notification", async () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      // SAFETY: test helper guarantees element exists for this selector.
      const card = container.querySelector(".card-assembly") as HTMLElement;
      fireAnimationEnd(card, "ripple-out");
      fireAnimationEnd(card, "ripple-out");
      fireAnimationEnd(card, "ripple-out");
      expect(container.querySelector(".pulse-goal")).toBeNull();

      rerender(<StatusRailCard slot={{ ...GOAL, expanded: false }} />);
      expect(container.querySelector(".pulse-goal")).toBeNull();
      await flushFrame();
      expect(container.querySelector(".pulse-goal")).toBeNull();
    });
  });

  it("never plays a goal or red-card pulse for a High-priority generic signal", () => {
    const { container } = render(<StatusRailCard slot={AGENT_NEEDS_INPUT} />);
    expect(container.querySelector(".pulse-goal")).toBeNull();
    expect(container.querySelector(".pulse-red")).toBeNull();
    expect(container.querySelector(".cele-ripple")).toBeNull();
  });

  it("renders the idle clock, not a card, when the slot is empty", () => {
    const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
    expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    expect(container.querySelector(".time-only")).not.toBeNull();
    expect(container.querySelector(".icon-strip")).not.toBeNull();
    expect(container.querySelector(".status-dots")).toBeNull();
    expect(screen.queryByText("GOAL")).toBeNull();
  });

  it("never applies a width-modifier class for status-rail activity", () => {
    const active: StatusState = {
      paused: false,
      waiting: 1,
      agent: { activeSessions: 0 },
      football: { enabled: true, live: { label: "MTL 0–0 TOR", minute: "12'" } },
      news: { enabled: true, chargeFraction: 0, chargeCount: 0, isCharged: false },
    };
    const inactive: StatusState = {
      paused: false,
      waiting: 0,
      agent: { activeSessions: 0 },
      football: { enabled: false, live: null },
      news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
    };

    const { container, rerender } = render(
      <StatusRailCard slot={{ state: "empty" }} status={active} />,
    );
    expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    expect(container.querySelector(".card-assembly.status")).toBeNull();

    rerender(<StatusRailCard slot={{ state: "empty" }} status={inactive} />);
    expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    expect(container.querySelector(".card-assembly.status")).toBeNull();

    rerender(<StatusRailCard slot={{ state: "empty" }} />);
    expect(container.querySelector(".card-assembly.status")).toBeNull();

    rerender(<StatusRailCard slot={GOAL} status={active} />);
    expect(container.querySelector(".card-assembly.status")).toBeNull();
  });

  describe("live region placement", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("is not a live region while idle", () => {
      const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector('[role="status"], [aria-live]')).toBeNull();
    });

    it("is exactly one live region while showing (non-live-match), and the clock sits outside it", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      act(() => vi.advanceTimersByTime(175));

      const regions = container.querySelectorAll('[role="status"]');
      expect(regions.length).toBe(1);

      const clockEl = container.querySelector(".time-only");
      expect(clockEl).not.toBeNull();
      expect(clockEl?.closest('[role="status"]')).toBeNull();

      expect(container.querySelector(".icon-strip")).toBeNull();
      expect(container.querySelector(".status-dots")).toBeNull();
    });

    it("a live-match card's own scorecard chrome is not a live region", () => {
      const { container } = render(<StatusRailCard slot={liveSlot()} />);
      act(() => vi.advanceTimersByTime(175));
      expect(container.querySelector('[role="status"]')).toBeNull();
      expect(container.querySelector("[aria-live]")).toBeNull();
    });

    it("the live-region attribute is present at t=0 of a promotion, before the below-block content mounts ~175ms later", () => {
      const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
      rerender(<StatusRailCard slot={GOAL} />);

      expect(container.querySelector('[role="status"]')).not.toBeNull();
      expect(container.querySelector('[aria-live="polite"]')).not.toBeNull();
      expect(container.querySelector(".below-block")).toBeNull();

      act(() => vi.advanceTimersByTime(175));

      expect(container.querySelector(".below-block")).not.toBeNull();
      expect(container.querySelector('[role="status"]')).not.toBeNull();
    });
  });

  it("renders the priority class and expanded class when showing", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    expect(container.querySelector(".card-assembly.high.expanded")).not.toBeNull();
    expect(screen.getByText("GOAL")).toBeTruthy();
    expect(screen.getAllByText("Arsenal 2-0").length).toBe(2);
  });

  it("renders no ttl-bar while idle", () => {
    const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
    expect(container.querySelector(".ttl-bar")).toBeNull();
  });

  it("renders the ttl-bar last, after the compact content and the manifest wrap, when showing", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    // SAFETY: test helper guarantees element exists for this selector.
    const cardContent = container.querySelector(".below-block .card-content") as HTMLElement;
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
    // SAFETY: test helper guarantees element exists for this selector.
    const categoryChip = container.querySelector(".chip-category") as HTMLElement;
    expect(categoryChip.textContent).toBe("Politics");
    expect(categoryChip.classList.contains("chip")).toBe(true);
    expect(screen.getByText("5m ago").classList.contains("notif-time-inline")).toBe(true);
    expect(screen.getByText("5m ago").classList.contains("chip")).toBe(false);
    expect(container.querySelector(".card-assembly.low")).not.toBeNull();
    expect(container.querySelector(".below-block.news-shade.cat-politics")).not.toBeNull();
    expect(container.querySelector(".tier-code")).toBeNull();
    expect(screen.queryByText("L1")).toBeNull();
    expect(screen.getByText("Wire").classList.contains("stamp")).toBe(true);
    expect(screen.getByText("Summary").classList.contains("manifest-label")).toBe(true);
    expect(container.querySelector(".manifest-inner.news")).toBeNull();
    expect(container.querySelector(".manifest-meta")).toBeNull();
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

    expect(container.querySelector(".masthead")?.textContent).toContain("RSS");
    expect(container.querySelector(".compact-hint")?.textContent).toBe("⌃⇧N more");
    expect(container.querySelector(".manifest-wrap")?.getAttribute("aria-hidden")).toBe("true");
    expect(container.querySelector(".chip-category")).toBeNull();
    expect(container.querySelector(".notif-time-inline")).toBeNull();
    expect(container.querySelector(".pub-meta")).toBeNull();
    expect(container.querySelector(".below-block.news-shade.cat-generic")).not.toBeNull();
  });

  it("renders exactly one time expression (relative age) in the compact news card's meta row — no duplicate published time", () => {
    const now = vi.spyOn(Date, "now").mockReturnValue(2_000_000_000_000);
    const { container } = render(<StatusRailCard slot={{ ...NEWS, expanded: false }} />);

    // SAFETY: test helper guarantees element exists for this selector.
    const metaRow = container.querySelector(".notif-meta-row") as HTMLElement;
    expect(metaRow).not.toBeNull();
    const ageNode = metaRow.querySelector(".notif-time-inline");
    expect(ageNode).not.toBeNull();
    expect(ageNode?.textContent).toBe("5m ago");
    expect(container.querySelector(".pub-meta")).toBeNull();
    expect(metaRow.lastElementChild).toBe(ageNode);

    now.mockRestore();
  });

  it("renders the expanded news manifest as a full-width summary with no meta duplication", () => {
    const { container } = render(<StatusRailCard slot={{ ...NEWS, link: null }} />);

    expect(container.querySelector(".manifest-inner.news")).toBeNull();
    expect(container.querySelector(".manifest-block")).not.toBeNull();
    expect(screen.getByText("Summary").classList.contains("manifest-label")).toBe(true);
    expect(screen.getByText(NEWS.body).classList.contains("manifest-text")).toBe(true);
    expect(container.querySelector(".manifest-meta")).toBeNull();

    // SAFETY: test helper guarantees element exists for this selector.
    const footer = container.querySelector(".manifest-footer") as HTMLElement;
    expect(footer).not.toBeNull();
    expect(footer.textContent).toContain("⌃⇧N collapse");
    expect(footer.textContent).not.toContain("⌃⇧O read");
  });

  it("renders the news title as the summary when body is empty (redundant-summary fallback)", () => {
    const { container } = render(<StatusRailCard slot={{ ...NEWS, body: "" }} />);
    // SAFETY: test helper guarantees element exists for this selector.
    const manifest = container.querySelector(".manifest") as HTMLElement;

    const summary = within(manifest).getByText(NEWS.title);
    expect(summary.classList.contains("manifest-text")).toBe(true);
  });

  it("shows only the collapse control in an expanded manifest without a link", () => {
    const { container } = render(<StatusRailCard slot={{ ...AGENT_NEEDS_INPUT, link: null }} />);

    // SAFETY: test helper guarantees element exists for this selector.
    const footer = container.querySelector(".manifest-footer") as HTMLElement;
    expect(footer.textContent).toContain("⌃⇧N collapse");
    expect(footer.textContent).not.toContain("⌃⇧O read");
    expect(screen.queryByText("⌃⇧N more")).toBeNull();
  });

  it("shows the read and collapse controls in a non-news manifest with a link", () => {
    const { container } = render(
      <StatusRailCard
        slot={{
          ...AGENT_NEEDS_INPUT,
          link: "https://example.com/local-notification",
        }}
      />,
    );

    // SAFETY: test helper guarantees element exists for this selector.
    const footer = container.querySelector(".manifest-footer") as HTMLElement;
    expect(footer.textContent).toContain("⌃⇧O read · ⌃⇧N collapse");
  });

  it("renders the subtitle and each detail pair as compact cells, not duplicated in the manifest", () => {
    const { container } = render(<StatusRailCard slot={AGENT_RICH} />);
    // SAFETY: test helper guarantees element exists for this selector.
    const compact = container.querySelector(".compact") as HTMLElement;

    expect(within(compact).getByText("Permission request")).toBeTruthy();
    expect(within(compact).getByText("Tool")).toBeTruthy();
    expect(within(compact).getByText("Bash")).toBeTruthy();
    expect(within(compact).getByText("Command")).toBeTruthy();
    expect(within(compact).getByText("git push origin master")).toBeTruthy();
    expect(within(compact).getByText("Project")).toBeTruthy();
    expect(within(compact).getByText("/Users/x/proj")).toBeTruthy();

    // SAFETY: test helper guarantees element exists for this selector.
    const manifest = container.querySelector(".manifest") as HTMLElement;
    expect(within(manifest).queryByText("Subtitle")).toBeNull();
    expect(within(manifest).queryByText("Tool")).toBeNull();
    expect(within(manifest).queryByText("Bash")).toBeNull();
    expect(manifest.querySelector(".manifest-fields")).toBeNull();
  });

  it("shows the same subtitle and detail pairs in the compact card whether collapsed or expanded", () => {
    const { container } = render(<StatusRailCard slot={{ ...AGENT_RICH, expanded: false }} />);

    // SAFETY: test helper guarantees element exists for this selector.
    const wrap = container.querySelector(".manifest-wrap") as HTMLElement;
    expect(wrap.getAttribute("aria-hidden")).toBe("true");
    // SAFETY: test helper guarantees element exists for this selector.
    const compact = container.querySelector(".compact") as HTMLElement;
    expect(within(compact).getByText("Permission request")).toBeTruthy();
    expect(within(compact).getByText("Tool")).toBeTruthy();
    expect(within(compact).getByText("Bash")).toBeTruthy();
    expect(within(compact).getByText("Project")).toBeTruthy();
  });

  it("renders only the message body and hints in an agent slot's expanded manifest", () => {
    const { container } = render(<StatusRailCard slot={AGENT_RICH} />);
    // SAFETY: test helper guarantees element exists for this selector.
    const manifest = container.querySelector(".manifest") as HTMLElement;

    expect(within(manifest).getByText("Message").classList.contains("manifest-label")).toBe(true);
    expect(within(manifest).getByText(AGENT_RICH.body)).toBeTruthy();
    expect(manifest.querySelector(".manifest-meta")).toBeNull();
    expect(manifest.querySelector(".manifest-fields")).toBeNull();
    expect(manifest.querySelector(".detail-facts")).toBeNull();
    expect(manifest.querySelector(".fact-pill")).toBeNull();
    expect(within(manifest).queryByText("Subtitle")).toBeNull();

    // SAFETY: test helper guarantees element exists for this selector.
    const footer = manifest.querySelector(".manifest-footer") as HTMLElement;
    expect(footer.textContent).toContain("⌃⇧N collapse");
    expect(footer.textContent).not.toContain("⌃⇧O read");
  });

  it("carries aria-hidden on the manifest wrapper only while collapsed", () => {
    const { container, rerender } = render(
      <StatusRailCard slot={{ ...AGENT_RICH, expanded: false }} />,
    );
    expect(container.querySelector(".manifest-wrap")?.getAttribute("aria-hidden")).toBe("true");

    rerender(<StatusRailCard slot={AGENT_RICH} />);
    expect(container.querySelector(".manifest-wrap")?.getAttribute("aria-hidden")).toBe("false");
  });

  it("renders Clock and per-side Cards in the collapsed view for a live-match card", () => {
    const { container } = render(<StatusRailCard slot={LIVE_MATCH} />);
    // SAFETY: test helper guarantees element exists for this selector.
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
    // SAFETY: test helper guarantees element exists for this selector.
    const compact = container.querySelector(".compact") as HTMLElement;
    expect(within(compact).getByText("Arsenal 2-0")).toBeTruthy();
    expect(compact.querySelector(".detail-facts")).toBeNull();
    expect(compact.querySelector(".fact-pill")).toBeNull();
  });

  it("renders a detail value that contains an '=' verbatim (first-'=' split is CLI-side only)", () => {
    render(
      <StatusRailCard
        slot={{
          ...AGENT_RICH,
          details: [{ label: "Command", value: "FOO=bar make build" }],
        }}
      />,
    );
    expect(screen.getByText("FOO=bar make build")).toBeTruthy();
  });

  describe("agent accent", () => {
    function genericSlot(origin: SourceKind): SlotState {
      // SAFETY: validated via preceding checks; type assertion safe here.
      return { ...(AGENT_NEEDS_INPUT as Extract<SlotState, { state: "showing" }>), origin };
    }

    it('renders the masthead kicker "agent" (no Agent chip) and the below-block hairline for origin: agent', () => {
      const { container } = render(<StatusRailCard slot={genericSlot("agent")} />);
      expect(container.querySelector(".masthead")?.textContent).toContain("agent");
      expect(container.querySelector(".chip-cmux")).toBeNull();
      expect(screen.queryByText("Agent")).toBeNull();
      expect(container.querySelector(".below-block.agent-origin")).not.toBeNull();
    });

    it("shows the origin-derived kicker (never the removed Agent chip) for every non-agent origin", () => {
      const expectedKicker = {
        football: "football",
        news: "news",
        manual: "cli",
      } satisfies Record<Exclude<SourceKind, "agent">, string>;
      for (const origin of ["football", "news", "manual"] as const) {
        const { container, unmount } = render(<StatusRailCard slot={genericSlot(origin)} />);
        expect(container.querySelector(".masthead")?.textContent).toContain(expectedKicker[origin]);
        expect(container.querySelector(".chip-cmux")).toBeNull();
        expect(container.querySelector(".below-block.agent-origin")).toBeNull();
        expect(container.innerHTML).not.toContain("chip-cmux");
        expect(container.innerHTML).not.toContain("agent-origin");
        unmount();
      }
    });

    it("never touches the priority accent channel — same priority, different origin, identical shell classes", () => {
      const { container: agentContainer } = render(<StatusRailCard slot={genericSlot("agent")} />);
      const { container: manualContainer } = render(
        <StatusRailCard slot={genericSlot("manual")} />,
      );
      // SAFETY: test helper guarantees element exists for this selector.
      const agentShell = agentContainer.querySelector(".card-assembly") as HTMLElement;
      // SAFETY: test helper guarantees element exists for this selector.
      const manualShell = manualContainer.querySelector(".card-assembly") as HTMLElement;
      expect(agentShell.className).toBe(manualShell.className);
      expect(agentShell.className).not.toContain("agent");
    });
  });

  describe("source identity class", () => {
    function belowBlockClasses(container: HTMLElement): string[] {
      return (container.querySelector(".below-block")?.className ?? "").split(" ").filter(Boolean);
    }

    it("resolves the runtime-specific class for an agent slot with a known agentRuntime", () => {
      const { container } = render(
        <StatusRailCard
          slot={{
            // SAFETY: validated via preceding checks; type assertion safe here.
            ...(AGENT_NEEDS_INPUT as Extract<SlotState, { state: "showing" }>),
            agentRuntime: "claude-code",
          }}
        />,
      );
      const classes = belowBlockClasses(container);
      expect(classes).toContain("src-claude-code");
      expect(classes).not.toContain("cat-generic");
    });

    it("falls back to the neutral src-agent class when agentRuntime is null on an agent-origin slot", () => {
      const { container } = render(<StatusRailCard slot={AGENT_NEEDS_INPUT} />);
      const classes = belowBlockClasses(container);
      expect(classes).toContain("src-agent");
      expect(classes).not.toContain("cat-generic");
    });

    it("resolves src-manual for a manual-origin slot", () => {
      const { container } = render(
        <StatusRailCard
          slot={{
            // SAFETY: validated via preceding checks; type assertion safe here.
            ...(AGENT_NEEDS_INPUT as Extract<SlotState, { state: "showing" }>),
            origin: "manual",
            agentRuntime: null,
          }}
        />,
      );
      expect(belowBlockClasses(container)).toContain("src-manual");
    });

    it("resolves src-football for a football-origin slot", () => {
      const { container } = render(<StatusRailCard slot={GOAL} />);
      expect(belowBlockClasses(container)).toContain("src-football");
    });

    it("keeps cat-* on a news slot and adds no src-* class", () => {
      const { container } = render(<StatusRailCard slot={NEWS} />);
      const classes = belowBlockClasses(container);
      expect(classes.some((c) => c.startsWith("cat-"))).toBe(true);
      expect(classes.some((c) => c.startsWith("src-"))).toBe(false);
    });
  });

  describe("resting_state: notch", () => {
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
      expect(container.querySelector(".below-block")).not.toBeNull();

      await vi.waitFor(() => {
        const shell = container.querySelector(".card-assembly");
        expect(shell?.classList.contains("bare")).toBe(true);
        expect(container.querySelector(".below-block")).toBeNull();
      });
    });

    it("an unhovered showing->idle exit unmounts the clock before `.bare` lands (not just once it does); rail mode keeps it mounted, and brings the icon strip back, across the same exit", async () => {
      const notch = render(<StatusRailCard slot={GOAL} restingState="notch" />);
      expect(notch.container.querySelector(".time-only")).not.toBeNull();
      expect(notch.container.querySelector(".icon-strip")).toBeNull();

      notch.rerender(<StatusRailCard slot={{ state: "empty" }} restingState="notch" />);
      await vi.waitFor(() => {
        expect(notch.container.querySelector(".time-only")).toBeNull();
      });
      expect(notch.container.querySelector(".icon-strip")).toBeNull();
      expect(notch.container.querySelector(".card-assembly.bare")).toBeNull();
      expect(notch.container.querySelector(".card-assembly.exit-to-bare")).not.toBeNull();

      const rail = render(<StatusRailCard slot={GOAL} restingState="rail" />);
      rail.rerender(<StatusRailCard slot={{ state: "empty" }} restingState="rail" />);
      await vi.waitFor(() => {
        expect(rail.container.querySelector(".card-assembly.idle")).not.toBeNull();
      });
      expect(rail.container.querySelector(".time-only")).not.toBeNull();
      expect(rail.container.querySelector(".icon-strip")).not.toBeNull();
    });
  });

  describe("resting_state: rail (default) and unset", () => {
    it('renders the idle clock/status rail when restingState is "rail"', () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} restingState="rail" />,
      );
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      expect(container.querySelector(".time-only")).not.toBeNull();
      expect(container.querySelector(".icon-strip")).not.toBeNull();
    });

    it("renders the idle clock/status rail when restingState is omitted", () => {
      const { container } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      expect(container.querySelector(".time-only")).not.toBeNull();
      expect(container.querySelector(".icon-strip")).not.toBeNull();
    });
  });

  it("updates the queue segments on a waiting-count change without remounting the card", () => {
    const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
    const barBefore = container.querySelector(".ttl-bar");
    expect(barBefore?.querySelectorAll(".ttl-seg")).toHaveLength(3);
    expect(barBefore?.querySelectorAll(".ttl-seg.done")).toHaveLength(0);
    expect(barBefore?.querySelector(".ttl-fill")).not.toBeNull();

    rerender(<StatusRailCard slot={{ ...GOAL, queueTotal: 5, queueDone: 1 }} />);
    const barAfter = container.querySelector(".ttl-bar");
    expect(barAfter).toBe(barBefore);
    expect(barAfter?.querySelectorAll(".ttl-seg")).toHaveLength(5);
    expect(barAfter?.querySelectorAll(".ttl-seg.done")).toHaveLength(1);
    expect(barAfter?.querySelector(".ttl-fill")).toBe(barBefore?.querySelector(".ttl-fill"));
  });

  it("never renders a standalone .track row in the compact card", () => {
    const { container } = render(<StatusRailCard slot={GOAL} />);
    expect(container.querySelector(".track")).toBeNull();
    expect(container.querySelector(".compact .track")).toBeNull();
    expect(container.querySelector(".compact")).not.toBeNull();
  });

  describe("same-slot rotation uses lighter timings", () => {
    it("a showing(A)->showing(B) key change marks the new content as a rotation", async () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(
        container.querySelector(".below-block .card-content")?.getAttribute("data-rotation-swap"),
      ).toBe("false");

      rerender(<StatusRailCard slot={RED_CARD} />);
      await vi.waitFor(() => {
        expect(screen.getByText("Red Card")).toBeTruthy();
      });
      expect(
        container.querySelector(".below-block .card-content")?.getAttribute("data-rotation-swap"),
      ).toBe("true");
    });

    describe("idle->showing promotion", () => {
      beforeEach(() => {
        vi.useFakeTimers();
      });
      afterEach(() => {
        vi.useRealTimers();
      });

      it("is never marked a rotation", () => {
        const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
        rerender(<StatusRailCard slot={GOAL} />);
        act(() => vi.advanceTimersByTime(175));
        expect(
          container.querySelector(".below-block .card-content")?.getAttribute("data-rotation-swap"),
        ).toBe("false");
      });
    });

    it("a second rotation in a row is still marked a rotation (not just the first one after idle)", async () => {
      const { rerender } = render(<StatusRailCard slot={GOAL} />);
      rerender(<StatusRailCard slot={RED_CARD} />);
      await vi.waitFor(() => {
        expect(screen.getByText("Red Card")).toBeTruthy();
      });

      rerender(<StatusRailCard slot={{ ...GOAL, id: "n4" }} />);
      await vi.waitFor(() => {
        expect(screen.queryByText("Red Card")).toBeNull();
      });
      expect(
        screen.getByText("GOAL").closest(".card-content")?.getAttribute("data-rotation-swap"),
      ).toBe("true");
    });

    it("a same-id update (no key change at all) is not itself a rotation swap replay — the node stays mounted, unmarked", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      const before = container.querySelector(".below-block .card-content");
      rerender(<StatusRailCard slot={{ ...GOAL, queueDone: 1 }} />);
      const after = container.querySelector(".below-block .card-content");
      expect(after).toBe(before);
      // SAFETY: test helper guarantees element exists for this selector.
      expect((after as HTMLElement | null)?.getAttribute("data-rotation-swap")).toBe("false");
    });
  });

  describe("Priority Preemption interrupt detection", () => {
    const LOW_WITH_TIME_LEFT: SlotState = {
      ...GOAL,
      id: "low-fresh",
      priority: "low",
      ttlMs: 8000,
      remainingMs: 8000,
    };
    const LOW_ALMOST_DONE: SlotState = {
      ...GOAL,
      id: "low-almost-done",
      priority: "low",
      ttlMs: 8000,
      remainingMs: 50,
    };
    const HIGH_ARRIVAL: SlotState = {
      ...RED_CARD,
      id: "high-arrival",
      priority: "high",
      ttlMs: 5000,
      remainingMs: 5000,
    };
    const HIGH_VISIBLE: SlotState = {
      ...GOAL,
      id: "high-visible",
      priority: "high",
      ttlMs: 8000,
      remainingMs: 8000,
    };
    const LOW_ARRIVAL: SlotState = {
      ...RED_CARD,
      id: "low-arrival",
      priority: "low",
      ttlMs: 5000,
      remainingMs: 5000,
    };
    const MEDIUM_ARRIVAL: SlotState = {
      ...RED_CARD,
      id: "medium-arrival",
      priority: "medium",
      ttlMs: 5000,
      remainingMs: 5000,
    };

    it("a strictly-higher-priority arrival that cuts a Low card short (real time left) is marked an interrupt, not a plain rotation", async () => {
      const { container, rerender } = render(<StatusRailCard slot={LOW_WITH_TIME_LEFT} />);
      rerender(<StatusRailCard slot={HIGH_ARRIVAL} />);
      await vi.waitFor(() => {
        expect(screen.getByText("Red Card")).toBeTruthy();
      });
      const content = container.querySelector(".below-block .card-content");
      expect(content?.getAttribute("data-interrupt-swap")).toBe("true");
      expect(content?.getAttribute("data-rotation-swap")).toBe("false");
    });

    it("a higher-priority arrival after the outgoing item's own turn had already run out is an ordinary rotation, not an interrupt", async () => {
      const { container, rerender } = render(<StatusRailCard slot={LOW_ALMOST_DONE} />);
      rerender(<StatusRailCard slot={HIGH_ARRIVAL} />);
      await vi.waitFor(() => {
        expect(screen.getByText("Red Card")).toBeTruthy();
      });
      const content = container.querySelector(".below-block .card-content");
      expect(content?.getAttribute("data-interrupt-swap")).toBe("false");
      expect(content?.getAttribute("data-rotation-swap")).toBe("true");
    });

    it("equal priority never interrupts, even with plenty of time left on the outgoing card", async () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      rerender(<StatusRailCard slot={RED_CARD} />);
      await vi.waitFor(() => {
        expect(screen.getByText("Red Card")).toBeTruthy();
      });
      const content = container.querySelector(".below-block .card-content");
      expect(content?.getAttribute("data-interrupt-swap")).toBe("false");
      expect(content?.getAttribute("data-rotation-swap")).toBe("true");
    });

    it("a lower-priority arrival is never an interrupt", async () => {
      const { container, rerender } = render(<StatusRailCard slot={HIGH_VISIBLE} />);
      rerender(<StatusRailCard slot={LOW_ARRIVAL} />);
      await vi.waitFor(() => {
        expect(screen.getByText("Red Card")).toBeTruthy();
      });
      const content = container.querySelector(".below-block .card-content");
      expect(content?.getAttribute("data-interrupt-swap")).toBe("false");
      expect(content?.getAttribute("data-rotation-swap")).toBe("true");
    });

    it("Medium preempting a Low card with time left is also marked an interrupt (not just High)", async () => {
      const { container, rerender } = render(<StatusRailCard slot={LOW_WITH_TIME_LEFT} />);
      rerender(<StatusRailCard slot={MEDIUM_ARRIVAL} />);
      await vi.waitFor(() => {
        expect(screen.getByText("Red Card")).toBeTruthy();
      });
      const content = container.querySelector(".below-block .card-content");
      expect(content?.getAttribute("data-interrupt-swap")).toBe("true");
    });

    it("an idle->showing promotion is never marked an interrupt", () => {
      vi.useFakeTimers();
      try {
        const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
        rerender(<StatusRailCard slot={HIGH_ARRIVAL} />);
        act(() => vi.advanceTimersByTime(175));
        expect(
          container
            .querySelector(".below-block .card-content")
            ?.getAttribute("data-interrupt-swap"),
        ).toBe("false");
      } finally {
        vi.useRealTimers();
      }
    });
  });

  describe("contentExitVariants", () => {
    it("the rotation exit uses ROTATION_EXIT_MS, in seconds, with the house ease", () => {
      // SAFETY: validated via preceding checks; type assertion safe here.
      const variant = contentExitVariants.exit({
        isRotation: true,
        isInterrupt: false,
      }) as {
        transition: { duration: number; ease: unknown };
      };
      expect(variant.transition.duration).toBe(ROTATION_EXIT_MS / 1000);
      expect(variant.transition.ease).toEqual(NOTCHTAP_EASE);
    });

    it("the non-rotation (promotion/exit) leg uses CONTENT_EXIT_MS, in seconds, with the house ease", () => {
      // SAFETY: validated via preceding checks; type assertion safe here.
      const variant = contentExitVariants.exit({
        isRotation: false,
        isInterrupt: false,
      }) as {
        transition: { duration: number; ease: unknown };
      };
      expect(variant.transition.duration).toBe(CONTENT_EXIT_MS / 1000);
      expect(variant.transition.ease).toEqual(NOTCHTAP_EASE);
    });

    it("the interrupt exit uses INTERRUPT_EXIT_MS with its own sharp ease and a yank, even when isRotation is also true", () => {
      // SAFETY: validated via preceding checks; type assertion safe here.
      const variant = contentExitVariants.exit({
        isRotation: true,
        isInterrupt: true,
      }) as {
        opacity: number;
        transform: string;
        transition: { duration: number; ease: unknown };
      };
      expect(variant.transition.duration).toBe(INTERRUPT_EXIT_MS / 1000);
      expect(variant.transition.duration).toBeLessThan(ROTATION_EXIT_MS / 1000);
      expect(variant.transition.ease).toEqual(INTERRUPT_EASE);
      expect(variant.transition.ease).not.toEqual(NOTCHTAP_EASE);
      expect(variant.opacity).toBe(0);
      expect(variant.transform).toContain("translateY(8px)");
      expect(variant.transform).toContain("scale(0.96)");
    });
  });

  describe("live-match football scorecard", () => {
    it("renders the title (slot.body) and stamp, plus the league chip, live-pill, clock, crests-as-abbrev, and score", () => {
      const { container } = render(<StatusRailCard slot={liveSlot()} />);
      expect(container.querySelector(".title.headline")?.textContent).toBe("Goal — K. Havertz 78'");
      expect(container.querySelector(".stamp")?.textContent).toBe("Live");
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

    it("goal: title is the body text, plays cele-goal (not pulse-goal), never the ripple", () => {
      const { container } = render(
        <StatusRailCard slot={liveSlot({ signal: "goal", body: "Goal — K. Havertz 78'" })} />,
      );
      expect(container.querySelector(".title.headline")?.textContent).toBe("Goal — K. Havertz 78'");
      expect(container.querySelector(".card-assembly.cele-goal")).not.toBeNull();
      expect(container.querySelector(".card-assembly.pulse-goal")).toBeNull();
      expect(container.querySelector(".cele-ripple")).toBeNull();
    });

    it("penalty scored: title is the body text, same cele-goal celebration family", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({ signal: "goal", body: "Penalty - Scored — Mohamed Salah 44'" })}
        />,
      );
      expect(container.querySelector(".title.headline")?.textContent).toBe(
        "Penalty - Scored — Mohamed Salah 44'",
      );
      expect(container.querySelector(".card-assembly.cele-goal")).not.toBeNull();
    });

    it("own goal: score updates, title is the body text, NO celebration", () => {
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
      expect(container.querySelector(".title.headline")?.textContent).toBe(
        "Own Goal — W. Saliba 12'",
      );
      expect(container.querySelector(".card-assembly.cele-goal")).toBeNull();
      expect(container.querySelector(".card-assembly.cele-yc")).toBeNull();
      expect(container.querySelector(".card-assembly.cele-rc")).toBeNull();
    });

    it("yellow card: title is the body text, stamp reads Card, cele-yc, and the per-side cards line ticks up", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({
            signal: "yellow_card",
            body: "Yellow Card — B. Saka 54'",
            espn: { ...ESPN_BASE, homeCards: [1, 0], awayCards: [2, 0] },
          })}
        />,
      );
      expect(container.querySelector(".title.headline")?.textContent).toBe(
        "Yellow Card — B. Saka 54'",
      );
      expect(container.querySelector(".stamp")?.textContent).toBe("Card");
      expect(container.querySelector(".card-assembly.cele-yc")).not.toBeNull();
      expect(container.querySelector(".cards-line")?.textContent).toBe("ARS 1Y0R · PSG 2Y0R");
    });

    it("red card: title is the body text, stamp reads Off, and cele-rc", () => {
      const { container } = render(
        <StatusRailCard
          slot={liveSlot({
            signal: "red_card",
            body: "Red Card — M. Dembélé 71'",
            espn: { ...ESPN_BASE, homeCards: [1, 0], awayCards: [2, 1] },
          })}
        />,
      );
      expect(container.querySelector(".title.headline")?.textContent).toBe(
        "Red Card — M. Dembélé 71'",
      );
      expect(container.querySelector(".stamp")?.textContent).toBe("Off");
      expect(container.querySelector(".card-assembly.cele-rc")).not.toBeNull();
      expect(container.querySelector(".card-assembly.pulse-red")).toBeNull();
    });

    it.each([
      ["foul", "Foul", "Foul — D. Rice 62'"],
      ["offside", "Offside", "Offside — K. Mbappé 55'"],
      ["var_check", "VAR", "VAR check — possible penalty 67'"],
      ["substitution", "Sub", "Substitution — L. Trossard for G. Martinelli 70'"],
    ] as const)(
      "%s: title is the body text, stamp reads %s, no celebration",
      (signal, stamp, body) => {
        const { container } = render(<StatusRailCard slot={liveSlot({ signal, body })} />);
        expect(container.querySelector(".title.headline")?.textContent).toBe(body);
        expect(container.querySelector(".stamp")?.textContent).toBe(stamp);
        expect(container.querySelector(".card-assembly.cele-goal")).toBeNull();
        expect(container.querySelector(".card-assembly.cele-yc")).toBeNull();
        expect(container.querySelector(".card-assembly.cele-rc")).toBeNull();
      },
    );

    it("clears cele-goal on its ring animation ending, and never both pulse and cele stack", () => {
      const { container } = render(<StatusRailCard slot={liveSlot({ signal: "goal" })} />);
      // SAFETY: test helper guarantees element exists for this selector.
      const card = container.querySelector(".card-assembly") as HTMLElement;
      expect(container.querySelector(".cele-goal")).not.toBeNull();
      fireAnimationEnd(card, "cele-ring");
      expect(container.querySelector(".cele-goal")).toBeNull();
    });

    it("clears cele-rc on the red-strobe animation ending", () => {
      const { container } = render(<StatusRailCard slot={liveSlot({ signal: "red_card" })} />);
      // SAFETY: test helper guarantees element exists for this selector.
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

    it("full-time: Final pill (live-dot fades out, not unmounted) and the FT clock", () => {
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
      expect(pill?.querySelector(".live-dot")).not.toBeNull();
      expect(container.querySelector(".clock-pill")?.textContent).toBe("FT");
    });

    it("omits the cards line on a clean match (no cards either side)", () => {
      const { container } = render(<StatusRailCard slot={liveSlot()} />);
      expect(container.querySelector(".cards-line")).toBeNull();
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
      expect(container.querySelector(".sc-head")).not.toBeNull();
      expect(container.querySelector(".score-row")).not.toBeNull();
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
        // SAFETY: test helper guarantees element exists for this selector.
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
        // SAFETY: test helper guarantees element exists for this selector.
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

  describe("hovered prop", () => {
    it("toggles the .hovered class when hovered is true", () => {
      const { container } = render(<StatusRailCard slot={GOAL} hovered={true} />);
      expect(container.querySelector(".card-assembly.hovered")).not.toBeNull();
    });

    it("renders byte-identically when hovered is omitted (regression pin)", () => {
      const withHovered = render(<StatusRailCard slot={GOAL} hovered={false} />);
      const withoutHovered = render(<StatusRailCard slot={GOAL} />);
      expect(withoutHovered.container.innerHTML).toBe(withHovered.container.innerHTML);
      expect(withoutHovered.container.querySelector(".card-assembly.hovered")).toBeNull();
    });
  });

  describe("manifest expand is keyboard-only, not hover-driven", () => {
    const COLLAPSED: SlotState = { ...GOAL, expanded: false };
    const EXPANDED: SlotState = { ...GOAL, expanded: true };

    it("stays collapsed while hovered if slot.expanded is false", () => {
      const { container } = render(<StatusRailCard slot={COLLAPSED} hovered={true} />);
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
      const wrap = container.querySelector(".manifest-wrap");
      expect(wrap?.classList.contains("expanded")).toBe(false);
      expect(wrap?.getAttribute("aria-hidden")).toBe("true");
    });

    it("stays expanded regardless of hover if slot.expanded is true", () => {
      const { container: hoveredCase } = render(<StatusRailCard slot={EXPANDED} hovered={true} />);
      expect(hoveredCase.querySelector(".card-assembly.expanded")).not.toBeNull();

      const { container: notHoveredCase } = render(
        <StatusRailCard slot={EXPANDED} hovered={false} />,
      );
      expect(notHoveredCase.querySelector(".card-assembly.expanded")).not.toBeNull();
    });
  });

  describe("arrival-pop marker (`promoting`)", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });
    afterEach(() => {
      vi.useRealTimers();
    });

    it("lands on the very same render an idle->showing promotion arrives — no timer advance", () => {
      const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.promoting")).toBeNull();

      rerender(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.promoting")).not.toBeNull();
      expect(container.querySelector(".card-assembly.high.expanded")).not.toBeNull();
    });

    it("clears exactly EXPAND_MS after the entrance, not before", () => {
      const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
      rerender(<StatusRailCard slot={GOAL} />);

      act(() => vi.advanceTimersByTime(EXPAND_MS - 1));
      expect(container.querySelector(".card-assembly.promoting")).not.toBeNull();

      act(() => vi.advanceTimersByTime(1));
      expect(container.querySelector(".card-assembly.promoting")).toBeNull();
      expect(container.querySelector(".card-assembly.high.expanded")).not.toBeNull();
    });

    it("is dropped by an ordinary showing(A)->showing(B) rotation on that same render", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.promoting")).not.toBeNull();

      rerender(<StatusRailCard slot={RED_CARD} />);
      expect(container.querySelector(".card-assembly.promoting")).toBeNull();
    });

    it("is never armed by a hover — manifest expand is keyboard-only", () => {
      const { container, rerender } = render(
        <StatusRailCard slot={{ ...GOAL, expanded: false }} hovered={false} />,
      );
      act(() => vi.advanceTimersByTime(EXPAND_MS));
      expect(container.querySelector(".card-assembly.promoting")).toBeNull();

      rerender(<StatusRailCard slot={{ ...GOAL, expanded: false }} hovered={true} />);
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
      expect(container.querySelector(".card-assembly.promoting")).toBeNull();
    });

    it("is dropped on the same render a showing->idle exit begins — never coexists with `.exiting`", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.promoting")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.exiting")).not.toBeNull();
      expect(container.querySelector(".card-assembly.promoting")).toBeNull();
    });
  });

  describe("idle hover-expanded peek/reveal", () => {
    const AMBIENT_STATUS: StatusState = {
      paused: false,
      waiting: 0,
      agent: { activeSessions: 0 },
      football: { enabled: true, live: { label: "MTL 1-0 TOR", minute: "63'" } },
      news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
    };

    it("mounts no below-block while idle and not hovered (flanks stay rounded — 091's shell untouched)", () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} status={AMBIENT_STATUS} hovered={false} />,
      );
      expect(container.querySelector(".below-block")).toBeNull();
    });

    it("mounts a .below-block.idle-peek while idle and hovered", () => {
      const { container } = render(
        <StatusRailCard slot={{ state: "empty" }} status={AMBIENT_STATUS} hovered={true} />,
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
    });
  });

  describe("peek survives a promotion mid-open", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("keeps the peek's own exit animation playing across a promotion, instead of tearing it out unanimated; the card content mounts regardless", () => {
      const { container, rerender } = render(
        <StatusRailCard slot={{ state: "empty" }} hovered={true} />,
      );
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();

      rerender(<StatusRailCard slot={GOAL} hovered={true} />);
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
      expect(container.querySelector(".below-block .card-content")).toBeNull();

      act(() => vi.advanceTimersByTime(175));

      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
      expect(container.querySelector(".below-block .card-content")).not.toBeNull();
    });
  });

  it("the peek actually leaves the DOM once its own exit animation finishes after a mid-open promotion", async () => {
    const { container, rerender } = render(
      <StatusRailCard slot={{ state: "empty" }} hovered={true} />,
    );
    expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();

    rerender(<StatusRailCard slot={GOAL} hovered={true} />);

    await vi.waitFor(() => {
      expect(container.querySelector(".idle-peek")).toBeNull();
    });
    expect(container.querySelector(".below-block .card-content")).not.toBeNull();
  });

  describe("idle face", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("never appears while a card is showing", () => {
      const { container } = render(<StatusRailCard slot={GOAL} hovered={false} />);
      act(() => vi.advanceTimersByTime(10_000));
      expect(container.querySelector(".idle-face")).toBeNull();
    });

    it("never appears while idle but hovered", () => {
      const { container } = render(<StatusRailCard slot={{ state: "empty" }} hovered={true} />);
      act(() => vi.advanceTimersByTime(10_000));
      expect(container.querySelector(".idle-face")).toBeNull();
    });

    it("appears only after the idle delay elapses while truly idle", () => {
      const { container } = render(<StatusRailCard slot={{ state: "empty" }} hovered={false} />);
      expect(container.querySelector(".idle-face")).toBeNull();

      act(() => vi.advanceTimersByTime(4499));
      expect(container.querySelector(".idle-face")).toBeNull();

      act(() => vi.advanceTimersByTime(1));
      expect(container.querySelector(".idle-face")).not.toBeNull();
    });

    it("resets the reveal delay when hover breaks idle then idle resumes, rather than reusing a stale timer", () => {
      const { container, rerender } = render(
        <StatusRailCard slot={{ state: "empty" }} hovered={false} />,
      );
      act(() => vi.advanceTimersByTime(3000));
      expect(container.querySelector(".idle-face")).toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} hovered={true} />);
      rerender(<StatusRailCard slot={{ state: "empty" }} hovered={false} />);

      act(() => vi.advanceTimersByTime(1500));
      expect(container.querySelector(".idle-face")).toBeNull();

      act(() => vi.advanceTimersByTime(3000));
      expect(container.querySelector(".idle-face")).not.toBeNull();
    });

    describe("on real notch hardware (presentationFacts().mode === 'notch')", () => {
      beforeEach(() => {
        window.__NOTCHTAP_MODE__ = "notch";
      });

      afterEach(() => {
        delete window.__NOTCHTAP_MODE__;
      });

      it("never reveals the face even long past the reveal delay while truly idle", () => {
        const { container } = render(<StatusRailCard slot={{ state: "empty" }} hovered={false} />);
        act(() => vi.advanceTimersByTime(60_000));
        expect(container.querySelector(".idle-face")).toBeNull();
      });

      it("never mounts the face at all — no new timer gets scheduled by it as idle continues", () => {
        render(<StatusRailCard slot={{ state: "empty" }} hovered={false} />);
        const baseline = vi.getTimerCount();
        act(() => vi.advanceTimersByTime(60_000));
        expect(vi.getTimerCount()).toBeLessThanOrEqual(baseline);
      });
    });
  });

  describe("compact->idle geometry as one state machine", () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("idle->showing promotion applies showing geometry (priority + expanded) on the same render the promotion arrives", () => {
      const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();

      rerender(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.high.expanded")).not.toBeNull();
      expect(container.querySelector(".card-assembly.idle")).toBeNull();
    });

    it("a promotion arriving with expanded:false renders compact on the very first render — no expand-then-collapse flash", () => {
      const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();

      rerender(<StatusRailCard slot={{ ...GOAL, expanded: false }} />);
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
      expect(container.querySelector(".card-assembly.idle")).toBeNull();
    });

    it("showing->idle exit holds the showing-geometry class until the SWAP_EXIT_MS swap, then settles idle", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();
      expect(container.querySelector(".card-assembly.idle")).toBeNull();

      act(() => vi.advanceTimersByTime(174));
      expect(container.querySelector(".card-assembly.high")).not.toBeNull();
      expect(container.querySelector(".card-assembly.idle")).toBeNull();

      act(() => vi.advanceTimersByTime(1));
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
      expect(container.querySelector(".card-assembly.high")).toBeNull();
    });

    it("an expanded card's showing->idle exit keeps the expanded class until the swap completes", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();

      act(() => vi.advanceTimersByTime(174));
      expect(container.querySelector(".card-assembly.expanded")).not.toBeNull();

      act(() => vi.advanceTimersByTime(1));
      expect(container.querySelector(".card-assembly.expanded")).toBeNull();
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    });

    it("showing->idle exit adds `.exiting` on the same render showing goes false, and drops it exactly when the swap settles", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.exiting")).toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} />);
      expect(container.querySelector(".card-assembly.exiting")).not.toBeNull();
      expect(container.querySelector(".card-assembly.high.exiting")).not.toBeNull();

      act(() => vi.advanceTimersByTime(174));
      expect(container.querySelector(".card-assembly.exiting")).not.toBeNull();

      act(() => vi.advanceTimersByTime(1));
      expect(container.querySelector(".card-assembly.exiting")).toBeNull();
      expect(container.querySelector(".card-assembly.idle")).not.toBeNull();
    });

    it("idle->showing promotion never applies `.exiting` (entrance stays untouched)", () => {
      const { container, rerender } = render(<StatusRailCard slot={{ state: "empty" }} />);

      rerender(<StatusRailCard slot={GOAL} />);
      expect(container.querySelector(".card-assembly.exiting")).toBeNull();

      act(() => vi.advanceTimersByTime(175));
      expect(container.querySelector(".card-assembly.exiting")).toBeNull();
    });

    it("notch resting mode: showing->idle exit carries `exiting exit-to-bare` (not `.bare`) for the whole window, then settles `.bare` with both exit classes gone", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} restingState="notch" />);

      rerender(<StatusRailCard slot={{ state: "empty" }} restingState="notch" />);
      expect(container.querySelector(".card-assembly.exiting.exit-to-bare")).not.toBeNull();
      expect(container.querySelector(".card-assembly.bare")).toBeNull();

      act(() => vi.advanceTimersByTime(174));
      expect(container.querySelector(".card-assembly.exiting.exit-to-bare")).not.toBeNull();
      expect(container.querySelector(".card-assembly.bare")).toBeNull();

      act(() => vi.advanceTimersByTime(1));
      expect(container.querySelector(".card-assembly.bare")).not.toBeNull();
      expect(container.querySelector(".card-assembly.exiting")).toBeNull();
      expect(container.querySelector(".card-assembly.exit-to-bare")).toBeNull();
    });

    it("rail resting mode: showing->idle exit never applies `exit-to-bare`, at any point in or after the exit window", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} restingState="rail" />);

      rerender(<StatusRailCard slot={{ state: "empty" }} restingState="rail" />);
      expect(container.querySelector(".card-assembly.exit-to-bare")).toBeNull();

      act(() => vi.advanceTimersByTime(175));
      expect(container.querySelector(".card-assembly.exit-to-bare")).toBeNull();
    });

    it("notch resting mode: a hovered showing->idle exit never applies `exit-to-bare`, at any point in or after the exit window", () => {
      const { container, rerender } = render(
        <StatusRailCard slot={GOAL} restingState="notch" hovered={true} />,
      );

      rerender(<StatusRailCard slot={{ state: "empty" }} restingState="notch" hovered={true} />);
      expect(container.querySelector(".card-assembly.exiting")).not.toBeNull();
      expect(container.querySelector(".card-assembly.exit-to-bare")).toBeNull();

      act(() => vi.advanceTimersByTime(174));
      expect(container.querySelector(".card-assembly.exit-to-bare")).toBeNull();

      act(() => vi.advanceTimersByTime(1));
      expect(container.querySelector(".card-assembly.bare")).not.toBeNull();
      expect(container.querySelector(".card-assembly.exit-to-bare")).toBeNull();
    });

    it("notch resting mode: a hover arriving mid-window drops `exit-to-bare` immediately", () => {
      const { container, rerender } = render(<StatusRailCard slot={GOAL} restingState="notch" />);

      rerender(<StatusRailCard slot={{ state: "empty" }} restingState="notch" />);
      expect(container.querySelector(".card-assembly.exit-to-bare")).not.toBeNull();

      act(() => vi.advanceTimersByTime(100));
      expect(container.querySelector(".card-assembly.exit-to-bare")).not.toBeNull();

      rerender(<StatusRailCard slot={{ state: "empty" }} restingState="notch" hovered={true} />);
      expect(container.querySelector(".card-assembly.exit-to-bare")).toBeNull();
      expect(container.querySelector(".card-assembly.exiting")).not.toBeNull();
    });
  });
});

function readSourceCss(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  const raw = readFileSync(fileURLToPath(url), "utf-8");
  return raw.replace(/^@import\s+["'](\.[^"']+)["'];\s*$/gm, (_match, importPath: string) =>
    readFileSync(fileURLToPath(new NodeURL(importPath, url)), "utf-8"),
  );
}

function ruleBody(css: string, selector: string): string {
  const pattern = selector
    .split(/\s+/)
    .map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("\\s+");
  const match = css.match(new RegExp(`${pattern}\\s*\\{`));
  if (!match || match.index === undefined) {
    throw new Error(`selector not found in stylesheet: ${selector}`);
  }
  const braceStart = match.index + match[0].length - 1;
  let i = braceStart + 1;
  let inComment = false;
  while (i < css.length) {
    if (!inComment && css.startsWith("/*", i)) {
      inComment = true;
      i += 2;
      continue;
    }
    if (inComment && css.startsWith("*/", i)) {
      inComment = false;
      i += 2;
      continue;
    }
    if (!inComment && css[i] === "}") {
      return css.slice(braceStart + 1, i);
    }
    i++;
  }
  throw new Error(`unterminated rule for selector: ${selector}`);
}

function propValue(body: string, prop: string): string {
  const escaped = prop.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = body.match(new RegExp(`${escaped}\\s*:\\s*([^;]+);`));
  if (!match) {
    throw new Error(`property not found: ${prop}`);
  }
  return match[1].trim();
}

describe("exit-to-bare CSS convergence invariant", () => {
  const overlayCardCss = readSourceCss("../overlay-card.css");

  const exitShellBody = ruleBody(overlayCardCss, ".card-root .card-assembly.exiting.exit-to-bare");
  const bareShellBody = ruleBody(overlayCardCss, ".card-root .card-assembly.bare");

  const exitFlankBody = ruleBody(
    overlayCardCss,
    ".card-root .card-assembly.exiting.exit-to-bare .flank-left, .card-root .card-assembly.exiting.exit-to-bare .flank-right",
  );
  const bareFlankBody = ruleBody(
    overlayCardCss,
    ".card-root .card-assembly.bare .flank-left, .card-root .card-assembly.bare .flank-right",
  );

  const exitCutoutBody = ruleBody(
    overlayCardCss,
    ':root[data-notchtap-mode="hud"] .card-root .card-assembly.exiting.exit-to-bare .synthetic-cutout',
  );
  const bareCutoutBody = ruleBody(
    overlayCardCss,
    ':root[data-notchtap-mode="hud"] .card-root .card-assembly.bare:not(:has(.below-block)) .synthetic-cutout',
  );

  it("--cw: the exit-to-bare shell converges on `.bare`'s own value", () => {
    expect(propValue(exitShellBody, "--cw")).toBe(propValue(bareShellBody, "--cw"));
  });

  it("flank background: the exit-to-bare animation's end state matches `.bare`'s (both transparent)", () => {
    expect(propValue(exitFlankBody, "background-color")).toBe(
      propValue(bareFlankBody, "background"),
    );
  });

  it("flank padding: the exit-to-bare animation's end state matches `.bare`'s (both zero)", () => {
    expect(propValue(exitFlankBody, "padding")).toBe(propValue(bareFlankBody, "padding"));
  });

  it("synthetic-cutout bottom-left radius: the exit-to-bare animation's end state matches `.bare`'s", () => {
    expect(propValue(exitCutoutBody, "border-bottom-left-radius")).toBe(
      propValue(bareCutoutBody, "border-bottom-left-radius"),
    );
  });

  it("synthetic-cutout bottom-right radius: the exit-to-bare animation's end state matches `.bare`'s", () => {
    expect(propValue(exitCutoutBody, "border-bottom-right-radius")).toBe(
      propValue(bareCutoutBody, "border-bottom-right-radius"),
    );
  });

});

describe("tab-notch integration", () => {
  afterEach(cleanup);

  const QUIET: StatusState = {
    paused: false,
    waiting: 0,
    agent: { activeSessions: 0 },
    football: { enabled: false, live: null },
    news: { enabled: false, chargeFraction: 0, chargeCount: 0, isCharged: false },
  };

  const LIVE_MATCH_STATUS: StatusState = {
    ...QUIET,
    football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
  };

  function agentSession(overrides: Partial<AgentSessionView> = {}): AgentSessionView {
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

  describe("the icon strip's mount gate", () => {
    it("mounts inside the right flank while idle, so the flank's own hover paint is what reveals it", () => {
      const { container } = render(<StatusRailCard slot={{ state: "empty" }} status={QUIET} />);
      expect(container.querySelector(".flank-right .icon-strip")).not.toBeNull();
    });

    it("never mounts while a card is showing, hovered or not (a pushed card is unaffected by tab selection)", () => {
      const { container } = render(<StatusRailCard slot={GOAL} status={QUIET} hovered={true} />);
      expect(container.querySelector(".icon-strip")).toBeNull();
    });

  });

  describe("the selection-driven below-block swap", () => {
    function hoveredIdle(
      selectedTab: "agent" | "football" | "news" | null,
      status: StatusState,
      sessions: AgentSessionView[] = [],
    ) {
      return render(
        <StatusRailCard
          slot={{ state: "empty" }}
          status={status}
          hovered={true}
          selectedTab={selectedTab}
          agentSessions={sessions}
          agentCapturedAtMs={1_000_000}
        />,
      );
    }

    it("with nothing selected, keeps the ambient peek", () => {
      const { container } = hoveredIdle(null, QUIET);
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
      expect(container.querySelector('[data-testid="agent-below-block"]')).toBeNull();
    });

    it("selecting agent with no sessions degrades to the ambient peek, not to a blank shell", () => {
      const { container } = hoveredIdle("agent", QUIET, []);
      expect(container.querySelector('[data-testid="agent-below-block"]')).toBeNull();
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
      expect(container.querySelectorAll(".below-block").length).toBe(1);
    });

    it("selecting news keeps the ambient peek, since no story wire exists to fill the block", () => {
      const { container } = hoveredIdle("news", {
        ...QUIET,
        news: { enabled: true, chargeFraction: 1, chargeCount: 3, isCharged: true },
      });
      expect(container.querySelector(".below-block.idle-peek")).not.toBeNull();
      expect(container.querySelectorAll(".below-block").length).toBe(1);
    });

    it("selecting agent with live sessions mounts the agent below-block and closes the peek", () => {
      const { container } = hoveredIdle("agent", { ...QUIET, agent: { activeSessions: 1 } }, [
        agentSession(),
      ]);
      expect(container.querySelector('[data-testid="agent-below-block"]')).not.toBeNull();
      expect(container.querySelector(".below-block.idle-peek")).toBeNull();
      expect(container.querySelectorAll(".below-block").length).toBe(1);
    });

    it("threads viewedSessionIndex through to the below-block, changing which session's hero renders", () => {
      const sessions = [
        agentSession({ project: { name: "alpha-repo", cwd: null } }),
        agentSession({ project: { name: "beta-repo", cwd: null } }),
      ];
      const status: StatusState = { ...QUIET, agent: { activeSessions: 2 } };

      const { container, rerender } = render(
        <StatusRailCard
          slot={{ state: "empty" }}
          status={status}
          hovered={true}
          selectedTab="agent"
          agentSessions={sessions}
          agentCapturedAtMs={1_000_000}
          viewedSessionIndex={0}
        />,
      );
      expect(screen.getByText(/alpha-repo/)).toBeTruthy();
      expect(screen.queryByText(/beta-repo/)).toBeNull();
      const segmentsAtZero = container.querySelectorAll(".ttl-bar > *");
      expect(segmentsAtZero[0]?.className).toContain("ttl-fill");
      expect(segmentsAtZero[1]?.className).not.toContain("ttl-fill");

      rerender(
        <StatusRailCard
          slot={{ state: "empty" }}
          status={status}
          hovered={true}
          selectedTab="agent"
          agentSessions={sessions}
          agentCapturedAtMs={1_000_000}
          viewedSessionIndex={1}
        />,
      );
      expect(screen.getByText(/beta-repo/)).toBeTruthy();
      expect(screen.queryByText(/alpha-repo/)).toBeNull();
      const segmentsAtOne = container.querySelectorAll(".ttl-bar > *");
      expect(segmentsAtOne[1]?.className).toContain("ttl-fill");
      expect(segmentsAtOne[0]?.className).not.toContain("ttl-fill");
    });

    it("selecting football shows the shipped scorecard reveal", () => {
      const { container } = hoveredIdle("football", LIVE_MATCH_STATUS);
      expect(container.querySelector(".idle-reveal-scorecard")).not.toBeNull();
    });

    it("never mounts more than one below-block at a time (the rounding law depends on it)", () => {
      const { container } = hoveredIdle("agent", { ...QUIET, agent: { activeSessions: 1 } }, [
        agentSession(),
      ]);
      expect(container.querySelectorAll(".below-block").length).toBe(1);
    });

    it("mounts no tab below-block while un-hovered, however the selection stands", () => {
      const { container } = render(
        <StatusRailCard
          slot={{ state: "empty" }}
          status={{ ...QUIET, agent: { activeSessions: 1 } }}
          hovered={false}
          selectedTab="agent"
          agentSessions={[agentSession()]}
          agentCapturedAtMs={1_000_000}
        />,
      );
      expect(container.querySelector(".below-block")).toBeNull();
    });

    it("mounts the pulled card in a placed wrapper that is a DIRECT child of the shell", () => {
      const { container } = hoveredIdle("agent", { ...QUIET, agent: { activeSessions: 1 } }, [
        agentSession(),
      ]);
      const assembly = container.querySelector(".card-assembly");
      const slot = container.querySelector(".tab-below-slot");
      expect(assembly).not.toBeNull();
      expect(slot).not.toBeNull();
      expect(slot?.parentElement).toBe(assembly);
      const block = container.querySelector('[data-testid="agent-below-block"]');
      expect(block).not.toBeNull();
      expect(block?.parentElement).toBe(slot);
      expect(block?.className).toContain("below-block");
    });

    it("leaves the ambient peek's own already-direct mount alone — it needs no wrapper", () => {
      const { container } = hoveredIdle(null, QUIET);
      const peek = container.querySelector(".below-block.idle-peek");
      expect(peek?.parentElement).toBe(container.querySelector(".card-assembly"));
      expect(container.querySelector(".tab-below-slot")).toBeNull();
    });

    it("the wrapper's class resolves to the same grid cell every other below-block occupies", () => {
      const css = readSourceCss("../overlay-card.css");
      const slotRule = ruleBody(css, ".card-root .tab-below-slot");
      const blockRule = ruleBody(css, ".card-root .below-block");
      expect(propValue(slotRule, "grid-column")).toBe(propValue(blockRule, "grid-column"));
      expect(propValue(slotRule, "grid-row")).toBe(propValue(blockRule, "grid-row"));
      const declaredProps = slotRule
        .split(";")
        .map((declaration) => declaration.split(":")[0].trim())
        .filter((prop) => prop.length > 0);
      expect(declaredProps).toEqual([
        "grid-column",
        "grid-row",
        "position",
        "box-sizing",
        "width",
        "min-width",
      ]);
    });

    it("the bare shell re-widens for ANY hover-mounted block, not just the ambient peek", () => {
      const css = readSourceCss("../overlay-card.css");
      const bareRevealed = ruleBody(css, ".card-root .card-assembly.bare:has(.below-block)");
      expect(propValue(bareRevealed, "--cw")).toContain("--present-icons");
      expect(() => ruleBody(css, ".card-root .card-assembly.bare:has(.idle-peek)")).toThrow(
        /selector not found/,
      );
    });

    it("a showing card renders its own content, untouched, whatever is selected", () => {
      const { container } = render(
        <StatusRailCard
          slot={GOAL}
          status={{ ...QUIET, agent: { activeSessions: 1 } }}
          hovered={true}
          selectedTab="agent"
          agentSessions={[agentSession()]}
          agentCapturedAtMs={1_000_000}
        />,
      );
      expect(container.querySelector('[data-testid="agent-below-block"]')).toBeNull();
      expect(screen.getByText("GOAL")).toBeTruthy();
    });
  });
});
