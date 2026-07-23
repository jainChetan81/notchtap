// plan 117: single-sources the `prefers-reduced-motion: reduce` read that
// used to be inlined at multiple call sites, each with its own copy of the
// `typeof window.matchMedia === "function"` jsdom guard. Sole remaining
// consumer today: TtlBar's rAF-arming effect (the motion migration removed
// the IdleHoverPeek call sites).
//
// Reduced-motion is a PERMANENT NON-GOAL for this app (operator decree,
// 2026-07-23): leave what exists, never extend it. A subscribing
// `usePrefersReducedMotion` hook that shipped here "for future use" with
// zero consumers was removed under that decree — do not re-add PRM
// infrastructure without the operator explicitly reopening the decision.
//
// Deliberately NOT touching the CSS `@media (prefers-reduced-motion:
// reduce)` art blocks in overlay-card.css — those are a separate, correct
// layer (pure-CSS animations no JS ever gates) and stay as-is.
const QUERY = "(prefers-reduced-motion: reduce)";

// Plain, on-demand read for call sites that need the value inside a
// non-render callback (TtlBar's rAF-arming effect), where subscribing to a
// hook would change re-render timing the effect doesn't currently have.
export function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia(QUERY).matches;
}
