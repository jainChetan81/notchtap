import { useLayoutEffect, useRef, useState } from "react";

// Stand-in for `AnimatePresence mode="wait"` (see styles.css for the CSS
// half). Freezes `value` at its last snapshot while `key` changed but the
// exit animation hasn't finished; same-key updates sync immediately, in
// place, with no timer or replay.
export function useDelayedSwap<T>(
  value: T,
  key: unknown,
  exitDurationMs: number,
): { value: T; exiting: boolean } {
  const [shown, setShown] = useState<{ key: unknown; value: T }>({ key, value });
  const [exiting, setExiting] = useState(false);

  // `lastLiveValueRef` mirrors `value` on every render where `key` matches
  // `shown.key`, so the exit freeze reads the truly-last-rendered value for
  // the outgoing key, never a stale `shown.value` snapshot.
  const lastLiveValueRef = useRef(value);
  if (key === shown.key) {
    lastLiveValueRef.current = value;
  }

  // Mirrors the latest (key, value) unconditionally, so a pending exit
  // timer lands on the newest value, not the one from the render that
  // scheduled it.
  const incomingRef = useRef<{ key: unknown; value: T }>({ key, value });
  incomingRef.current = { key, value };

  // Only a `key` change (re)starts the exit timer; same-key updates sync
  // at render time. `value` is intentionally not a dependency — the timer
  // body reads `incomingRef`, so the list is genuinely exhaustive.
  useLayoutEffect(() => {
    if (key === shown.key) {
      return;
    }
    setExiting(true);
    const id = window.setTimeout(() => {
      setShown(incomingRef.current);
      setExiting(false);
    }, exitDurationMs);
    return () => window.clearTimeout(id);
  }, [key, shown.key, exitDurationMs]);

  // same key: live value straight through, no remount. key changed:
  // freeze on the last rendered value via `lastLiveValueRef`, not
  // `shown.value`.
  const liveValue = key === shown.key ? value : lastLiveValueRef.current;
  return { value: liveValue, exiting };
}
