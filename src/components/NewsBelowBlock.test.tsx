import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NewsBelowBlock, type NewsStoryView } from "./NewsBelowBlock";

afterEach(cleanup);

function story(overrides: Partial<NewsStoryView> = {}): NewsStoryView {
  return {
    source: "The Verge",
    headline: "Apple opens the notch region to third-party status items",
    summary:
      "The beta exposes a narrow API for persistent items either side of the camera housing.",
    category: "tech",
    age: "12m",
    priority: "low",
    hasLink: true,
    ...overrides,
  };
}

describe("NewsBelowBlock (plan 171, slice I)", () => {
  it("renders nothing when there are zero stories", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[]}
        currentIndex={0}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    expect(container.querySelector('[data-testid="news-below-block"]')).toBeNull();
    expect(container.firstChild).toBeNull();
  });

  it("carries the below-block/news-shade/category classes the shipped wash reads from", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story({ category: "tech" })]}
        currentIndex={0}
        freshCount={5}
        cycleEndedAgo="2m"
        expanded={false}
      />,
    );
    const block = container.querySelector('[data-testid="news-below-block"]');
    expect(block?.classList.contains("below-block")).toBe(true);
    expect(block?.classList.contains("news-shade")).toBe(true);
    expect(block?.classList.contains("cat-tech")).toBe(true);
  });

  it("falls back to the generic category class when a story carries no category", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story({ category: null })]}
        currentIndex={0}
        freshCount={1}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    const block = container.querySelector('[data-testid="news-below-block"]');
    expect(block?.classList.contains("cat-generic")).toBe(true);
  });

  it("mounts the batch header with the fresh count and cycle-ended-ago text", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story()]}
        currentIndex={0}
        freshCount={10}
        cycleEndedAgo="2m"
        expanded={false}
      />,
    );
    expect(container.querySelector(".batch-head")?.textContent).toContain("10 fresh");
    expect(container.querySelector(".batch-head")?.textContent).toContain("cycle ended 2m ago");
  });

  it("passes the batch header's nav callbacks straight through", () => {
    const onPrevious = vi.fn();
    const onNext = vi.fn();
    const { container } = render(
      <NewsBelowBlock
        stories={[story(), story()]}
        currentIndex={0}
        freshCount={2}
        cycleEndedAgo={null}
        expanded={false}
        onPrevious={onPrevious}
        onNext={onNext}
      />,
    );
    fireEvent.click(container.querySelector('[aria-label="previous story"]') as HTMLButtonElement);
    fireEvent.click(container.querySelector('[aria-label="next story"]') as HTMLButtonElement);
    expect(onPrevious).toHaveBeenCalledTimes(1);
    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it("renders the viewed story's own content, not necessarily stories[0]", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[
          story({ headline: "first story", source: "Wire A" }),
          story({ headline: "second story", source: "Wire B" }),
        ]}
        currentIndex={1}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    expect(container.querySelector(".title.headline")?.textContent).toBe("second story");
    expect(container.querySelector(".masthead")?.textContent).toContain("Wire B");
  });

  it("renders the category chip and relative age when present", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story({ category: "sports", age: "9m" })]}
        currentIndex={0}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    expect(container.querySelector(".chip-category")?.textContent).toBe("Sports");
    expect(container.querySelector(".notif-time-inline")?.textContent).toBe("9m");
  });

  it("omits the meta row entirely when a story has neither category nor age", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story({ category: null, age: null })]}
        currentIndex={0}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    expect(container.querySelector(".notif-meta-row")).toBeNull();
  });

  it("renders the real Wire stamp for a news story", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story()]}
        currentIndex={0}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    expect(container.querySelector(".stamp")?.textContent).toBe("Wire");
  });

  it("mounts the existing Manifest, toggling its expanded state from the expanded prop", () => {
    const { container, rerender } = render(
      <NewsBelowBlock
        stories={[story({ summary: "the full summary text" })]}
        currentIndex={0}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    let wrap = container.querySelector(".manifest-wrap");
    expect(wrap?.classList.contains("expanded")).toBe(false);
    expect(wrap?.getAttribute("aria-hidden")).toBe("true");
    expect(container.querySelector(".manifest-text")?.textContent).toBe("the full summary text");

    rerender(
      <NewsBelowBlock
        stories={[story({ summary: "the full summary text" })]}
        currentIndex={0}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={true}
      />,
    );
    wrap = container.querySelector(".manifest-wrap");
    expect(wrap?.classList.contains("expanded")).toBe(true);
    expect(wrap?.getAttribute("aria-hidden")).toBe("false");
  });

  it("feeds the position bar the full story count and the current index", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story(), story(), story()]}
        currentIndex={1}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    const segments = Array.from(container.querySelectorAll(".ttl-bar > *"));
    expect(segments).toHaveLength(3);
    expect(segments[1].className).toBe("ttl-fill");
  });

  it("clamps an out-of-range currentIndex instead of crashing on a stale index", () => {
    const { container } = render(
      <NewsBelowBlock
        stories={[story({ headline: "a" }), story({ headline: "b" })]}
        currentIndex={99}
        freshCount={0}
        cycleEndedAgo={null}
        expanded={false}
      />,
    );
    expect(container.querySelector('[data-testid="news-below-block"]')).not.toBeNull();
    expect(container.querySelector(".title.headline")?.textContent).toBe("b");
    const segments = Array.from(container.querySelectorAll(".ttl-bar > *"));
    expect(segments[1].className).toBe("ttl-fill");
  });
});
