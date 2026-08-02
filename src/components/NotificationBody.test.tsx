// plan 151 (items A + B): the football scorecard's own motion contract —
// the match-state chip morphs rather than cuts, and the score digits roll
// on a goal (and ONLY on a goal). The component's rendering is otherwise
// covered end-to-end through StatusRailCard.test.tsx's "live-match
// football scorecard" block; this file exists for the two things that
// block can't express, which both need the component rendered DIRECTLY so
// a prop can be changed in place:
//   - DOM-identity assertions across a re-render (does a clock tick
//     remount the score spans?), and
//   - the CSS rules the animation actually lives in (jsdom has no layout
//     or transition engine, so those are pinned at the string level
//     against the real stylesheet — the same technique
//     celebrationStacking.test.tsx and IdleHoverPeek.test.tsx use).
//
// plan 170: this file used to be `LiveMatchScorecard.test.tsx`, direct-
// rendering the now-deleted `LiveMatchScorecard` component. That
// component's content moved into `FootballHeroCard` (this file's own
// `NotificationBody.tsx`), rendered through the shared masthead/stamp/
// accent-stripe template instead of a bespoke `.notif-block` layout — the
// odometer/chip-morph CSS assertions below carry over unchanged (those
// rules aren't moving, only which component renders the markup they
// target).
import { readFileSync } from "node:fs";
import { fileURLToPath, URL as NodeURL } from "node:url";
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { EspnMeta, SlotState } from "../useSlotState";
import { AgentHeroCard, type Fact, FootballHeroCard, NotificationBody } from "./NotificationBody";

afterEach(cleanup);

function readSourceCss(relativePath: string): string {
  const url = new NodeURL(relativePath, import.meta.url);
  const raw = readFileSync(fileURLToPath(url), "utf-8");
  return raw.replace(/^@import\s+["'](\.[^"']+)["'];\s*$/gm, (_match, importPath: string) =>
    readFileSync(fileURLToPath(new NodeURL(importPath, url)), "utf-8"),
  );
}

function ruleBody(css: string, selector: string): string {
  const marker = `${selector} {`;
  const start = css.indexOf(marker);
  if (start === -1) {
    throw new Error(`selector not found in stylesheet: ${selector}`);
  }
  const braceStart = start + marker.length - 1;
  const braceEnd = css.indexOf("}", braceStart);
  if (braceEnd === -1) {
    throw new Error(`unterminated rule for selector: ${selector}`);
  }
  return css.slice(braceStart + 1, braceEnd);
}

const overlayCardCss = readSourceCss("../overlay-card.css");

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

function card(espn: Partial<EspnMeta> = {}) {
  return (
    <FootballHeroCard
      title="Goal — K. Havertz 78'"
      priority="high"
      signal="goal"
      eventType="score_update"
      liveEspn={{ ...ESPN_BASE, ...espn }}
      pillVariant="live"
      pillLabel="Live"
      cardsClean={true}
    />
  );
}

/** The two odometer clips, home first. */
function digits(container: HTMLElement): Element[] {
  return Array.from(container.querySelectorAll(".score-digit"));
}

/** Every rolling span inside each clip — the span is keyed on the score
 * value, so its identity is what "did the odometer fire?" means, and its
 * COUNT is what "is a roll in flight?" means: mid-roll a clip briefly
 * holds two (the outgoing digit is kept mounted by AnimatePresence,
 * taken out of flow by `mode="popLayout"`, until its exit finishes). */
function rollsIn(container: HTMLElement, side: 0 | 1): Element[] {
  return Array.from(digits(container)[side].querySelectorAll(".score-digit-roll"));
}

/** The single settled roll span per side — only valid when no roll is in
 * flight, which every "nothing should have moved" assertion here asserts
 * first via the length check. */
function rolls(container: HTMLElement): (Element | undefined)[] {
  return [rollsIn(container, 0)[0], rollsIn(container, 1)[0]];
}

