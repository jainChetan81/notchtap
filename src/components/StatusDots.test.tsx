import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { StatusState } from "../useStatusState";
import { StatusDots } from "./StatusDots";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers.
afterEach(cleanup);

const ALL_ON: StatusState = {
  paused: false,
  waiting: 3,
  football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
  news: { enabled: true },
  weather: {
    enabled: true,
    current: { tempDisplay: "27°", condition: "Cloudy", isDay: true, rainPct: null },
  },
  media: { enabled: false, current: null },
};

const ALL_OFF: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
  weather: { enabled: false, current: null },
  media: { enabled: false, current: null },
};

describe("StatusDots", () => {
  it("renders exactly three dots in Football/News/Weather order", () => {
    const { container } = render(<StatusDots status={ALL_ON} />);
    const dots = container.querySelectorAll(".status-dot");
    expect(dots).toHaveLength(3);
    expect(dots[0].classList.contains("football")).toBe(true);
    expect(dots[1].classList.contains("news")).toBe(true);
    expect(dots[2].classList.contains("weather")).toBe(true);
  });

  it("marks every dot active when every source is enabled", () => {
    const { container } = render(<StatusDots status={ALL_ON} />);
    const dots = container.querySelectorAll(".status-dot");
    for (const dot of Array.from(dots)) {
      expect(dot.classList.contains("active")).toBe(true);
      expect(dot.classList.contains("dim")).toBe(false);
    }
  });

  it("marks every dot dim when every source is disabled", () => {
    const { container } = render(<StatusDots status={ALL_OFF} />);
    const dots = container.querySelectorAll(".status-dot");
    for (const dot of Array.from(dots)) {
      expect(dot.classList.contains("dim")).toBe(true);
      expect(dot.classList.contains("active")).toBe(false);
    }
  });

  it("reads each dot's active/dim state off its own source independently", () => {
    const { container } = render(
      <StatusDots
        status={{
          paused: false,
          waiting: 0,
          football: { enabled: true, live: null },
          news: { enabled: false },
          weather: { enabled: true, current: null },
          media: { enabled: false, current: null },
        }}
      />,
    );
    const [football, news, weather] = Array.from(container.querySelectorAll(".status-dot"));
    expect(football.classList.contains("active")).toBe(true);
    expect(news.classList.contains("dim")).toBe(true);
    expect(weather.classList.contains("active")).toBe(true);
  });

  // plan 034's live-match/weather-reading text is old rail furniture — the
  // dots carry no text content at all, only color/glow state.
  it("carries no text content (dots only, no labels)", () => {
    const { container } = render(<StatusDots status={ALL_ON} />);
    expect(container.textContent).toBe("");
  });

  // settings preview / older hosts render without a status prop at all —
  // every dot must fall back to dim rather than throwing.
  it("dims every dot when status is omitted", () => {
    const { container } = render(<StatusDots />);
    const dots = container.querySelectorAll(".status-dot");
    expect(dots).toHaveLength(3);
    for (const dot of Array.from(dots)) {
      expect(dot.classList.contains("dim")).toBe(true);
    }
  });

  // plan 092 (item 11): the paused indicator — every dot forces `dim`
  // (never `active`) while paused, even for sources that are otherwise
  // enabled, plus a static two-bar glyph renders beside the dot row.
  describe("paused (plan 092)", () => {
    it("forces every dot dim, even for otherwise-enabled sources, while paused", () => {
      const { container } = render(
        <StatusDots
          status={{
            paused: true,
            waiting: 0,
            football: { enabled: true, live: null },
            news: { enabled: true },
            weather: { enabled: true, current: null },
            media: { enabled: false, current: null },
          }}
        />,
      );
      const dots = container.querySelectorAll(".status-dot");
      for (const dot of Array.from(dots)) {
        expect(dot.classList.contains("dim")).toBe(true);
        expect(dot.classList.contains("active")).toBe(false);
      }
    });

    it("renders the pause glyph only while paused", () => {
      const { container, rerender } = render(<StatusDots status={ALL_ON} />);
      expect(container.querySelector(".pause-glyph")).toBeNull();

      rerender(<StatusDots status={{ ...ALL_ON, paused: true }} />);
      const glyph = container.querySelector(".pause-glyph");
      expect(glyph).not.toBeNull();
      // two CSS-drawn bars, no text content (receive-only indicator, not
      // a label).
      expect(glyph?.querySelectorAll("span")).toHaveLength(2);
      expect(container.textContent).toBe("");
    });

    it("omits the pause glyph when status is omitted (no data to say paused)", () => {
      const { container } = render(<StatusDots />);
      expect(container.querySelector(".pause-glyph")).toBeNull();
    });
  });

  // plan 110 (Step D): each dot is `role="img"` + a truthful `aria-label`,
  // and carries a non-color shape class independent of active/dim.
  describe("accessible names + non-color shapes (plan 110)", () => {
    it("names every dot 'enabled' and shapes it filled when every source is enabled", () => {
      render(<StatusDots status={ALL_ON} />);
      const football = screen.getByRole("img", { name: "Football — enabled" });
      const news = screen.getByRole("img", { name: "News — enabled" });
      const weather = screen.getByRole("img", { name: "Weather — enabled" });
      for (const dot of [football, news, weather]) {
        expect(dot.classList.contains("shape-enabled")).toBe(true);
      }
    });

    it("names every dot 'disabled' and shapes it hollow when every source is disabled", () => {
      render(<StatusDots status={ALL_OFF} />);
      const football = screen.getByRole("img", { name: "Football — disabled" });
      const news = screen.getByRole("img", { name: "News — disabled" });
      const weather = screen.getByRole("img", { name: "Weather — disabled" });
      for (const dot of [football, news, weather]) {
        expect(dot.classList.contains("shape-disabled")).toBe(true);
      }
    });

    it("names every dot 'status unavailable' and shapes it hollow-circle when status is omitted", () => {
      render(<StatusDots />);
      const football = screen.getByRole("img", { name: "Football — status unavailable" });
      const news = screen.getByRole("img", { name: "News — status unavailable" });
      const weather = screen.getByRole("img", { name: "Weather — status unavailable" });
      for (const dot of [football, news, weather]) {
        expect(dot.classList.contains("shape-unavailable")).toBe(true);
      }
    });

    it("reads each dot's name/shape off its own source independently", () => {
      render(
        <StatusDots
          status={{
            paused: false,
            waiting: 0,
            football: { enabled: true, live: null },
            news: { enabled: false },
            weather: { enabled: true, current: null },
            media: { enabled: false, current: null },
          }}
        />,
      );
      expect(screen.getByRole("img", { name: "Football — enabled" })).toBeTruthy();
      expect(screen.getByRole("img", { name: "News — disabled" })).toBeTruthy();
      expect(screen.getByRole("img", { name: "Weather — enabled" })).toBeTruthy();
    });

    // The core bug this plan fixes: the label must come from the RAW
    // config flag, never the pause-suppressed display booleans — while
    // paused, an otherwise-enabled source is still truthfully "enabled"
    // (dim luminance + its configured shape retained), and the pause
    // fact lives exclusively on the pause glyph, never on a dot's label.
    it("keeps a dot labeled 'enabled' (dim, configured shape retained) while paused — the pause fact lives only on the glyph", () => {
      const { container } = render(
        <StatusDots
          status={{
            paused: true,
            waiting: 0,
            football: { enabled: true, live: null },
            news: { enabled: false },
            weather: { enabled: true, current: null },
            media: { enabled: false, current: null },
          }}
        />,
      );
      const football = screen.getByRole("img", { name: "Football — enabled" });
      expect(football.classList.contains("dim")).toBe(true);
      expect(football.classList.contains("active")).toBe(false);
      expect(football.classList.contains("shape-enabled")).toBe(true);

      const glyph = screen.getByRole("img", { name: "Notifications paused" });
      expect(glyph.classList.contains("pause-glyph")).toBe(true);
      expect(container.querySelectorAll('[aria-label="Notifications paused"]')).toHaveLength(1);

      // no dot's accessible name ever mentions "paused" — that fact is
      // exclusively the glyph's.
      for (const dot of Array.from(container.querySelectorAll(".status-dot"))) {
        expect(dot.getAttribute("aria-label")?.toLowerCase()).not.toContain("paused");
      }
    });
  });
});
