# 166 — Give the flank's corner-radius a real entrance transition

- **Status**: DONE (2026-07-31) — `border-radius var(--expand-ms, 320ms) var(--ease-notchtap)` added to the base flank rule's transition list; the higher-specificity exit-direction rule left untouched, confirmed by both the spec-axis code review and a passing full test suite (573/573).
- **Commit**: ef91a0f
- **Severity**: MEDIUM
- **Category**: Interruptibility & timing / cohesion (review-animations skill, "one shape, one motion")
- **Estimated scope**: 1 file, 1 line added to an existing rule
- **Depends on**: plan 163 (this plan adds a real transition; without 163 the curve it uses is inert)

## Problem

The flank's bottom-outer corner-radius currently unrounds (snaps from
`8px` to `0`) with **zero transition** the instant a below-block mounts
(an idle/bare→showing promotion) — this is not a bug in the sense of
wrong code, it's confirmed and documented as the current, intended-if-
suboptimal behavior by the rule's own comment:

```css
/* src/overlay/card-chrome.css:318-327 (current) */
.card-root .card-assembly:not(:has(.below-block)) .flank-left,
.card-root .card-assembly.exiting .flank-left {
  border-bottom-left-radius: var(--card-radius, 8px);
  transition: border-radius var(--content-exit-ms, 105ms) var(--ease-notchtap);
}
.card-root .card-assembly:not(:has(.below-block)) .flank-right,
.card-root .card-assembly.exiting .flank-right {
  border-bottom-right-radius: var(--card-radius, 8px);
  transition: border-radius var(--content-exit-ms, 105ms) var(--ease-notchtap);
}
```

The comment above this rule (lines 271-282) states directly: "on entrance
this rule STOPS matching (below-block mounts, `:has()` flips), so the
un-round there has never been transitioned by this declaration — it snaps
instantly, before and after this change alike."

The mirror-image *exit* direction already received a dedicated fix for
exactly this class of problem — the "one overlapping collapse" pass
(`card-chrome.css:291-310`, wave B, 2026-07-23) explicitly unified the
exit's width shrink, content fade, and corner round-in into one
lockstep motion. The entrance direction never got the equivalent
treatment.

**Why the fix isn't "just add border-radius to the base rule's transition
list and also change the conditional rule"** — the mechanism to
understand before editing: `.card-root .card-assembly:not(:has(.below-block))
.flank-left` (and its `.exiting` sibling) has HIGHER CSS specificity than
the plain base rule `.card-root .flank-left`. Because `transition` is a
single shorthand property, whichever selector wins the cascade governs the
*entire* transition list for that element — not just the sub-properties it
mentions. Currently:

- While `:not(:has(.below-block))` matches (idle/bare, or `.exiting`): the
  conditional rule wins outright, and its own `border-radius
  var(--content-exit-ms, 105ms)` is the only active transition (the base
  rule's `background-color`/`padding` transitions are shadowed — this
  doesn't visibly break anything today because `IdleHoverPeek`'s
  `.idle-peek` below-block mounts synchronously with the `hovered` flag in
  the same React commit, so by the time background-color's value actually
  changes, `:has(.idle-peek)` has already flipped and the base rule has
  already regained control).
- The instant a below-block mounts for a real promotion, `:not(:has(.below-block))`
  stops matching entirely. Only the base (lower-specificity) rule matches
  now — which currently has no `border-radius` in its transition list, so
  there is no active transition context for that property at all, hence
  the snap.

Adding `border-radius` to the **base** rule's transition list (not
touching the conditional rule) is therefore both correct and sufficient:
during idle/bare/exiting, the higher-specificity conditional rule still
wins outright and keeps its existing, working 105ms exit-direction
transition unchanged. During the entrance/showing states (below-block
present, conditional rule no longer matching), the base rule becomes the
only rule governing `transition` — so its newly-added `border-radius` leg
is what actually applies, giving the entrance direction a real transition
for the first time, without touching or risking the already-correct exit
behavior.

## Target

```css
/* src/overlay/card-chrome.css:195-230 (current base flank rule, shown
   for context — do not restructure, only add one line to `transition`) */
.card-root .flank-left,
.card-root .flank-right {
  box-sizing: border-box;
  grid-row: 1;
  overflow: hidden;
  display: flex;
  align-items: center;
  background: #000;
  color: var(--overlay-fg);
  transition:
    background-color var(--reveal-ms, 260ms) var(--ease-notchtap),
    padding var(--reveal-ms, 260ms) var(--ease-notchtap);
  min-width: 0;
}
```

