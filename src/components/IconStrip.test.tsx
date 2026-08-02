import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { IconStrip, type IconStripProps, TAB_ORDER } from "./IconStrip";

afterEach(cleanup);

const BASE: IconStripProps = {
  agent: "hidden",
  football: "hidden",
  music: "hidden",
  weather: "present",
  news: "present",
  newsCharge: 0,
  newsCharged: false,
  newsCount: null,
  selected: null,
};

describe("IconStrip", () => {
  it("renders all five tabs, in the fixed strip order, regardless of presence", () => {
    const { container } = render(<IconStrip {...BASE} />);
    const buttons = container.querySelectorAll(".icon");
    expect(buttons).toHaveLength(5);
    expect(TAB_ORDER).toEqual(["agent", "football", "music", "weather", "news"]);
    // class list order mirrors TAB_ORDER -- the strip must never reorder
    // icons based on which are present, per the design source's "fixed
    // order" rule.
    const tabsInDom = Array.from(buttons).map((el) =>
      TAB_ORDER.find((tab) => el.classList.contains(tab)),
    );
    expect(tabsInDom).toEqual(TAB_ORDER);
  });

  it("a hidden icon carries no is-present/is-live class and is disabled", () => {
    const { container } = render(<IconStrip {...BASE} agent="hidden" />);
    const agentIcon = container.querySelector(".icon.agent") as HTMLButtonElement;
    expect(agentIcon.classList.contains("is-present")).toBe(false);
    expect(agentIcon.classList.contains("is-live")).toBe(false);
    expect(agentIcon.disabled).toBe(true);
  });

  it("present-but-not-live carries is-present without is-live", () => {
    const { container } = render(<IconStrip {...BASE} weather="present" />);
    const weatherIcon = container.querySelector(".icon.weather") as HTMLButtonElement;
    expect(weatherIcon.classList.contains("is-present")).toBe(true);
    expect(weatherIcon.classList.contains("is-live")).toBe(false);
    expect(weatherIcon.disabled).toBe(false);
  });

  it("live carries both is-present and is-live", () => {
    const { container } = render(<IconStrip {...BASE} agent="live" />);
    const agentIcon = container.querySelector(".icon.agent");
    expect(agentIcon?.classList.contains("is-present")).toBe(true);
    expect(agentIcon?.classList.contains("is-live")).toBe(true);
  });

  it("marks the selected tab is-selected and aria-pressed, and only that one", () => {
    const { container } = render(<IconStrip {...BASE} weather="present" selected="weather" />);
    const weatherIcon = container.querySelector(".icon.weather");
    const newsIcon = container.querySelector(".icon.news");
    expect(weatherIcon?.classList.contains("is-selected")).toBe(true);
    expect(weatherIcon?.getAttribute("aria-pressed")).toBe("true");
    expect(newsIcon?.classList.contains("is-selected")).toBe(false);
    expect(newsIcon?.getAttribute("aria-pressed")).toBe("false");
  });

  it("clicking a present icon fires onSelect with that tab", () => {
    const onSelect = vi.fn();
    render(<IconStrip {...BASE} weather="present" onSelect={onSelect} />);
    fireEvent.click(screen.getByRole("button", { name: "Weather" }));
    expect(onSelect).toHaveBeenCalledWith("weather");
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("clicking a hidden icon never fires onSelect (disabled buttons don't dispatch click)", () => {
    const onSelect = vi.fn();
    render(<IconStrip {...BASE} agent="hidden" onSelect={onSelect} />);
    fireEvent.click(screen.getByRole("button", { name: "Agent" }));
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("news carries is-charged only when newsCharged is true, independent of the news tier", () => {
    const { container: charged } = render(
      <IconStrip {...BASE} news="present" newsCharged={true} />,
    );
    expect(charged.querySelector(".icon.news")?.classList.contains("is-charged")).toBe(true);

    const { container: uncharged } = render(
      <IconStrip {...BASE} news="present" newsCharged={false} />,
    );
    expect(uncharged.querySelector(".icon.news")?.classList.contains("is-charged")).toBe(false);
  });

  it("renders the count badge only when newsCount is non-null", () => {
    const { container: withCount } = render(<IconStrip {...BASE} newsCount={7} />);
    expect(withCount.querySelector(".charge-count")?.textContent).toBe("7");

    const { container: withoutCount } = render(<IconStrip {...BASE} newsCount={null} />);
    expect(withoutCount.querySelector(".charge-count")).toBeNull();
  });

  it("news charge fill scaleY reflects newsCharge, clamped to [0,1]", () => {
    const { container: mid } = render(<IconStrip {...BASE} newsCharge={0.4} />);
    const midFill = mid.querySelector(".icon.news .charge") as SVGElement;
    expect(midFill.style.transform).toBe("scaleY(0.4)");

    const { container: over } = render(<IconStrip {...BASE} newsCharge={1.5} />);
    const overFill = over.querySelector(".icon.news .charge") as SVGElement;
    expect(overFill.style.transform).toBe("scaleY(1)");

    const { container: under } = render(<IconStrip {...BASE} newsCharge={-0.3} />);
    const underFill = under.querySelector(".icon.news .charge") as SVGElement;
    expect(underFill.style.transform).toBe("scaleY(0)");
  });

  // CodeRabbit review fix (PR #13): the charge rect used to be drawn at
  // y="15.5", entirely below the clipPath's own y=[2.5, 15.5] bounds —
  // the fill never overlapped the clip at ANY scaleY value, so it
  // rendered invisible at every charge level despite `scaleY` itself
  // computing correctly (which is why the test above never caught this —
  // it only asserts the transform value, never where the rect actually
  // sits). Pinning the geometry directly so a future edit can't
  // reintroduce the same silent-invisibility bug.
  it("the charge rect's own y coordinate matches the page outline's top (2.5), so scaleY(1) fills it exactly", () => {
    const { container } = render(<IconStrip {...BASE} />);
    const fill = container.querySelector(".icon.news .charge") as SVGElement;
    expect(fill.getAttribute("y")).toBe("2.5");
  });

  it("two IconStrips in the same document never collide on the news glyph's clip-path id", () => {
    const { container } = render(
      <div>
        <IconStrip {...BASE} />
        <IconStrip {...BASE} />
      </div>,
    );
    const clipIds = Array.from(container.querySelectorAll("clipPath")).map((el) => el.id);
    expect(clipIds).toHaveLength(2);
    expect(new Set(clipIds).size).toBe(2); // both non-empty and distinct
    // and each glyph's own charge rect references ITS sibling clipPath,
    // not the other instance's.
    const fills = container.querySelectorAll(".icon.news .charge");
    expect(fills[0].getAttribute("clip-path")).toBe(`url(#${clipIds[0]})`);
    expect(fills[1].getAttribute("clip-path")).toBe(`url(#${clipIds[1]})`);
  });

  it("every present icon has an accessible name matching its tab", () => {
    render(
      <IconStrip
        {...BASE}
        agent="live"
        football="live"
        music="live"
        weather="present"
        news="present"
      />,
    );
    for (const name of ["Agent", "Football", "Music", "Weather", "News"]) {
      expect(screen.getByRole("button", { name })).toBeTruthy();
    }
  });

  it("includes the pending count in the news tab's accessible name, since an aria-label overrides the visible badge for assistive tech (CodeRabbit review fix, PR #13)", () => {
    render(<IconStrip {...BASE} news="present" newsCount={3} />);
    expect(screen.getByRole("button", { name: "News, 3 new" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "News" })).toBeNull();
  });
});