describe("FootballHeroCard score odometer (plan 151 item B)", () => {
  it("renders each side's score inside its own clip, reading the same as before", () => {
    const { container } = render(card());
    expect(container.querySelector(".score")?.textContent).toBe("1–1");
    expect(digits(container)).toHaveLength(2);
    expect(rolls(container)[0]?.textContent).toBe("1");
    expect(rolls(container)[1]?.textContent).toBe("1");
  });

  // THE restraint guard: this card re-renders once a minute purely to
  // move the clock pill on. If the clock tick remounted the score spans,
  // the digits would roll every minute of the match — the exact opposite
  // of "the payload moves when, and only when, the payload changes".
  it("a clock tick does not remount either score span (no roll without a goal)", () => {
    const { container, rerender } = render(card());
    const before = rolls(container);
    rerender(card({ clock: "79'" }));
    expect(container.querySelector(".clock-pill")?.textContent).toBe("79'");
    expect(rollsIn(container, 0)).toHaveLength(1);
    expect(rollsIn(container, 1)).toHaveLength(1);
    expect(rolls(container)[0]).toBe(before[0]);
    expect(rolls(container)[1]).toBe(before[1]);
  });

  // Same guard against a same-slot rotation re-emit: an identical
  // scoreline arriving again is not a goal. Value-keying covers this for
  // free, which is why the scorecard needs no `.rotation-swap`-style
  // off-switch (news-category.css).
  it("a re-emit carrying an unchanged scoreline does not remount either score span", () => {
    const { container, rerender } = render(card());
    const before = rolls(container);
    rerender(card({ homeCards: [1, 0] }));
    expect(rollsIn(container, 0)).toHaveLength(1);
    expect(rollsIn(container, 1)).toHaveLength(1);
    expect(rolls(container)[0]).toBe(before[0]);
    expect(rolls(container)[1]).toBe(before[1]);
  });

  it("a goal rolls ONLY the side that scored — the other digit holds still", () => {
    const { container, rerender } = render(card());
    const before = rolls(container);
    const clipsBefore = digits(container);
    rerender(card({ homeScore: 2 }));

    // the scoring side is mid-roll: the old "1" is still mounted (exiting)
    // and the new "2" has joined it inside the same clip.
    const home = rollsIn(container, 0);
    expect(home).toHaveLength(2);
    expect(home).toContain(before[0]);
    expect(home.map((span) => span.textContent)).toContain("2");

    // the other side never even re-mounted its span.
    expect(rollsIn(container, 1)).toHaveLength(1);
    expect(rolls(container)[1]).toBe(before[1]);

    // the clips themselves are stable containers — only their contents
    // change, so the row never re-lays-out around a goal.
    expect(digits(container)[0]).toBe(clipsBefore[0]);
    expect(digits(container)[1]).toBe(clipsBefore[1]);
  });

  it("clips the roll: fixed one-line-box height with overflow hidden", () => {
    const body = ruleBody(overlayCardCss, ".card-root .score-digit");
    expect(body).toContain("overflow: hidden;");
    expect(body).toContain("height: 1em;");
    // popLayout takes the outgoing digit out of flow — it can only land
    // back inside the clip if the clip is the positioned ancestor.
    expect(body).toContain("position: relative;");
  });
});

describe("FootballHeroCard match-state chip (plan 151 item A)", () => {
  it("keeps the live dot mounted in every variant, final included", () => {
    for (const variant of ["live", "break", "final"] as const) {
      const { container, unmount } = render(
        <FootballHeroCard
          title="full-time"
          priority="high"
          signal="fulltime"
          eventType="match_state"
          liveEspn={ESPN_BASE}
          pillVariant={variant}
          pillLabel={variant}
          cardsClean={true}
        />,
      );
      expect(container.querySelector(".chip-live .live-dot")).not.toBeNull();
      unmount();
    }
  });

  it("morphs the chip's colours over the reveal window instead of cutting", () => {
    const body = ruleBody(overlayCardCss, ".card-root .chip-live");
    expect(body).toContain("color var(--reveal-ms, 260ms) var(--ease-notchtap)");
    expect(body).toContain("background-color var(--reveal-ms, 260ms) var(--ease-notchtap)");
    expect(body).toContain("border-color var(--reveal-ms, 260ms) var(--ease-notchtap)");
  });

  it("fades and collapses the dot at full-time rather than blinking it out", () => {
    const dotBase = ruleBody(overlayCardCss, ".card-root .chip-live .live-dot");
    expect(dotBase).toContain("opacity var(--reveal-ms, 260ms) var(--ease-notchtap)");
    const finalDot = ruleBody(overlayCardCss, ".card-root .chip-live.final .live-dot");
    expect(finalDot).toContain("opacity: 0;");
    // the collapse cancels the chip's own 5px gap exactly, so the label
    // lands where a dot-less chip would have put it.
    expect(finalDot).toContain("width: 0;");
    expect(finalDot).toContain("margin-right: -5px;");
    expect(ruleBody(overlayCardCss, ".card-root .chip-live")).toContain("gap: 5px;");
  });

  it("ruleBody throws on a selector that doesn't exist — no vacuous pass", () => {
    expect(() => ruleBody(overlayCardCss, ".card-root .no-such-selector")).toThrow();
  });
});

