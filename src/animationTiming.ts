// JS-side animation timing, single source for every motion consumer.
//
// Lockstep pairs with overlay-card.css, guarded by animationTiming.test.ts:
// SWAP_EXIT_MS ↔ `.card-assembly.exiting` `transition: width`,
// CONTENT_EXIT_MS ↔ flank-round `transition: border-radius`.

// Lockstep consumers (see header): useDelayedSwap's exit window, the
// motion.div swap duration (`SWAP_EXIT_MS / 1000`), and overlay-card.css's
// `.card-assembly.exiting` `transition: width` — all derive from this one.
export const SWAP_EXIT_MS = 175;

// Below-block's own exit window — shorter than, and independent of,
// SWAP_EXIT_MS; exit-only (entrance stays gated on `renderedShowing`).
// Lockstep: overlay-card.css's flank-round `transition: border-radius`
// must stay numerically equal (header's second pair).
export const CONTENT_EXIT_MS = 105;

// Lockstep: numeric twin of `--ease-notchtap` (shared-ui design/tokens.css
// and src/styles.css's redeclaration), guarded by animationTiming.test.ts.
export const NOTCHTAP_EASE: [number, number, number, number] = [0.23, 1, 0.32, 1];

// Shell entrance width-grow and manifest disclosure expand/collapse.
// Lockstep: equals shared-ui's --duration-normal; CSS consumers keep
// `var(--expand-ms, ...)` fallbacks.
export const EXPAND_MS = 300;

// Bare<->hovered rail paint coordination (flank + status-dot mount fades).
// Lockstep: injected as `--reveal-ms`; every CSS consumer keeps a 260ms
// `var()` fallback.
export const REVEAL_MS = 260;

// The `.card-assembly.hovered` scale response — faster than REVEAL_MS:
// REVEAL_MS governs chrome paint, HOVER_MS the whole-card scale on top.
export const HOVER_MS = 160;

// Lighter pair for showing->showing rotations only (news rotation,
// live-match signal updates) — never the promotion/exit legs. Exit is a
// quick fade, enter a longer settle; asymmetric on purpose.
export const ROTATION_EXIT_MS = 70;
export const ROTATION_ENTER_MS = 120;

// Fastest, sharpest leg: a preempted card reads as "yanked", not an
// end-of-turn rotation — INTERRUPT_EXIT_MS < ROTATION_EXIT_MS, paired
// with INTERRUPT_EASE. Enter leg reuses SWAP_EXIT_MS/NOTCHTAP_EASE.
export const INTERRUPT_EXIT_MS = 60;
export const INTERRUPT_EASE: [number, number, number, number] = [0.4, 0, 1, 1];

// Outermost crossfade (App.tsx Agent Board <-> status-rail). Symmetric,
// house-eased, no geometry leg — unlike SWAP_EXIT_MS.
export const SURFACE_SWAP_MS = 180;

// Board summon arrival — deliberately longer than SURFACE_SWAP_MS's exit
// (an arrival earns emphasis, a dismissal quieter). Law: any future
// entrance emphasis animates ONLY the below-block, NEVER the shell — the
// cutout must never scale, translate, or fade.
export const BOARD_SUMMON_MS = 260;

// Shared hover-disclosure spring; slight overshoot deliberate ("pulled
// open"). Spring drives every animated property, opacity included, so an
// interruption retargets all on one clock. Do not reintroduce a
// per-property duration override here.
export const DISCLOSURE_SPRING = { type: "spring", stiffness: 480, damping: 37 } as const;

// IDLE_REVEAL_MS: whole-element entrance, slower/softer than a hover
// response. IDLE_GLANCE_MS: eyes' CSS transform shifts — quicker, or
// the face looks sedated.
export const IDLE_REVEAL_MS = 240;
export const IDLE_GLANCE_MS = 200;

// Stagger so no glyph frame renders against the desktop before the
// flank's black paint starts.
export const ICON_STRIP_STAGGER_MS = 60;

// News charge-fill step: one quiet step per newly-landed story;
// charging, never glows.
export const NEWS_CHARGE_STEP_MS = 320;
