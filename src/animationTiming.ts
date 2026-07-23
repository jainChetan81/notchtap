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

// 2026-07-23 review fix (Duplicated Code finding, wave C — CSS custom-
// property injection): the shell's own entrance width-grow (base
// `.card-assembly`'s `transition: width`) and the manifest disclosure's
// expand/collapse (`.manifest-wrap`'s `transition: grid-template-rows`/
// `opacity`) used to carry two independently-hand-tuned literals, 320ms
// and 300ms — a 20ms gap with no documented reason, just an artifact of
// two separate polish passes touching one but not the other. Neither
// number has ever had a JS-side consumer (unlike SWAP_EXIT_MS/
// CONTENT_EXIT_MS above), so there was no drift RISK, only needless
// inconsistency between two "something is opening" motions that read as
// the same gesture. Unified onto this single constant (applied via
// applyAnimationTiming.ts, same as the other two) rather than leaving
// the 20ms offset undocumented — screenshot-verified
// (docs/review-logs) that neither the shell's own promotion-grow nor the
// manifest's expand toggle changed character at the unified 320ms.
export const EXPAND_MS = 320;

// plan 127 (Step 1, /improve-animations audit finding #4): the bare<->
// hovered rail's own reveal/paint coordination duration — StatusRailCard's
// FlankClock/StatusDots mount fades (each a bare `AnimatePresence` +
// `motion.span`/`motion.div`, not a hover-driven CSS transition) and
// overlay-card.css's flank background/padding fade + `.track span`
// background fade all used to hand-type the same 260ms/0.26 literal in
// four independent spots with no lockstep guard between them — the exact
// "desynced clocks" shape this file exists to prevent, just one this
// plan's audit was the first to name explicitly. Single-sourced here and
// injected as `--reveal-ms` (see `applyAnimationTiming.ts`); every CSS
// consumer keeps a `260ms` fallback for the same defense-in-depth reason
// EXPAND_MS's own fallback does.
export const REVEAL_MS = 260;

// plan 127 (Step 1, finding #5 groundwork): the hover "breathe" response
// (the `.card-assembly.hovered` scale, overlay-card.css) used to ride
// REVEAL_MS's 260ms — comfortably outside the ~125-200ms budget a hover
// response should land in (see Step 4's own doc for the audit finding).
// A dedicated, faster constant rather than repurposing REVEAL_MS, since
// the two now diverge: REVEAL_MS still governs the bare<->hovered PAINT
// coordination (chrome fading in/out), HOVER_MS governs the whole-card
// scale response layered on top of that paint.
export const HOVER_MS = 160;

// plan 127 (Step 1, finding #3 groundwork): the same-slot content
// rotation swap (StatusRailCard's inner `AnimatePresence mode="wait"`,
// keyed `swapKey`) — a LIGHTER pair of durations used only for
// showing->showing rotations (news items rotating every ~10s, live-match
// signal updates, ...), never for the idle<->showing promotion/exit legs,
// which keep CONTENT_EXIT_MS/SWAP_EXIT_MS untouched (see Step 3's own
// doc in StatusRailCard.tsx for the full "why"). Deliberately two
// separate constants (not a single "rotation" duration split arithmetically
// in half) since exit and enter play genuinely different roles here: the
// exit is a quick fade-away of stale content, the enter is a slightly
// longer settle of the fresh content — asymmetric on purpose, unlike the
// promotion/exit legs' shared NOTCHTAP_EASE-only symmetry.
export const ROTATION_EXIT_MS = 70;
export const ROTATION_ENTER_MS = 120;
