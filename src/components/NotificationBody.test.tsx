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

function digits(container: HTMLElement): Element[] {
  return Array.from(container.querySelectorAll(".score-digit"));
}

function rollsIn(container: HTMLElement, side: 0 | 1): Element[] {
  return Array.from(digits(container)[side].querySelectorAll(".score-digit-roll"));
}

function rolls(container: HTMLElement): (Element | undefined)[] {
  return [rollsIn(container, 0)[0], rollsIn(container, 1)[0]];
}

describe("FootballHeroCard score odometer", () => {
  it("renders each side's score inside its own clip, reading the same as before", () => {
    const { container } = render(card());
    expect(container.querySelector(".score")?.textContent).toBe("1–1");
    expect(digits(container)).toHaveLength(2);
    expect(rolls(container)[0]?.textContent).toBe("1");
    expect(rolls(container)[1]?.textContent).toBe("1");
  });

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

    const home = rollsIn(container, 0);
    expect(home).toHaveLength(2);
    expect(home).toContain(before[0]);
    expect(home.map((span) => span.textContent)).toContain("2");

    expect(rollsIn(container, 1)).toHaveLength(1);
    expect(rolls(container)[1]).toBe(before[1]);

    expect(digits(container)[0]).toBe(clipsBefore[0]);
    expect(digits(container)[1]).toBe(clipsBefore[1]);
  });

  it("clips the roll: fixed one-line-box height with overflow hidden", () => {
    const body = ruleBody(overlayCardCss, ".card-root .score-digit");
    expect(body).toContain("overflow: hidden;");
    expect(body).toContain("height: 1em;");
    expect(body).toContain("position: relative;");
  });
});

describe("FootballHeroCard match-state chip", () => {
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
    expect(finalDot).toContain("width: 0;");
    expect(finalDot).toContain("margin-right: -5px;");
    expect(ruleBody(overlayCardCss, ".card-root .chip-live")).toContain("gap: 5px;");
  });

  it("ruleBody throws on a selector that doesn't exist — no vacuous pass", () => {
    expect(() => ruleBody(overlayCardCss, ".card-root .no-such-selector")).toThrow();
  });
});

describe("FootballHeroCard crossbar variant", () => {
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
    // SAFETY: a `secondaryMatches` fixture makes FootballHeroCard render the
    // `.score-block.stacked` wrapper, so the match is non-null.
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
    // SAFETY: the `secondaryMatches` fixture renders the `.score-block.stacked`
    // wrapper, so the match is non-null.
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

describe("fact pills: tags and tones", () => {
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
