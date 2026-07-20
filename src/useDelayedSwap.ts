import { useLayoutEffect, useState } from "react";

// Small stand-in for `AnimatePresence mode="wait"` (plan 078 dropped
// `motion` from the overlay bundle — see styles.css for the CSS half of
// this). Freezes `value` at its last snapshot while `key` has changed
// but the exit animation hasn't finished, then swaps to the new
// value/key together once `exitDurationMs` elapses. A same-key update
// (the content changed but the key didn't — e.g. a queue-counter tick
// on the still-visible item) is synced immediately, in place, with no
// timer and no animation replay.
export function useDelayedSwap<T>(
  value: T,
  key: unknown,
  exitDurationMs: number,
): { value: T; exiting: boolean } {
  const [shown, setShown] = useState<{ key: unknown; value: T }>({ key, value });
  const [exiting, setExiting] = useState(false);

  // biome-ignore lint/correctness/useExhaustiveDependencies: `value` is deliberately excluded — only a `key` change should (re)start the exit timer; a same-key value update is synced below, at render time, not through this effect.
  useLayoutEffect(() => {
    if (key === shown.key) {
      return;
    }
    setExiting(true);
    const id = window.setTimeout(() => {
      setShown({ key, value });
      setExiting(false);
    }, exitDurationMs);
    return () => window.clearTimeout(id);
  }, [key, shown.key, exitDurationMs]);

  // same key: pass the live value straight through (no state update, no
  // re-render caused by this hook) — this is what makes the existing
  // "no remount on same-key content update" test hold.
  const liveValue = key === shown.key ? value : shown.value;
  return { value: liveValue, exiting };
}
