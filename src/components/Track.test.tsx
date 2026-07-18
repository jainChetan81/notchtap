import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { Track } from "./Track";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers —
// without this, DOM from one test's render leaks into the next.
afterEach(cleanup);

function spans(container: HTMLElement) {
  return Array.from(container.querySelectorAll(".track span"));
}

describe("Track (queue slider, plan 033)", () => {
  it("renders one segment per batch item when the batch is at most 10", () => {
    const { container } = render(<Track total={5} done={2} />);
    const all = spans(container);
    expect(all).toHaveLength(5);
    expect(all.slice(0, 2).every((s) => s.classList.contains("done"))).toBe(true);
    expect(all[2].classList.contains("cur")).toBe(true);
    expect(all.slice(3).every((s) => s.className === "")).toBe(true);
  });

  it("lights only the current segment for a single-item batch", () => {
    const { container } = render(<Track total={1} done={0} />);
    const all = spans(container);
    expect(all).toHaveLength(1);
    expect(all[0].classList.contains("cur")).toBe(true);
    expect(all[0].classList.contains("done")).toBe(false);
  });

  it("clamps the segment count to at least one even for a degenerate total", () => {
    const { container } = render(<Track total={0} done={0} />);
    expect(spans(container)).toHaveLength(1);
  });

  it("caps the segment count at 10 for batches beyond the ceiling", () => {
    const { container } = render(<Track total={15} done={0} />);
    const all = spans(container);
    expect(all).toHaveLength(10);
    expect(all[0].classList.contains("cur")).toBe(true);
    expect(container.querySelectorAll(".track span.done")).toHaveLength(0);
  });

  it("maps the index proportionally past the 10-segment ceiling", () => {
    // total=20: each segment is 2 items. floor(done * 10 / total).
    const mid = render(<Track total={20} done={10} />);
    const midSpans = spans(mid.container);
    expect(midSpans).toHaveLength(10);
    expect(midSpans.slice(0, 5).every((s) => s.classList.contains("done"))).toBe(true);
    expect(midSpans[5].classList.contains("cur")).toBe(true);

    // the last item of the batch lights the final segment (floor(19*10/20)=9)
    const last = render(<Track total={20} done={19} />);
    const lastSpans = spans(last.container);
    expect(lastSpans.slice(0, 9).every((s) => s.classList.contains("done"))).toBe(true);
    expect(lastSpans[9].classList.contains("cur")).toBe(true);
  });

  it("hands the segment count to the grid via the --queue-n custom property", () => {
    const { container } = render(<Track total={4} done={0} />);
    const track = container.querySelector(".track") as HTMLElement;
    expect(track.style.getPropertyValue("--queue-n")).toBe("4");
  });
});
