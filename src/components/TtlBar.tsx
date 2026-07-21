import { useEffect, useRef } from "react";

// plan 081: the thin rotation-countdown bar every showing card carries.
// Deliberately NOT React state per frame — the prototype
// (prototype/notch-states.html §4's ttlTick) mutates the fill node's
// style.width directly via rAF, and this component follows that: a
// re-render per animation frame would be needless work for a value React
// never needs to read back.
//
// Anchoring: `remainingMs` is a snapshot taken server-side at emission
// time (queue.rs's current_slot_state) — by the time this component
// receives it, some of that time has already elapsed in flight. Anchoring
// `deadline = performance.now() + remainingMs` on receipt (mount, or
// whenever remainingMs/ttlMs/slotId changes) and counting down locally
// from there is the honest way to track it without needing wall-clock
// sync between rust and the webview.
export function TtlBar({
  slotId,
  ttlMs,
  remainingMs,
}: {
  slotId: string;
  ttlMs: number;
  remainingMs: number;
}) {
  const fillRef = useRef<HTMLDivElement | null>(null);

  // Re-anchors on mount and on every slotId/ttlMs/remainingMs change: a
  // new promotion (slotId changes) or a same-id re-emit with a fresh
  // remainingMs (supersede top-up, manual expand) both need the countdown
  // to restart from the new numbers, not keep counting from the old ones.
  // biome-ignore lint/correctness/useExhaustiveDependencies: slotId isn't read in the body, but it's the deliberate re-anchor trigger documented above (same pattern as StatusRailCard's currentId) — a new promotion must restart the countdown even when ttlMs/remainingMs happen to coincide.
  useEffect(() => {
    const fill = fillRef.current;
    if (!fill) {
      return;
    }

    // Idle-CPU discipline (plans 015/018): under prefers-reduced-motion, a
    // CSS rule alone can't stop a rAF loop, so the loop itself is gated in
    // JS — render a static full-width fill and never arm the loop.
    const reducedMotion =
      typeof window.matchMedia === "function" &&
      window.matchMedia("(prefers-reduced-motion: reduce)").matches;

    if (reducedMotion || ttlMs <= 0) {
      fill.style.width = "100%";
      return;
    }

    const deadline = performance.now() + remainingMs;
    let frame: number;

    function tick() {
      const fillEl = fillRef.current;
      if (!fillEl) {
        return;
      }
      const remaining = Math.max(0, deadline - performance.now());
      const pct = Math.min(100, (remaining / ttlMs) * 100);
      fillEl.style.width = `${pct}%`;
      frame = requestAnimationFrame(tick);
    }
    frame = requestAnimationFrame(tick);

    return () => cancelAnimationFrame(frame);
  }, [slotId, ttlMs, remainingMs]);

  return (
    <div className="ttl-bar">
      <div className="ttl-fill" ref={fillRef} />
    </div>
  );
}
