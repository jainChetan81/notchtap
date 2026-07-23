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
// plan 12x (wave 3, operator-feedback polish pass): dropped 220 -> 175
// (~20% quicker) for a snappier feel, per that pass's "faster overall"
// finding — every consumer's own duration-derivation is untouched (still
// `SWAP_EXIT_MS`/`SWAP_EXIT_MS / 1000`), so this single edit retunes the
// geometry freeze, the content-swap ENTER duration, AND (indirectly,
// since StatusRailCard.test.tsx asserts against this constant, not a
// hardcoded number) the pinned "compact->idle geometry" test window, all
// at once. No consumer needed its own edit.
export const SWAP_EXIT_MS = 175;

// plan 11x: the below-block's OWN exit window — deliberately shorter than
// (and independent of) SWAP_EXIT_MS above, and exit-only (entrance is
// untouched, still gated on the SWAP_EXIT_MS-driven `renderedShowing`
// exactly as before). Fixes the "compact ends, then ~200ms later the
// corner rounds" bug: previously the below-block stayed mounted at full
// (square-cornered) shape for the FULL SWAP_EXIT_MS before vanishing, so
// the flank corner-round (overlay-card.css) — which can only safely
// start once the below-block is actually gone, per that file's ROUNDING
// LAW — couldn't begin until SWAP_EXIT_MS in, then took its own
// (formerly 260ms, now much shorter) duration on top: two visibly
// chained acts. Shortening JUST the below-block's own close (this
// constant) to run in parallel with its own opacity fade, then letting
// the flank round start right after, collapses the two acts into one
// motion. StatusRailCard.tsx pairs this with a matching, shortened
// flank-round transition duration in overlay-card.css
// (`:not(:has(.below-block))`'s `transition: border-radius`) — both must
// change together, same discipline as the SWAP_EXIT_MS/CSS pairing
// above. Not reusing SWAP_EXIT_MS itself for this because that value is
// also the outer shell's GEOMETRY freeze (plan 107, pinned by
// StatusRailCard.test.tsx's "compact->idle geometry" describe block) and
// the content swap's ENTER duration — shortening it directly would speed
// up (and change the feel of) both of those, which this plan must not
// touch.
//
// plan 12x (wave 3): dropped 130 -> 105 (~19% quicker), same pass and
// same reasoning as SWAP_EXIT_MS above — paired unconditionally with
// overlay-card.css's flank-round `transition: border-radius` duration,
// which must stay numerically equal to this constant (see that rule's
// own comment).
export const CONTENT_EXIT_MS = 105;
