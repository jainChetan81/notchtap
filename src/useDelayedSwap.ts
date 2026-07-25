import { useLayoutEffect, useRef, useState } from "react";

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

  // H3 fix (2026-07-25): `shown.value` is written by this hook at only
  // two points — initial mount (above) and the exit timeout below — so
  // it used to go stale across any number of same-key renders in
  // between: a same-key update passes through live via `liveValue`
  // below, but was NEVER persisted anywhere durable. If `key` then
  // changed, the exit freeze read that stale value instead of whatever
  // was actually on screen the instant before the exit started (e.g. a
  // card that auto-collapsed `expanded` mid-visible would still exit
  // with `expanded: true`, because that's what `shown.value` was frozen
  // at back when this key was first promoted in). `lastLiveValueRef`
  // fixes this: it mirrors `value` on every render where `key` still
  // matches `shown.key`, so it always holds the truly-last-rendered
  // value for the currently-live key — that's what the freeze below
  // reads instead of `shown.value`.
  const lastLiveValueRef = useRef(value);
  if (key === shown.key) {
    lastLiveValueRef.current = value;
  }

  // Mirrors the latest (key, value) pair unconditionally, on every
  // render. The exit timeout (below) reads from this ref rather than
  // closing over the `value`/`key` from the render that scheduled it —
  // the same staleness bug as `lastLiveValueRef` fixes above, but on the
  // INCOMING side: if the new key re-renders with an updated value
  // while its exit timer is still pending (key unchanged, so the effect
  // below doesn't re-run and the timer isn't reset), the eventual swap
  // must land on that latest value, not the one captured when the timer
  // was first set.
  const incomingRef = useRef<{ key: unknown; value: T }>({ key, value });
  incomingRef.current = { key, value };

  // Only a `key` change (re)starts the exit timer; a same-key value
  // update is synced at render time (via the refs above), never through
  // this effect — so `value` is intentionally not a dependency, and the
  // dependency list below is genuinely exhaustive (the timer body reads
  // the incoming value through `incomingRef`, not a captured `value`).
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

  // same key: pass the live value straight through (no state update, no
  // re-render caused by this hook) — this is what makes the existing
  // "no remount on same-key content update" test hold.
  // key changed (exit window): freeze on the LAST value actually
  // rendered for the outgoing key, via `lastLiveValueRef` — not
  // `shown.value`, which is only ever touched by the timeout above and
  // would otherwise replay a promotion-time snapshot for the whole exit
  // window (H3).
  const liveValue = key === shown.key ? value : lastLiveValueRef.current;
  return { value: liveValue, exiting };
}
