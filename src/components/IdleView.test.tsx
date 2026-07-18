import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { StatusState } from "../useStatusState";
import { IdleView } from "./IdleView";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers.
afterEach(cleanup);

// plan 034's three demo states: a live match evening, the "all clear"
// (espn off, rss on, nothing waiting), and the plain clock (every gate
// off, empty queue — no rail at all, narrow card).

const LIVE_EVENING: StatusState = {
  paused: false,
  waiting: 3,
  football: { enabled: true, live: { label: "Arsenal 2–0 Chelsea", minute: "45'" } },
  news: { enabled: true },
};

const ALL_CLEAR: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: true },
};

const EVERYTHING_OFF: StatusState = {
  paused: false,
  waiting: 0,
  football: { enabled: false, live: null },
  news: { enabled: false },
};

describe("IdleView status rail", () => {
  it("live evening: match chip with score and minute, News chip, N queued", () => {
    const { container } = render(<IdleView status={LIVE_EVENING} />);
    const liveChip = container.querySelector(".src-chip.live");
    expect(liveChip).not.toBeNull();
    expect(liveChip?.textContent).toContain("Arsenal 2–0 Chelsea");
    expect(liveChip?.textContent).toContain("45'");
    expect(liveChip?.querySelector(".live-dot")).not.toBeNull();
    expect(screen.getByText("News")).toBeTruthy();
    expect(screen.getByText("3 queued")).toBeTruthy();
  });

  it('all clear: News chip plus "clear", no live chip', () => {
    const { container } = render(<IdleView status={ALL_CLEAR} />);
    expect(container.querySelector(".src-chip.live")).toBeNull();
    expect(screen.getByText("News")).toBeTruthy();
    expect(screen.getByText("clear")).toBeTruthy();
  });

  it("plain clock: every gate off and nothing queued renders no rail", () => {
    const { container } = render(<IdleView status={EVERYTHING_OFF} />);
    expect(container.querySelector(".src-rail")).toBeNull();
    // the clock itself is untouched
    expect(container.querySelector(".idle-view .time")).not.toBeNull();
  });

  it("news gate off reads as a dimmed News paused chip", () => {
    const { container } = render(
      <IdleView
        status={{
          paused: false,
          waiting: 0,
          football: { enabled: true, live: null },
          news: { enabled: false },
        }}
      />,
    );
    const dimmed = container.querySelector(".src-chip.dim");
    expect(dimmed?.textContent).toBe("News paused");
  });
});
