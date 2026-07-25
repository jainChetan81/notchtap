import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDelayedSwap } from "./useDelayedSwap";

const EXIT_MS = 220;

function renderSwap(initialValue: string, initialKey: string) {
  return renderHook(({ value, key }) => useDelayedSwap(value, key, EXIT_MS), {
    initialProps: { value: initialValue, key: initialKey },
  });
}

describe("useDelayedSwap", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("syncs a same-key value update immediately, with no exit phase", () => {
    const { result, rerender } = renderSwap("v1", "k1");
    expect(result.current).toEqual({ value: "v1", exiting: false });

    rerender({ value: "v2", key: "k1" });
    expect(result.current).toEqual({ value: "v2", exiting: false });

    act(() => vi.advanceTimersByTime(EXIT_MS * 2));
    expect(result.current).toEqual({ value: "v2", exiting: false });
  });

  it("freezes the old value on a key change, then swaps after exitDurationMs", () => {
    const { result, rerender } = renderSwap("old", "k1");

    rerender({ value: "new", key: "k2" });
    // synchronously after the key change (useLayoutEffect): old value
    // still shown, exit phase flagged.
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS - 1));
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(1));
    expect(result.current).toEqual({ value: "new", exiting: false });
  });

  it("a second key change before the first timer fires cancels it — only the latest key swaps", () => {
    const { result, rerender } = renderSwap("v1", "k1");

    rerender({ value: "v2", key: "k2" });
    expect(result.current).toEqual({ value: "v1", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    rerender({ value: "v3", key: "k3" });
    // still frozen on the original snapshot — no intermediate swap to v2.
    expect(result.current).toEqual({ value: "v1", exiting: true });

    // the k2 timer would have fired within this window if it hadn't been
    // cleaned up; only k3's timer may fire, landing straight on v3.
    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "v1", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "v3", exiting: false });
  });

  // H3 regression: a same-key update (e.g. a card's `expanded` flag
  // auto-retracting mid-visible) must be what a subsequent key change
  // freezes on, not the value from whenever this key was first
  // promoted in. Before the fix, `shown.value` was only ever written at
  // mount/swap time, so it silently replayed the ORIGINAL value for the
  // whole exit window instead of the last-rendered one.
  it("freezes the LAST-rendered same-key value on a key change, not the value from when the key first appeared", () => {
    const { result, rerender } = renderSwap("A", "k1");
    expect(result.current).toEqual({ value: "A", exiting: false });

    // same-key update: passes through live, no state write, no timer —
    // this is exactly the update that used to get lost.
    rerender({ value: "B", key: "k1" });
    expect(result.current).toEqual({ value: "B", exiting: false });

    // key changes: the exit window must freeze on "B" (last rendered),
    // not "A" (the stale mount-time snapshot).
    rerender({ value: "C", key: "k2" });
    expect(result.current).toEqual({ value: "B", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS));
    expect(result.current).toEqual({ value: "C", exiting: false });
  });

  // H3 regression, incoming side: if the NEW key re-renders again with
  // an updated value while its own exit timer is still pending (key
  // unchanged, so the timer isn't reset), the eventual swap must land
  // on that freshest value rather than the one captured when the timer
  // was first scheduled.
  it("swaps to the freshest incoming value if the new key re-renders again before its exit timer fires", () => {
    const { result, rerender } = renderSwap("old", "k1");

    rerender({ value: "mid", key: "k2" });
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    // same key (k2), updated value — still mid-exit, frozen output
    // unaffected, but this should become the eventual swap target.
    rerender({ value: "final", key: "k2" });
    expect(result.current).toEqual({ value: "old", exiting: true });

    act(() => vi.advanceTimersByTime(EXIT_MS / 2));
    expect(result.current).toEqual({ value: "final", exiting: false });
  });
});