```css
/* target — add ONE new line to the existing transition list */
.card-root .flank-left,
.card-root .flank-right {
  box-sizing: border-box;
  grid-row: 1;
  overflow: hidden;
  display: flex;
  align-items: center;
  background: #000;
  color: var(--overlay-fg);
  /* plan 166: `border-radius` added to this list so the entrance
     direction (a below-block newly mounting) gets a real transition for
     the flank's own corner-round, instead of the instant snap the
     rounding-law comment above (:271-282, unchanged by this plan)
     documents. Does NOT affect the exit direction — the higher-
     specificity `:not(:has(.below-block))`/`.exiting` rule below still
     wins the whole `transition` shorthand outright whenever IT matches,
     keeping its own `--content-exit-ms` (105ms) duration exactly as
     before; this leg only ever becomes active once that rule stops
     matching (a below-block has newly mounted), which is precisely the
     entrance moment that previously had no transition context at all.
     Uses `--expand-ms` (320ms), matching the shell's own width-grow
     duration for the same promotion — so the corner-round finishes in
     step with the width, not on its own separate clock. */
  transition:
    background-color var(--reveal-ms, 260ms) var(--ease-notchtap),
    padding var(--reveal-ms, 260ms) var(--ease-notchtap),
    border-radius var(--expand-ms, 320ms) var(--ease-notchtap);
  min-width: 0;
}
```

## Repo conventions to follow

- `var(--expand-ms, 320ms)` is the existing single-sourced token
  (`EXPAND_MS`) already used by the base `.card-assembly` width transition
  for this exact same promotion moment — reusing it here keeps the corner
  round-in in lockstep with the width grow it accompanies, the same "one
  clock" principle plans 164/165 apply elsewhere in this batch.
- Do not touch the conditional rule (`card-chrome.css:318-327`) or its
  comment block — it remains the authority for the exit direction, exactly
  as documented.

## Steps

1. In `src/overlay/card-chrome.css`, locate the shared base rule for
   `.flank-left`/`.flank-right` (currently lines 195-230, the `transition`
   declaration at lines 221-223).
2. Add a third line to that `transition` list: `border-radius
   var(--expand-ms, 320ms) var(--ease-notchtap),` — insert it after the
   existing `padding` line, with the comment shown in the Target section
   (place the comment above the `transition:` declaration, replacing/
   extending whatever comment currently precedes it, or add it as a new
   block immediately above — match the file's existing comment-placement
   style for this rule).
3. Do not touch any other rule in this file, in particular do not touch
   the `:not(:has(.below-block))`/`.exiting` conditional rule at lines
   318-327.

## Boundaries

- Do NOT modify the conditional rule at `card-chrome.css:318-327` — per
  the specificity reasoning in the Problem section, it correctly continues
  to own the exit-direction transition unchanged; touching it risks
  breaking the existing, working exit choreography.
- Do NOT change `--content-exit-ms` or `--expand-ms`'s numeric values.
- Do NOT add `border-radius` to any other selector in this file.
- If the current code at `card-chrome.css:195-230` doesn't match the
  quoted excerpt (drift since commit `58cccd9`), STOP and report instead
  of improvising — in particular, re-verify the specificity relationship
  described in the Problem section still holds (i.e. the conditional rule
  at ~318-327 still has strictly higher specificity than the base rule)
  before applying this fix, since the whole approach depends on it.

## Verification

- **Mechanical**: `npx vitest run` (repo root) clean — in particular,
  re-run any test asserting on the flank's corner-radius during idle/
  exiting states (search for "radius" or "flank" in `StatusRailCard.test.tsx`)
  to confirm the exit-direction behavior is genuinely unaffected, not just
  assumed unaffected. `npx biome ci .` clean.
- **Prerequisite**: plan 163 must be applied first, or the curve this
  fix uses (`var(--ease-notchtap)`) won't resolve and no visible change
  will occur.
- **Feel check**: with plan 163 also applied, run the app and trigger a
  genuine idle/bare→showing promotion (a new notification arriving, not a
  hover):
  - Confirm the flank's bottom-outer corner now visibly rounds down
    smoothly (over ~320ms) as the card promotes, rather than snapping
    square instantly.
  - In DevTools Animations panel at 10% playback, confirm the corner
    round-in and the shell's own width grow (already bouncy per the base
    rule, unaffected by this plan) finish around the same time.
  - Confirm a showing→idle EXIT (the mirror-image direction) still behaves
    exactly as before this change — the corner should still round back up
    over the existing ~105ms window, unaffected.
- **Done when**: the base flank rule's `transition` list includes
  `border-radius var(--expand-ms, 320ms) var(--ease-notchtap)`, `npx
  vitest run` is clean (exit-direction tests unaffected), and the
  feel-check (with plan 163 applied) confirms the entrance corner-round
  now animates while the exit direction is unchanged.
