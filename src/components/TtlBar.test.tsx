import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TtlBar } from "./TtlBar";

// this project's vitest config doesn't set `test.globals`, so RTL's
// auto-cleanup (which hooks a global `afterEach`) never registers —
// without this, DOM from one test's render leaks into the next.
afterEach(cleanup);

// TtlBar.tsx animates `.ttl-fill` via `transform: scaleX(<fraction>)`,
// never by mutating `style.width` every frame — a layout property under
// `.card-assembly`'s `filter: drop-shadow` forces a re-layout and
// re-rasterize on every frame. This helper reads the scaleX fraction
// back out and reports it on the 0-100 percentage scale the assertions
// below expect, so each one reads as a percentage of remaining time.
function fillScalePercent(container: HTMLElement): number {
  // SAFETY: nullable cast because querySelector may miss; the very next
  // line's `not.toBeNull()` guards it before `fill` is dereferenced.
  const fill = container.querySelector(".ttl-fill") as HTMLElement | null;
  expect(fill).not.toBeNull();
  // SAFETY: `fill` is confirmed non-null one line above, so reading `.style`
  // through this narrowed cast cannot dereference null here.
  const transform = (fill as HTMLElement).style.transform;
  const match = transform.match(/^scaleX\(([\d.]+)\)$/);
  expect(match).not.toBeNull();
  return Number(match?.[1]) * 100;
}