// Plan 171 (tab-notch redesign, slice G): the crossbar persistent
// variant — a second, stacked score-block for a secondary live match.
describe("FootballHeroCard crossbar variant (plan 171, slice G)", () => {
  const SECOND_ESPN: EspnMeta = {
    league: "EPL",
    homeAbbrev: "MCI",
    awayAbbrev: "LIV",
    homeScore: 0,
    awayScore: 0,
    clock: "HT",
    homeCards: [0, 0],
    awayCards: [0, 0],
    homeCrest: null,
    awayCrest: null,
  };

  it("renders exactly one score-block when secondaryMatches is omitted (byte-identical to before this slice)", () => {
    const { container } = render(card());
    expect(container.querySelectorAll(".score-block")).toHaveLength(1);
    expect(container.querySelector(".score-block.stacked")).toBeNull();
  });

  it("renders a second, stacked score-block for a secondary match", () => {
    const { container } = render(
      <FootballHeroCard
        title="2 matches live"
        priority="high"
        signal="goal"
        eventType="score_update"
        liveEspn={ESPN_BASE}
        pillVariant="live"
        pillLabel="Live"
        cardsClean={true}
        secondaryMatches={[
          { liveEspn: SECOND_ESPN, pillVariant: "break", pillLabel: "Break", cardsClean: true },
        ]}
      />,
    );
    const blocks = container.querySelectorAll(".score-block");
    expect(blocks).toHaveLength(2);
    expect(blocks[0].classList.contains("stacked")).toBe(false);
    expect(blocks[1].classList.contains("stacked")).toBe(true);
  });

  it("renders the secondary match's own league, pill label, clock, and score", () => {
    const { container } = render(
      <FootballHeroCard
        title="2 matches live"
        priority="high"
        signal="goal"
        eventType="score_update"
        liveEspn={ESPN_BASE}
        pillVariant="live"
        pillLabel="Live"
        cardsClean={true}
        secondaryMatches={[
          { liveEspn: SECOND_ESPN, pillVariant: "break", pillLabel: "Break", cardsClean: true },
        ]}
      />,
    );
    const stacked = container.querySelector(".score-block.stacked") as HTMLElement;
    expect(stacked.querySelector(".chip-league")?.textContent).toBe("EPL");
    expect(stacked.querySelector(".chip-live")?.classList.contains("break")).toBe(true);
    expect(stacked.querySelector(".clock-pill")?.textContent).toBe("HT");
    expect(stacked.querySelectorAll(".side")).toHaveLength(2);
  });

  it("still suppresses the primary block's title-headline behaviour not at all — title stays a plain caller-controlled string", () => {
    const { container } = render(
      <FootballHeroCard
        title="2 matches live"
        priority="high"
        signal="goal"
        eventType="score_update"
        liveEspn={ESPN_BASE}
        pillVariant="live"
        pillLabel="Live"
        cardsClean={true}
        secondaryMatches={[
          { liveEspn: SECOND_ESPN, pillVariant: "break", pillLabel: "Break", cardsClean: true },
        ]}
      />,
    );
    expect(container.querySelector(".title.headline")?.textContent).toBe("2 matches live");
  });

  it("shows the secondary match's own cards-line only when it has cards, independent of the primary's cardsClean", () => {
    const { container } = render(
      <FootballHeroCard
        title="2 matches live"
        priority="high"
        signal="goal"
        eventType="score_update"
        liveEspn={ESPN_BASE}
        pillVariant="live"
        pillLabel="Live"
        cardsClean={true}
        secondaryMatches={[
          {
            liveEspn: { ...SECOND_ESPN, homeCards: [1, 0] },
            pillVariant: "break",
            pillLabel: "Break",
            cardsClean: false,
          },
        ]}
      />,
    );
    const stacked = container.querySelector(".score-block.stacked") as HTMLElement;
    expect(stacked.querySelector(".cards-line")).not.toBeNull();
    expect(container.querySelector(".score-block:not(.stacked) .cards-line")).toBeNull();
  });

  it("renders multiple secondary matches with a stable, non-index key (no React key warning) using league/team identity", () => {
    const THIRD_ESPN: EspnMeta = {
      ...SECOND_ESPN,
      league: "LaLiga",
      homeAbbrev: "RMA",
      awayAbbrev: "BAR",
    };
    const { container } = render(
      <FootballHeroCard
        title="3 matches live"
        priority="high"
        signal="goal"
        eventType="score_update"
        liveEspn={ESPN_BASE}
        pillVariant="live"
        pillLabel="Live"
        cardsClean={true}
        secondaryMatches={[
          { liveEspn: SECOND_ESPN, pillVariant: "break", pillLabel: "Break", cardsClean: true },
          { liveEspn: THIRD_ESPN, pillVariant: "live", pillLabel: "Live", cardsClean: true },
        ]}
      />,
    );
    expect(container.querySelectorAll(".score-block.stacked")).toHaveLength(2);
  });
});

