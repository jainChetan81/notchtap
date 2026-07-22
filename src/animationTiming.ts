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

// StatusRailCard's `useDelayedSwap(slot, swapKey, ...)` exit window — must
// equal overlay-card.css's `.card-content`/`.card-content.swap-exit`
// `card-enter-*`/`card-exit-*` animation durations (220ms).
export const SWAP_EXIT_MS = 220;
