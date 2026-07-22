// plan 117: single-sources two JS-side duration literals that were each
// duplicated between a component and overlay-card.css, with nothing
// enforcing the pair stayed equal. This file only changes the JS side —
// the CSS values (overlay-card.css) are left as plain literals, annotated
// with a comment pointing back here (converting them to a CSS custom
// property was evaluated and deliberately skipped: the risk of shifting
// the load-bearing swap/peek timing outweighed the DRY win for two
// numbers). If either value below changes, its CSS counterpart MUST
// change in the same commit.

// StatusRailCard's `useDelayedSwap(slot, swapKey, ...)` exit window — must
// equal overlay-card.css's `.card-content`/`.card-content.swap-exit`
// `card-enter-*`/`card-exit-*` animation durations (220ms).
export const SWAP_EXIT_MS = 220;

// IdleHoverPeek's post-unhover unmount delay — must equal overlay-card.css's
// `.below-block.idle-peek.closing` `idle-peek-close` animation duration
// (260ms).
export const IDLE_PEEK_CLOSE_MS = 260;