// Plan 169 fidelity pass (2026-08-02): the shared fact-pill renderer's
// two knobs — the optional `.fp-tag` qualifier and the per-call tone —
// asserted directly on the two components that pass them differently.
// The mock (`prototype/agent-board.html`, proposal section) gives the
// agent hero a toned pill in every state and a coloured tag on the two
// alarm states; a generic (non-agent) card's pills stay neutral, which
// is the contrast this block pins so the two can't silently converge.
describe("fact pills: tags and tones (plan 169 fidelity pass)", () => {
  function heroWith(facts: Fact[], factsTone: "accent" | "danger" | "safe") {
    return (
      <AgentHeroCard
        dotKey="working"
        pulse={false}
        title="Agent working"
        subtitle="Codex · notchtap"
        body={null}
        priority="medium"
        facts={facts}
        factsTone={factsTone}
      />
    );
  }

  it("renders a tagged fact as label + value + `.fp-tag`, in that order", () => {
    const { container } = render(
      heroWith(
        [{ label: "Tool", value: "rm", tag: { text: "destructive", tone: "danger" } }],
        "danger",
      ),
    );
    const pill = container.querySelector(".fact-pill");
    expect(pill?.querySelector(".fp-label")?.textContent).toBe("Tool");
    expect(pill?.querySelector(".fp-tag")?.textContent).toBe("destructive");
    expect(pill?.textContent).toBe("Toolrmdestructive");
  });

  it("omits `.fp-tag` entirely for an untagged fact — never an empty span", () => {
    const { container } = render(heroWith([{ label: "Progress", value: "63%" }], "accent"));
    expect(container.querySelector(".fact-pill .fp-tag")).toBeNull();
  });

  it("a tagged fact's own tone wins over the call-level tone", () => {
    const { container } = render(
      heroWith(
        [
          { label: "Progress", value: "63%" },
          { label: "Exit", value: "1", tag: { text: "error", tone: "danger" } },
        ],
        "accent",
      ),
    );
    const pills = container.querySelectorAll(".fact-pill");
    expect(pills[0].classList.contains("tone-accent")).toBe(true);
    expect(pills[1].classList.contains("tone-danger")).toBe(true);
    expect(pills[1].classList.contains("tone-accent")).toBe(false);
  });

  // The other half of the contrast: the generic branch passes no tone at
  // all, so a manual/CLI card's pills stay the plain neutral pill.
  it("a generic card's pills carry no tone class", () => {
    const slot: Extract<SlotState, { state: "showing" }> = {
      state: "showing",
      id: "n1",
      title: "Build finished",
      body: "All green",
      eventType: "generic",
      priority: "medium",
      signal: "generic",
      origin: "manual",
      agentRuntime: null,
      expanded: false,
      source: null,
      category: null,
      publishedAtMs: null,
      link: null,
      subtitle: null,
      details: [{ label: "Tool", value: "Bash" }],
      queueTotal: 1,
      queueDone: 0,
      ttlMs: 8000,
      remainingMs: 8000,
    };
    const { container } = render(
      <NotificationBody
        news={false}
        slot={slot}
        newsCategory={null}
        newsAge={null}
        bodyContent={slot.body}
        expanded={false}
        liveVisibleDetails={slot.details}
        hovered={false}
      />,
    );
    const pill = container.querySelector(".fact-pill");
    expect(pill).not.toBeNull();
    expect(pill?.className).toBe("fact-pill");
    expect(pill?.querySelector(".fp-tag")).toBeNull();
  });
});
