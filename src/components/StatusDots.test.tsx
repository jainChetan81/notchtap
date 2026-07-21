import { cleanup, render } from "@testing-library/react";
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
  weather: { enabled: true, current: { tempDisplay: "27°", condition: "Cloudy" } },
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
});
