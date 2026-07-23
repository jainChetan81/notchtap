# Plan 127: overlay motion core — peek interrupt, rotation swap, tokens, hover cost, aria-live, origin, dots/chip polish

> Filed 2026-07-23 from the /improve-animations audit (findings #2, #3,
> #4, #5, #6, #7, #9 + missed opps: StatusDots flip soften, rain-chip
> fade). Authored at master `304d078`. Single-owner plan: this executor
> owns `src/components/StatusRailCard.tsx`,
> `src/components/IdleHoverPeek.tsx`, `src/overlay-card.css`,
> `src/animationTiming.ts` (+ its apply/injection module and tests),
> `src/components/StatusDots.tsx` if needed, their test files, and the
> §0 frontend count line. Do NOT touch `IdleFace.tsx` (plan 125) or
> anything under src/settings//src/components/ui (plan 126).
> Execute in the step order below — later steps build on the tokens
> from step 1.

## Step 1 — New timing tokens (finding #4 groundwork)

`src/animationTiming.ts`: add, with doc comments in the file's voice:
- `export const REVEAL_MS = 260;` — the bare↔hovered rail
  reveal/paint coordination duration (today hand-typed as 260ms/0.26
  across overlay-card.css:214-215/:1182 and StatusRailCard.tsx:439/:504).
- `export const HOVER_MS = 160;` — hover-breathe response (new; see
  Step 4).
- `export const ROTATION_EXIT_MS = 70;` and
  `export const ROTATION_ENTER_MS = 120;` — same-slot content
  rotation swap (see Step 3).
Inject all four as CSS vars (`--reveal-ms`, `--hover-ms`,
`--rotation-exit-ms`, `--rotation-enter-ms`) wherever the existing
`--swap-exit-ms`/`--content-exit-ms` injection happens, and extend the
existing lockstep test that guards constant↔CSS-var parity.

