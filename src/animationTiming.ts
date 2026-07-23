// plan 117: single-sources the overlay's JS-side animation timing.
//
// HISTORY NOTE (2026-07-23 review fix): this header used to demand a CSS
// counterpart for SWAP_EXIT_MS ("must change in the same commit") — that
// contract died in wave 2, when the card-enter/exit @keyframes it referred
// to were deleted in favor of motion's AnimatePresence. SWAP_EXIT_MS's two
// consumers were both JS-side for a while (see its own doc below); wave B
// (2026-07-23, "one overlapping collapse") reintroduced a real CSS
// counterpart — `.card-assembly.exiting`'s `width` transition duration in
// overlay-card.css, which must stay numerically equal to SWAP_EXIT_MS so
// the shell's width finishes shrinking to idle by the same tick the
// geometry-class freeze itself lets go (see StatusRailCard.tsx's
// `shellExiting` doc for the full "why"). This file now carries TWO CSS
// lockstep pairs, both guarded by animationTiming.test.ts: CONTENT_EXIT_MS
// ↔ the flank-round `transition: border-radius` duration, and SWAP_EXIT_MS
// ↔ `.card-assembly.exiting`'s `transition: width` duration, both in
// overlay-card.css.
//
// plan 12x: this file used to also carry `IDLE_PEEK_CLOSE_MS`, the
// hand-rolled unmount-delay timer IdleHoverPeek.tsx used to run alongside
// a matching CSS close-keyframe. That machine is gone — IdleHoverPeek now
// mounts/unmounts via `motion`'s `AnimatePresence`, which owns its own
// exit window, so there's no longer a JS-side literal to single-source
// for it.

// plan 12x (wave 2): StatusRailCard's content-swap moved off CSS
// `@keyframes` onto `motion` (AnimatePresence + motion.div), but this
// constant is still load-bearing in multiple places, and all must stay
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
//   3. (wave B, 2026-07-23) overlay-card.css's `.card-assembly.exiting`
//      `transition: width` duration — a real CSS literal now, guarded by
//      animationTiming.test.ts (see this file's own header note).
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

// 2026-07-23 review fix (Duplicated Code finding): the overlay's signature
// easing curve, single-sourced for every JS/motion consumer. This is the
// numeric twin of shared-ui's `--ease-notchtap: cubic-bezier(.22,1,.36,1)`
// token (vendor/shared-ui/design/tokens.css) — a real cross-file lockstep
// pair, now GUARDED by a test in animationTiming.test.ts that parses the
// token and compares it to this array, so drift fails CI instead of
// shipping two subtly different eases.
export const NOTCHTAP_EASE: [number, number, number, number] = [0.22, 1, 0.36, 1];
