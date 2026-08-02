import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { PositionBar, segmentFor } from "./PositionBar";

afterEach(cleanup);

function segs(container: HTMLElement) {
  return Array.from(container.querySelectorAll(".ttl-bar .ttl-seg"));
}

describe("segmentFor", () => {
  it("maps 1:1 when total is at or under the 10-segment ceiling", () => {
    expect(segmentFor(2, 5)).toEqual({ segmentCount: 5, segmentIndex: 2 });
  });

  it("caps the segment count at 10 for larger totals", () => {
    expect(segmentFor(0, 15).segmentCount).toBe(10);
  });

  it("maps the current index proportionally past the ceiling", () => {
    // total=20: each segment is 2 items. floor(current * 10 / total).
    expect(segmentFor(10, 20)).toEqual({ segmentCount: 10, segmentIndex: 5 });
    expect(segmentFor(19, 20)).toEqual({ segmentCount: 10, segmentIndex: 9 });
  });

  it("clamps a negative or out-of-range index into the segment span", () => {
    expect(segmentFor(-1, 5).segmentIndex).toBe(0);
    expect(segmentFor(99, 5).segmentIndex).toBe(4);
  });

  it("treats a non-positive total as a single segment", () => {
    expect(segmentFor(0, 0)).toEqual({ segmentCount: 1, segmentIndex: 0 });
  });
});

describe("PositionBar (plan 171, spec section 8)", () => {
  it("renders nothing when total is zero", () => {
    const { container } = render(<PositionBar total={0} current={0} />);
    expect(container.querySelector(".ttl-bar")).toBeNull();
  });

  it("renders one segment per item up to the 10-segment ceiling", () => {
    const { container } = render(<PositionBar total={5} current={2} />);
    // the viewed segment carries ttl-fill instead of ttl-seg, so the
    // plain-class query below finds the other 4.
    expect(segs(container)).toHaveLength(4);
    expect(container.querySelectorAll(".ttl-bar > *")).toHaveLength(5);
  });

  it("marks the viewed segment with ttl-fill and no others", () => {
    const { container } = render(<PositionBar total={5} current={2} />);
    const all = Array.from(container.querySelectorAll(".ttl-bar > *"));
    expect(all[2].className).toBe("ttl-fill");
    expect(all.filter((s) => s.classList.contains("ttl-fill"))).toHaveLength(1);
  });

  it("marks segments before the viewed one done, and after it as the bright plain default", () => {
    const { container } = render(<PositionBar total={5} current={2} />);
    const all = Array.from(container.querySelectorAll(".ttl-bar > *"));
    expect(all.slice(0, 2).every((s) => s.className === "ttl-seg done")).toBe(true);
    expect(all.slice(3).every((s) => s.className === "ttl-seg")).toBe(true);
  });

  it("hands the segment count to the grid via the --queue-n custom property", () => {
    const { container } = render(<PositionBar total={4} current={0} />);
    const bar = container.querySelector(".ttl-bar") as HTMLElement;
    expect(bar.style.getPropertyValue("--queue-n")).toBe("4");
  });

  it("renders exactly one segment (the fill) for a single-item total", () => {
    const { container } = render(<PositionBar total={1} current={0} />);
    const all = Array.from(container.querySelectorAll(".ttl-bar > *"));
    expect(all).toHaveLength(1);
    expect(all[0].className).toBe("ttl-fill");
  });

  it("caps rendered segments at 10 for a larger total", () => {
    const { container } = render(<PositionBar total={15} current={0} />);
    expect(container.querySelectorAll(".ttl-bar > *")).toHaveLength(10);
  });

  it("clamps an out-of-range current index to the last segment rather than dropping the fill", () => {
    const { container } = render(<PositionBar total={5} current={99} />);
    const all = Array.from(container.querySelectorAll(".ttl-bar > *"));
    expect(all[4].className).toBe("ttl-fill");
  });
});