Then retire the literals (finding #4): overlay-card.css `:214-215`
(flank background/padding) → `var(--reveal-ms, 260ms)`; `:1182`
(track span background) → same; StatusRailCard.tsx `:439`/`:504`
`duration: 0.26` → `REVEAL_MS / 1000`, and (finding #9) their
`ease: "easeOut"` → `ease: NOTCHTAP_EASE` — these fades are coupled to
the flank paint that already runs `--ease-notchtap`; unify. The
plan-124 exit-to-bare rules keep `--swap-exit-ms` — untouched.

## Step 2 — Peek survives a promotion (finding #2, structural)

`StatusRailCard.tsx:516`:
`{!renderedShowing && <IdleHoverPeek status={status} hovered={hovered} />}`
unmounts the component together with the internal `AnimatePresence`
that owns the peek's exit — so a promotion arriving while the peek is
open tears out up to 100px of content with zero animation.

Fix: always render `<IdleHoverPeek status={status}
hovered={hovered} open={!renderedShowing && hovered} />` and move the
gating INSIDE `IdleHoverPeek`: its `AnimatePresence` condition
(currently `hovered ?` at `IdleHoverPeek.tsx:~256`) becomes the new
`open` prop. Keep the component's internal close-delay/reduced-motion
logic keyed off the same signal it uses today, just sourced from
`open` instead of `hovered` where the mount gate is concerned (hover
styling/props that read `hovered` for other reasons stay on
`hovered`). Result: a promotion flips `open` false and the existing
`exit={{ height: 0, opacity: 0, ... }}` collapse PLAYS while the card
content enters above it.

Constraints: the existing tests "the peek never mounts while a card is
showing" and the mount/close-delay lifecycle tests must stay green
(the DOM condition they assert is unchanged — only who evaluates it
moved). hover.rs is untouched (rust derives peek-open from its own
hover state, not the DOM). Add a test: with the peek open, promote a
card → the `.below-block.idle-peek` element is STILL present on the
promotion render (exit playing), gone after the exit window; the card
content mounts regardless.

## Step 3 — Lighter same-slot rotation (finding #3)

`StatusRailCard.tsx:549-565` (inner `AnimatePresence mode="wait"`
keyed `swapKey`): every news rotation (~10s) chains a 105ms opacity
exit THEN a 175ms `y:-4` slide enter — ~280ms of ceremony many
times/hour. Promotions (idle→showing) and exits (showing→idle) keep
today's exact feel; only showing→showing rotations lighten.

Mechanism: track the previous `swapKey`'s showing-ness in a ref
(`wasShowingRef`), compute
`const isRotation = showing && wasShowingRef.current` on the render
where the key changes, and pass it through AnimatePresence's
`custom` prop / variant functions (or equivalent — motion re-reads
`custom` for exits when set on AnimatePresence itself) so that:
- rotation exit: `{ opacity: 0 }`, `ROTATION_EXIT_MS / 1000`,
  `NOTCHTAP_EASE`, no y.
- rotation enter: `{ opacity: [0, 1] }`, `ROTATION_ENTER_MS / 1000`,
  `NOTCHTAP_EASE`, NO `y` offset (the slide is the part that reads as
  ceremony on repeat).
- non-rotation legs: byte-identical to today (105/175, y:-4 enter).
Update `wasShowingRef` after computing. Pin with tests: a
showing→showing key change uses the rotation timings (assert via
motion props or a data-attribute the component sets — pick the
sturdier pattern already used in this test file); idle→showing and
showing→idle assertions stay byte-untouched.

Also `overlay-card.css:1571-1582` (`pill-enter`): the chip/time
stagger replays every rotation. Change the keyframe to opacity-only
(drop `transform: translateY(3px)`) and remove the
`animation-delay: calc(var(--swap-exit-ms) * 0.32)` stagger — fade
duration stays `var(--swap-exit-ms, 175ms)`. Update the css comment.

## Step 4 — Hover breathe: faster, and stop animating filter (finding #5)

`overlay-card.css:103-106`: the base transition list animates
`transform 260ms` and `filter 260ms`; `:715-719` (`.hovered`) scales
1.02 AND deepens `drop-shadow` blur 18px→26px — a whole-card
re-rasterize per frame, both directions, on a many-times/day hover.
- `transform 260ms var(--ease-notchtap)` → `transform
  var(--hover-ms, 160ms) var(--ease-notchtap)` (hover response inside
  the 125-200ms budget).
- REMOVE `filter` from every transition list (`:106`, and the exit
  redeclaration `:351-352` — keep its `transform` leg, tokenized to
  `var(--reveal-ms, 260ms)` there since it mirrors the reveal, per
  the comment at `:349`).
- REMOVE the `.hovered` filter override entirely: the base
  `drop-shadow` (`:86`) stays static; the 1.02 scale alone carries the
  breathe. Update the rule's design comment (it currently argues for
  the deepened shadow — record the audit tradeoff: animated filter
  re-rasterizes the full ~500px composite; static shadow + scale
  reads nearly identical). FEEL-CHECK flag for the operator.
The plan-124 CSS convergence string-pins must stay green — none of
these rules feed `.bare` values, but run the suite and confirm.

## Step 5 — Scope the live region (finding #6)

`StatusRailCard.tsx:381-382`: `role="status"`/`aria-live="polite"`
sit on the card ROOT, so FlankClock's minute tick and live-score
chrome can re-announce while a card shows. Move both onto a STATIC
wrapper that encloses ONLY the notification content region (the
below-block/content area that carries title/body — NOT the
AnimatePresence-keyed node itself, which remounts per swap and would
re-announce; the wrapper must be a stable element outside the keyed
child). Clock, dots, and scorecard chrome end up outside the region.
Repoint (don't weaken) any test querying `role="status"`.

## Step 6 — Celebration origin (finding #7)

`overlay-card.css:785-787` (`.pulse-goal` → `goal-overshoot`): the
whole-card scale has no `transform-origin`, so it grows from center,
lifting the top edge off the notch — while `:717` pins
`transform-origin: top center` for the hover breathe with a comment
explaining the physical anchor. Add `transform-origin: top center;`
to the `.pulse-goal` rule (and to any sibling whole-card pulse rule
with the same gap — check `.pulse-*`/`.cele-*` rules that scale
`.card-assembly` itself; pseudo-element bursts/rings stay
center-origin, they're explosions). Also unify the celebration
layers' easing split (audit F8): `goal-burst` `:790` and `goal-ring`
`:809` `ease-out` → `var(--ease-notchtap)`. Durations untouched
(operator-tuned, plan 100).

## Step 7 — Dots flip + rain chip (missed opps)

- `overlay-card.css:1362-1440`: `.status-dot` has no transition — a
  pause/config flip snaps glow, opacity, and circle↔square shape. Add
  to the base `.status-dot` rule:
  `transition: opacity var(--hover-ms, 160ms) var(--ease-notchtap),
  box-shadow var(--hover-ms, 160ms) var(--ease-notchtap),
  border-radius var(--hover-ms, 160ms) var(--ease-notchtap);`
  This is a state-FLIP soften, not a loop — the operator's removed
  dot-pulse decision (plan 105) is about loops and stays respected;
  say so in the comment. Give `.pause-glyph` a matching opacity
  fade-in (CSS-only if it mounts via class toggle; if it's a React
  mount, a 160ms one-shot opacity keyframe is acceptable here since
  it's rare and self-ending).
- `IdleHoverPeek.tsx:115-117`: the rain chip mounts/unmounts bare
  when `rainPct` crosses null mid-peek. Wrap in the component's
  existing `AnimatePresence` idiom (or a sibling one,
  `initial={false}`) with a 120ms opacity fade, `NOTCHTAP_EASE`.
  `initial={false}` so peek-open renders the chip instantly with the
  container spring, no double animation.

## Verification

`npx tsc --noEmit`, `npx vitest run` (including the plan-124
convergence pins and every existing choreography test — green
UNMODIFIED except explicitly-sanctioned selector repoints in Steps 2/5),
`npx biome ci .`, `npx vite build`. §0 frontend line only. Feel checks
owed to the operator (flag in report): hover breathe without the
shadow deepen, rotation lightness, peek collapse under a promotion,
dot flips.

## STOP conditions

- Step 2: the mount-gate move can't keep the "never mounts while
  showing" contract without weakening it.
- Step 3: motion's custom/variant plumbing can't distinguish the legs
  reliably (report; do not ship a rotation change that also alters
  promotion/exit feel).
- Step 4: removing the filter transition breaks any string-pin.
- Any existing test needs weakening anywhere.
