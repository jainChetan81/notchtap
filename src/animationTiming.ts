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

// plan 146b (Priority Preemption, spec `docs/ARCHITECTURE.md` §21 / plan
// 146's "interrupt exit" deliverable): the ONE new timing pair this plan
// adds. A strictly-higher-priority arrival now cuts the Visible card's
// turn short (queue.rs's `try_preempt_visible`, rust-side, no wire flag —
// see StatusRailCard.tsx's own doc on how the frontend infers this from
// existing fields) — that handover must read as "yanked because
// something more important arrived," not as an ordinary end-of-turn
// rotation (ROTATION_EXIT_MS/ROTATION_ENTER_MS above) or a glitch.
// INTERRUPT_EXIT_MS is deliberately SHORTER than ROTATION_EXIT_MS itself
// (70ms) — the fastest, sharpest leg on the card — paired with
// INTERRUPT_EASE (a snap-in ease-in curve, not the house NOTCHTAP_EASE
// glide) and a small y/scale "yank" in `contentExitVariants` (see that
// export's own doc). The ENTER leg deliberately reuses SWAP_EXIT_MS/
// NOTCHTAP_EASE — the ordinary PROMOTION entrance, not the lighter
// rotation-enter — because the incoming card is a genuine new Promotion
// taking the Slot, not a same-tier rotation; only the OUTGOING leg needs
// its own distinct treatment.
export const INTERRUPT_EXIT_MS = 60;
export const INTERRUPT_EASE: [number, number, number, number] = [0.4, 0, 1, 1];

// plan 148 (motion token cohesion): the OUTERMOST transition in the
// product — App.tsx's Agent Board <-> status-rail crossfade — used to
// hand-type `{ duration: 0.18 }` at both of its `motion.div` sites, with
// NO `ease` at all, so motion's default tween curve applied there and
// only there while every other motion call in the repo passes
// NOTCHTAP_EASE. Same value as before (0.18s, this is tokenization, not
// a retune), now named and house-eased. Distinct from SWAP_EXIT_MS
// (which is a WITHIN-card content swap plus a geometry freeze) because
// this one swaps the whole top-level SURFACE: neither card is
// "continuing" into the other, so it's a plain symmetric crossfade with
// no geometry leg to stay in lockstep with.
export const SURFACE_SWAP_MS = 180;

// 2026-08-02 animation audit (Agent Board finding #1): the Board's own
// ARRIVAL leg, split off from SURFACE_SWAP_MS above. Since the Board now
// only appears when an agent actually needs the operator (permission /
// input / failure / completion), its summon is the highest-stakes moment
// the overlay has — and it was riding the same 180ms opacity-only fade a
// routine idle<->rail swap uses, so the most important surface in the
// product arrived with the least presence.
//
// Deliberately asymmetric with SURFACE_SWAP_MS, which stays the EXIT
// duration for both surfaces: an arrival earns emphasis (a longer,
// house-eased settle), a dismissal should be quieter than the thing it
// dismisses. That is a conscious exception to the "mirror the exit path
// exactly" spatial-consistency rule — see App.tsx's
// `BOARD_SURFACE_MOTION` doc for the full argument.
//
// This duration is now the ONLY emphasis the summon has. The pass that
// introduced this constant also gave the Board a transform entrance
// (scale + drop); the operator rejected that on sight the same day
// ("looks weird, something about its size animation") because it scaled
// the synthetic notch cutout, which must read as fixed hardware. The
// transform is gone, this longer clock stays. The standing law, in
// full: any future entrance emphasis on the Board animates ONLY content
// below the cutout row (the below-block), NEVER the shell — the cutout
// is the one element that must never scale, translate, or fade
// independently of the hardware it impersonates. (Also recorded at
// `BOARD_SURFACE_MOTION`'s FEEL-CHECK RESULT in App.tsx, the consumer.)
//
// Numerically equal to REVEAL_MS today (both 260ms — the house "a surface
// is revealing itself" budget) but kept as its own name rather than
// reusing that constant, for the same reason HOVER_MS was split out of it:
// REVEAL_MS governs the rail's bare<->hovered PAINT coordination, this
// governs a whole-surface summon. The two are free to diverge.
export const BOARD_SUMMON_MS = 260;

// plan 148: the shared hover-disclosure spring. One config used to be
// hand-copied byte-for-byte across four call sites in two files
// (IdleHoverPeek's idle-peek below-block, AgentBoard's three expanded
// disclosures) — the "same gesture, independently tuned" drift risk this
// file exists to prevent. Values unchanged from those copies
// (ζ ≈ 0.84): the slight overshoot is deliberate, since these
// disclosures follow a hover gesture and should feel like they're being
// pulled open rather than eased open.
//
// The old `opacity: { duration: 0.15 }` per-property override that rode
// alongside it is REMOVED on purpose: a fixed opacity tween runs on its
// own clock, so an INTERRUPTED hover flip (cursor flicked away
// mid-open) desynced the two — the box still visibly collapsing after
// opacity had already hit 0 (ghost box), or arrived at height 0 while
// still partly opaque. Letting the spring drive every animated property,
// opacity included, means an interruption retargets them all on one
// clock (Apple's interruptibility rule: one animation, one clock).
// Browsers clamp opacity to [0, 1], so the spring's slight overshoot is
// harmless on that property. Do not reintroduce a per-property
// duration override here.
export const DISCLOSURE_SPRING = { type: "spring", stiffness: 480, damping: 37 } as const;

// plan 148: IdleFace's two durations, previously hand-typed at their use
// sites as bare `0.24` and `200ms` — near-misses sitting between
// existing tokens (HOVER_MS 160, REVEAL_MS 260) with nothing saying
// whether the gap was deliberate. It is: IDLE_REVEAL_MS is the face's
// whole-element entrance (opacity + scale, a one-shot mount that should
// read slower and softer than a hover response), while IDLE_GLANCE_MS is
// the eyes' own CSS `transform` transition — every gaze shift and blink
// — which has to be quicker than the entrance or the face looks sedated.
// Both values are unchanged from the literals they replace; this is
// tokenization, not a retune.
export const IDLE_REVEAL_MS = 240;
export const IDLE_GLANCE_MS = 200;