describe("TtlBar", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance"] });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders the ttl-bar/ttl-fill DOM nodes", () => {
    const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} />);
    expect(container.querySelector(".ttl-bar")).not.toBeNull();
    expect(container.querySelector(".ttl-fill")).not.toBeNull();
  });

  it("anchors the fill to remainingMs/ttlMs and drains it over real time", () => {
    const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} />);

    // first animation frame: freshly anchored, ~50%.
    act(() => {
      vi.advanceTimersByTime(16);
    });
    const firstPct = fillScalePercent(container);
    expect(firstPct).toBeGreaterThan(0);
    expect(firstPct).toBeLessThanOrEqual(50);

    // advance real (faked) time by 2s: remaining drops to ~2000ms of 8000ms (~25%).
    act(() => {
      vi.advanceTimersByTime(2000);
    });
    const laterPct = fillScalePercent(container);
    expect(laterPct).toBeLessThan(firstPct);
    expect(laterPct).toBeCloseTo(25, 0);
  });

  it("clamps the fill at 0 once remainingMs has fully elapsed", () => {
    const { container } = render(<TtlBar slotId="n1" ttlMs={1000} remainingMs={500} />);
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(fillScalePercent(container)).toBe(0);
  });

  it("re-anchors the countdown when slotId changes (a new promotion)", () => {
    const { container, rerender } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={1000} />);
    // let n1 nearly drain.
    act(() => {
      vi.advanceTimersByTime(900);
    });
    expect(fillScalePercent(container)).toBeLessThan(15);

    // a new slot (new id) with a fresh full window must restart at ~100%,
    // not continue counting down from n1's near-zero remainder.
    rerender(<TtlBar slotId="n2" ttlMs={8000} remainingMs={8000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    expect(fillScalePercent(container)).toBeGreaterThan(90);
  });

  it("re-anchors on a same-id re-emit with a fresh remainingMs (supersede/extension)", () => {
    const { container, rerender } = render(<TtlBar slotId="n1" ttlMs={2000} remainingMs={100} />);
    act(() => {
      vi.advanceTimersByTime(90);
    });
    expect(fillScalePercent(container)).toBeLessThan(20);

    // same slotId, but a supersede top-up granted a fresh, larger window —
    // the bar must jump back up, not keep counting toward zero.
    rerender(<TtlBar slotId="n1" ttlMs={2000} remainingMs={2000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    expect(fillScalePercent(container)).toBeGreaterThan(90);
  });

  it("cancels the rAF loop on unmount", () => {
    const cancelSpy = vi.spyOn(window, "cancelAnimationFrame");
    const { unmount } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} />);
    act(() => {
      vi.advanceTimersByTime(16);
    });
    unmount();
    expect(cancelSpy).toHaveBeenCalled();
    // this project's vitest config sets neither `restoreMocks`
    // nor `clearMocks` (vite.config.ts), so an unrestored spy on a global
    // like `cancelAnimationFrame` silently outlives this test — the next
    // test's fake-timers instance (a fresh one per `beforeEach` above)
    // then calls through a spy still wrapping the TORN-DOWN previous
    // instance's fake `cancelAnimationFrame`, so a later mount's cleanup
    // silently fails to cancel its rAF loop. Harmless on its own, but
    // any later test in this file that mounts/unmounts TtlBar more than
    // once accumulates orphaned tick() loops from every prior mount,
    // which compounds into a genuine `RangeError: Maximum call stack
    // size exceeded` under `vi.advanceTimersByTime` — exactly the shape
    // of the hoverPaused tests below (multiple renders/rerenders in one
    // test).
    cancelSpy.mockRestore();
  });

  describe("hoverPaused", () => {
    // Freezing the fill alone reads as indistinguishable from a stall —
    // nothing visual marks the pause. `.paused` is the CSS hook
    // (ttl-bar.css) for the subtle dim/tint that marks it; pinned here
    // at the class-presence level since jsdom can't compute the resulting
    // computed style.
    it("marks the fill .paused exactly while hoverPaused is true", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />,
      );
      expect(container.querySelector(".ttl-fill")?.classList.contains("paused")).toBe(false);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      expect(container.querySelector(".ttl-fill")?.classList.contains("paused")).toBe(true);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />);
      expect(container.querySelector(".ttl-fill")?.classList.contains("paused")).toBe(false);
    });

    it("freezes the fill while hoverPaused is true", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />,
      );
      act(() => {
        vi.advanceTimersByTime(16);
      });
      const frozenAt = fillScalePercent(container);

      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(fillScalePercent(container)).toBe(frozenAt);

      // still frozen across a re-render with the same props too — not
      // just a one-shot skip.
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(fillScalePercent(container)).toBe(frozenAt);
    });

    it("resumes counting down from where it froze once hoverPaused clears, granting no extra time", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />,
      );
      // drain 2s of real active time.
      act(() => {
        vi.advanceTimersByTime(2000);
      });
      const beforePause = fillScalePercent(container);
      expect(beforePause).toBeCloseTo(75, 0);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      const duringPause = fillScalePercent(container);
      expect(duringPause).toBeCloseTo(beforePause, 0);

      // 5s spent hovering — must not count against the countdown at all.
      act(() => {
        vi.advanceTimersByTime(5000);
      });
      expect(fillScalePercent(container)).toBeCloseTo(beforePause, 0);

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      // resumes from ~75%, not from wherever an un-paused countdown would
      // have reached after the same 7s of real wall-clock time (~12.5%).
      expect(fillScalePercent(container)).toBeCloseTo(beforePause, 0);

      // and it keeps counting down normally from there.
      act(() => {
        vi.advanceTimersByTime(2000);
      });
      expect(fillScalePercent(container)).toBeCloseTo(50, 0);
    });

    it("does not reset the countdown when only hoverPaused toggles (no re-anchor)", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={false} />,
      );
      act(() => {
        vi.advanceTimersByTime(1000);
      });
      const before = fillScalePercent(container);

      // toggling hoverPaused true then immediately false again, with zero
      // time elapsed in between, must not perturb the reading at all —
      // proves this isn't secretly keyed into the anchoring effect.
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={true} />);
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={false} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      expect(fillScalePercent(container)).toBeCloseTo(before, 0);
    });

    // The bail-out behaviour itself — no new `requestAnimationFrame`
    // calls while hoverPaused, not merely a frozen visual value. Distinct from
    // "freezes the fill" above, which only pins the OUTPUT; this pins the
    // MECHANISM (spy on rAF itself) so a regression that keeps looping
    // but happens to keep painting the same number would still be
    // caught.
    it("stops requesting new animation frames while hoverPaused (no idle-CPU loop)", () => {
      const { rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={false} />,
      );
      act(() => {
        vi.advanceTimersByTime(100);
      });

      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} hoverPaused={true} />);
      act(() => {
        // let the in-flight frame (scheduled before the pause) fire and
        // bail out.
        vi.advanceTimersByTime(16);
      });

      const rafSpy = vi.spyOn(window, "requestAnimationFrame");
      act(() => {
        vi.advanceTimersByTime(3000);
      });
      expect(rafSpy).not.toHaveBeenCalled();
      rafSpy.mockRestore();
    });

    it("byte-identical when hoverPaused is omitted (regression pin)", () => {
      const withDefault = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} />);
      const withFalse = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} hoverPaused={false} />,
      );
      expect(withDefault.container.innerHTML).toBe(withFalse.container.innerHTML);
    });
  });

  // The queue slider lives in this bar: `total`/`done` drive a segmented
  // floor, never a separate `.track` row. These cover the
  // segment-count/proportional-mapping math against
  // `.ttl-seg`/`.ttl-fill`.
  describe("queue segments (stories merge)", () => {
    function segs(container: HTMLElement) {
      return Array.from(container.querySelectorAll(".ttl-bar .ttl-seg"));
    }

    it("renders one segment per batch item when the batch is at most 10", () => {
      const { container } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={5} done={2} />,
      );
      const all = segs(container);
      expect(all).toHaveLength(5);
      // done segments (before current) are marked; current (index 2)
      // hosts the fill instead of carrying a static class of its own;
      // everything after current is a plain, empty trough.
      expect(all.slice(0, 2).every((s) => s.classList.contains("done"))).toBe(true);
      expect(all[2].classList.contains("done")).toBe(false);
      expect(all.slice(2).every((s) => s.className === "ttl-seg")).toBe(true);

      // SAFETY: TtlBar always renders `.ttl-fill` (fixture passes
      // ttlMs/remainingMs); the next line's null-guard protects the cast.
      const fill = container.querySelector(".ttl-fill") as HTMLElement;
      expect(fill).not.toBeNull();
      // grid-column is 1-indexed; current=2 -> column 3.
      expect(fill.style.gridColumn).toBe("3");
    });

    it("defaults to a single, un-segmented bar when total/done are omitted", () => {
      const { container } = render(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} />);
      const all = segs(container);
      expect(all).toHaveLength(1);
      expect(all[0].classList.contains("done")).toBe(false);
      // SAFETY: TtlBar renders `.ttl-fill` even when total/done are omitted
      // (single un-segmented bar), so the match is non-null.
      const fill = container.querySelector(".ttl-fill") as HTMLElement;
      expect(fill.style.gridColumn).toBe("1");
    });

    it("renders exactly one segment (hosting the fill) for a single-item batch", () => {
      const { container } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={1} done={0} />,
      );
      const all = segs(container);
      expect(all).toHaveLength(1);
      expect(all[0].classList.contains("done")).toBe(false);
      expect(container.querySelector(".ttl-fill")).not.toBeNull();
    });

    it("caps the segment count at 10 for batches beyond the ceiling", () => {
      const { container } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={15} done={0} />,
      );
      const all = segs(container);
      expect(all).toHaveLength(10);
      expect(container.querySelectorAll(".ttl-bar .ttl-seg.done")).toHaveLength(0);
    });

    it("maps the current index proportionally past the 10-segment ceiling", () => {
      // total=20: each segment is 2 items. floor(done * 10 / total).
      const mid = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={20} done={10} />,
      );
      const midSegs = segs(mid.container);
      expect(midSegs).toHaveLength(10);
      expect(midSegs.slice(0, 5).every((s) => s.classList.contains("done"))).toBe(true);
      expect(midSegs[5].classList.contains("done")).toBe(false);
      // SAFETY: `.ttl-fill` always renders inside a mid-batch segmented bar
      // (total=20 fixture), so the inline match is non-null.
      expect((mid.container.querySelector(".ttl-fill") as HTMLElement).style.gridColumn).toBe("6");

      // the last item of the batch lights the final segment (floor(19*10/20)=9)
      const last = render(
        <TtlBar slotId="n2" ttlMs={8000} remainingMs={8000} total={20} done={19} />,
      );
      const lastSegs = segs(last.container);
      expect(lastSegs.slice(0, 9).every((s) => s.classList.contains("done"))).toBe(true);
      expect(lastSegs[9].classList.contains("done")).toBe(false);
      // SAFETY: `.ttl-fill` always renders inside the last-batch segment bar
      // (total=20 fixture), so the inline match is non-null.
      expect((last.container.querySelector(".ttl-fill") as HTMLElement).style.gridColumn).toBe(
        "10",
      );
    });

    it("hands the segment count to the grid via the --queue-n custom property", () => {
      const { container } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={4} done={0} />,
      );
      // SAFETY: TtlBar always renders its `.ttl-bar` container for this
      // total/done fixture, so the match to read --queue-n is non-null.
      const bar = container.querySelector(".ttl-bar") as HTMLElement;
      expect(bar.style.getPropertyValue("--queue-n")).toBe("4");
    });

    it("keeps the fill node itself stable when the current segment changes (no remount)", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={5} done={0} />,
      );
      const fillBefore = container.querySelector(".ttl-fill");
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={8000} total={5} done={2} />);
      const fillAfter = container.querySelector(".ttl-fill");
      expect(fillAfter).toBe(fillBefore);
      // SAFETY: `.ttl-fill` stays mounted across the rerender (pinned by the
      // `toBe(fillBefore)` check just above), so this read is non-null.
      expect((fillAfter as HTMLElement).style.gridColumn).toBe("3");
    });

    it("keeps the fill's own scaleX animation running when total/done change (same clock, unaffected)", () => {
      const { container, rerender } = render(
        <TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} total={3} done={0} />,
      );
      act(() => {
        vi.advanceTimersByTime(16);
      });
      const before = fillScalePercent(container);
      expect(before).toBeGreaterThan(0);

      // queue depth changes, ttlMs/remainingMs don't -> no re-anchor, the
      // countdown keeps counting from where it was, just relocated to a
      // new segment.
      rerender(<TtlBar slotId="n1" ttlMs={8000} remainingMs={4000} total={5} done={1} />);
      act(() => {
        vi.advanceTimersByTime(16);
      });
      expect(fillScalePercent(container)).toBeLessThanOrEqual(before);
    });
  });
});
