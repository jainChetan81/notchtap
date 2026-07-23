import {
  CONTENT_EXIT_MS,
  EXPAND_MS,
  HOVER_MS,
  REVEAL_MS,
  ROTATION_ENTER_MS,
  ROTATION_EXIT_MS,
  SWAP_EXIT_MS,
} from "./animationTiming";

// 2026-07-23 review fix (Duplicated Code / desynced-clocks structural
// fix, wave C): every JS<->CSS animation duration used to exist as a
// literal in BOTH animationTiming.ts and overlay-card.css, with a
// regex-parsing test (animationTiming.test.ts) standing guard against
// the two drifting apart. That guard caught drift after the fact; it
// never prevented a human from editing one side and forgetting the
// other. This module inverts the relationship: the JS constants become
// CSS custom properties on the document root, and overlay-card.css
// consumes them via `var(--x, <matching-literal-fallback>)` instead of
// hand-copied numbers — there is now exactly ONE place each duration is
// written as a literal (this file's import from animationTiming.ts),
// plus a CSS `var()` fallback that only ever matters if this module
// somehow never ran.
//
// Same pattern App.tsx's `applyAppearance` already uses for
// `--card-scale`/`--card-radius`/`--card-opacity` — a plain
// `root.style.setProperty` call, no framework machinery.
//
// MUST be called from BOTH real entry points (`src/main.tsx`, the
// overlay window, and `src/settings/main.tsx`, the settings window) —
// not just the overlay. The settings window's Appearance preview
// renders the exact same `overlay-card.css` (plan 111's shared
// stylesheet), and a prior harness bug is exactly why both matter: an
// undefined custom property used inside a `transition:` shorthand
// invalidates the ENTIRE shorthand, not just the one duration — so a
// settings-only skip wouldn't just fall back to the CSS fallback value,
// it would silently drop the whole transition, including properties
// that never had a JS-sourced duration at all (`transform`, `filter`,
// ...). The `var(--x, <fallback>)` fallbacks in overlay-card.css exist
// as defense in depth for a context this app doesn't currently have
// (e.g. a stylesheet-only preview tool), not as a reason to skip
// calling this from either real entry.
export function applyAnimationTiming(
  root: Pick<CSSStyleDeclaration, "setProperty"> = document.documentElement.style,
) {
  root.setProperty("--swap-exit-ms", `${SWAP_EXIT_MS}ms`);
  root.setProperty("--content-exit-ms", `${CONTENT_EXIT_MS}ms`);
  root.setProperty("--expand-ms", `${EXPAND_MS}ms`);
  // plan 127 (Step 1): same pattern as the three above — one JS-sourced
  // literal, a CSS custom property, a `var(--x, <matching-fallback>)`
  // consumer. ROTATION_EXIT_MS/ROTATION_ENTER_MS have no CSS consumer of
  // their own (StatusRailCard.tsx reads the JS constants directly for
  // motion's `transition.duration`, same as SWAP_EXIT_MS/CONTENT_EXIT_MS
  // already do) — injected anyway for symmetry with the other timing
  // tokens and in case a future CSS rule needs to reference them.
  root.setProperty("--reveal-ms", `${REVEAL_MS}ms`);
  root.setProperty("--hover-ms", `${HOVER_MS}ms`);
  root.setProperty("--rotation-exit-ms", `${ROTATION_EXIT_MS}ms`);
  root.setProperty("--rotation-enter-ms", `${ROTATION_ENTER_MS}ms`);
}
