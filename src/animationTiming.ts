// plan 117: single-sources a JS-side duration literal that was also
// duplicated in overlay-card.css, with nothing enforcing the pair stayed
// equal. This file only changes the JS side — the CSS value
// (overlay-card.css) is left as a plain literal, annotated with a comment
// pointing back here (converting it to a CSS custom property was
// evaluated and deliberately skipped: the risk of shifting the
// load-bearing swap timing outweighed the DRY win for one number). If the
// value below changes, its CSS counterpart MUST change in the same
// commit.
//
// plan 12x: this file used to also carry `IDLE_PEEK_CLOSE_MS`, the
// hand-rolled unmount-delay timer IdleHoverPeek.tsx used to run alongside
// a matching CSS close-keyframe. That machine is gone — IdleHoverPeek now
// mounts/unmounts via `motion`'s `AnimatePresence`, which owns its own
// exit window, so there's no longer a JS-side literal to single-source
// for it.

// plan 12x (wave 2): StatusRailCard's content-swap moved off CSS
// `@keyframes` onto `motion` (AnimatePresence + motion.div), but this
// constant is still load-bearing in TWO places there, and both must stay
// equal to each other:
//   1. `useDelayedSwap(slot, swapKey, SWAP_EXIT_MS)` — kept, but now
//      scoped to GEOMETRY only (the outer shell's priority/expanded
//      classes, plan 107's choreography): it must NOT move into motion,
//      per that plan's contract, so it still needs its own JS-timer
//      exit window.
//   2. the `motion.div` swap's own `transition.duration` (seconds,
//      SWAP_EXIT_MS / 1000) — so the visual content fade finishes at
//      (or just before) the geometry timer flips the shell to idle,
//      never after.
export const SWAP_EXIT_MS = 220;
