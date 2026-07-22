import { useEffect, useState } from "react";

// plan 117: single-sources the `prefers-reduced-motion: reduce` read that
// used to be inlined three times (TtlBar.tsx, IdleHoverPeek.tsx x2), each
// with its own copy of the `typeof window.matchMedia === "function"` jsdom
// guard. This file changes WHERE the boolean comes from only — every call
// site keeps its own existing WHEN/HOW (render-time hook vs. one-shot
// effect-time read), so behavior is unchanged.
//
// Deliberately NOT touching the 9 CSS `@media (prefers-reduced-motion:
// reduce)` art blocks in overlay-card.css — those are a separate, correct
// layer (pure-CSS animations no JS ever gates) and stay as-is.
const QUERY = "(prefers-reduced-motion: reduce)";

function matches(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return false;
  }
  return window.matchMedia(QUERY).matches;
}

// Plain, on-demand read for call sites that need the value inside a
// non-render callback — a `useEffect` body or a nested one-shot helper
// (TtlBar's rAF-arming effect, IdleHoverPeek's close-delay effect and its
// `useLiveTick` interval-arming effect) — where subscribing to a hook
// would change re-render timing that none of those effects currently have.
export function prefersReducedMotion(): boolean {
  return matches();
}

// Subscribing hook for call sites that need the value to stay live across
// a mid-session OS preference flip and re-render on change. Nothing in
// this codebase currently needs that (every existing consumer reads once,
// inside an effect, matching `prefersReducedMotion()` above) — this is the
// render-time counterpart for future use, jsdom/SSR-safe via the same
// `matchMedia` guard.
export function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(matches);

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
      return;
    }
    const mql = window.matchMedia(QUERY);
    const onChange = () => setReduced(mql.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);

  return reduced;
}
