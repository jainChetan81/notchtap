# Plan 123: exit-to-bare continuous morph (kill the "box, then rounded shape pops in")

> Filed 2026-07-23 (operator bug: in minimal/notch resting mode, a
> card's exit shows a wide rounded box shrinking, then the bare curved
> notch shape APPEARS in a discrete pop). Drift check: authored
> against master `7ee95d3`. Re-verify citations if
> `src/useExitChoreography.ts`, `src/components/StatusRailCard.tsx`,
> or `src/overlay-card.css` (regions ~:100-520) changed since.

## Mechanism (verified — this is coded behavior, not a glitch)

`.exiting` and `.bare` are mutually exclusive by construction
(`useExitChoreography.ts:99` `shellExiting = !showing &&
renderedShowing`; `:114` `bare = restingState==="notch" &&
!renderedShowing && !exiting`). During the exit window
(t=0..SWAP_EXIT_MS=175ms):

- `.card-assembly.exiting` animates `--cw` to the WIDE idle/rail
  width (`overlay-card.css:347-353`, same formula as `.idle` `:116`),
  and rounding lives on the FLANKS
  (`.exiting .flank-{left,right}` bottom radii, `:310-319`).
- At t=175ms the swap settles: `.exiting` clears, `.bare` mounts in
  the same render. Flanks instantly go transparent/`padding:0` and
  collapse (`:135-159`), `--cw` drops to cutout-only (`:132-134`),
  and rounding duty hands off to `.synthetic-cutout` — which was
  SQUARE until that instant (`:501-504` applies only under `.bare`).

Result: shape morphs on one element, then jumps to a different
element with no transition bridging them.

## Fix

Make the exit converge on the bare geometry BEFORE the class flip, so
the flip is visually a no-op. Notch-resting-mode only — rail-mode
exit behavior must stay byte-identical.

1. **useExitChoreography.ts** — return one new derived boolean:
   `exitToBare = shellExiting && restingState === "notch"`. Document
   it in the same comment style as its siblings (this hook is a
   design-record file; keep the voice). This stays plain
   derived-state + CSS — plan 107's contract (geometry choreography
   never moves into the motion library) applies.
2. **StatusRailCard.tsx** — apply class `exit-to-bare` on the
   assembly alongside `exiting` (`:232-236` region, same pattern as
   `bare`/`exiting`).
3. **overlay-card.css** — `.card-assembly.exiting.exit-to-bare`
   overrides, scoped so plain `.exiting` (rail mode) is untouched:
   - `--cw` target = the BARE formula
     (`min(var(--notchtap-cutout-width,200px),100%)`, `:132-134`) —
     the shell shrinks straight to cutout width instead of the wide
     rail width, over the same `var(--swap-exit-ms,175ms)`.
   - Flanks: transition to transparent + collapsed within the window
     (opacity/padding transition ≤175ms) instead of holding painted.
   - `.synthetic-cutout`: transition its bottom radii to the bare
     values (`var(--card-radius,14px)` /
     `min(calc(var(--card-radius,14px)*2),20px)`, `:501-504`) during
     the window, so at t=175ms it already IS the bare shape.
   - Gills (`.notch-gill`, `:436-468`): decide whether they fade in
     during the window or appear at flip; either is acceptable if
     deliberate — leave a one-line comment stating the choice. No
     pop of the main silhouette is the bar.
   - At t=175ms every property the `.bare` rules set must equal what
     the exit-to-bare transition just animated to (width, flank
     paint/padding, cutout radii) — enumerate them in a CSS comment
     so the invariant is checkable.
4. **Tests** — extend the existing fake-timer choreography coverage
   (StatusRailCard.test.tsx "compact->idle geometry" describe block):
   notch resting mode → during exit window assembly has
   `exiting exit-to-bare` and NOT `bare`; after timers run out,
   `bare` present, both exit classes gone. Rail mode → assert
   `exit-to-bare` NEVER appears; existing rail-mode exit assertions
   stay byte-untouched.

## Rust mirror check (do first, cheap)

`src-tauri/src/hover.rs` mirrors card-rect geometry. Read its width
formulas before touching CSS: if it models the exit window's width at
all, shrinking the visual to cutout width while the rust rect stays
rail-wide leaves an oversized hover rect for ≤175ms — conservative
and acceptable (document it in the plan-done note); but if hover.rs
would DESYNC in the dangerous direction (rect smaller than visual),
STOP. Do not edit hover.rs in this plan.

## Non-goals

- No motion-library involvement (plan 107 contract).
- No timing-constant changes (`SWAP_EXIT_MS`/`CONTENT_EXIT_MS` stay).
- Rail-mode exit: zero behavior change.
- Don't touch the peek CSS region (~:1550-1950) — plan 122 owns it.

## Verification ladder

`npx tsc --noEmit`, `npx vitest run`, `npx biome ci .`, `npx vite
build`. Update `docs/TESTING_STRATEGY.md` §0 counts. Final smoothness
is operator-verified on device (flag it — the repo's animation bug
class is desynced clocks; every duration here must come from the
existing CSS vars, no new literals).

## STOP conditions

- hover.rs desync in the dangerous direction (see above).
- The bare-vs-exit-end geometry can't be made to match exactly for
  any property (name it and stop rather than shipping a smaller pop).
- Any test in the rail-mode exit path needs weakening.
