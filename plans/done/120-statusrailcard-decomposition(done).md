# Plan 120: Decompose StatusRailCard.tsx (useExitChoreography hook + content-branch components)

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report — do not improvise a
> workaround. Do NOT push/merge/PR. Do NOT edit `plans/README.md` — the
> reviewer maintains it. Do NOT touch `src-tauri/**` — this plan is
> frontend-only.
>
> **Drift check (run first)**:
> `git diff --stat 2a840c4..HEAD -- src/components/StatusRailCard.tsx src/components/StatusRailCard.test.tsx src/overlay-card.css src/animationTiming.ts src/useDelayedSwap.ts src/App.tsx`
> Line numbers below are anchors verified against `2a840c4`, not gospel —
> re-grep each symbol before editing; on a named function/variable no
> longer existing at the stated location, STOP and re-anchor by symbol
> name before proceeding.
>
> **Concurrent-session hazard (real, not hypothetical)**: this repo's own
> history shows a concurrent session's branding commit (`b2b9627`)
> accidentally swept an in-progress decomposition mid-flight during plan
> 119, requiring a restore commit (`e568814`). `src/animationTiming.ts`
> and `src/overlay-card.css` are the two files most likely to have a
> *second* animation-work session actively mutating them (choreography
> timing constants, `.card-assembly.exiting` transition durations, the
> `:has(.below-block)` rounding-law selectors) while this plan runs. If
> the drift-check diff above shows **any** change to either file relative
> to `2a840c4`, or if `SWAP_EXIT_MS`/`CONTENT_EXIT_MS` in
> `animationTiming.ts` no longer read `175`/`105`, STOP — do not merge
> your extraction on top of a moving timing target; report the drift and
> wait for redispatch at a fresh baseline.

## Status

- **Priority**: P3 (pure maintainability — zero user-visible change)
- **Effort**: M
- **Risk**: MED (animation-adjacent — the file being split owns the
  showing/idle exit choreography; the repo's own animation memory names
  "desynced clocks," not tech choice, as the actual bug class here, so a
  mechanical move that silently changes *when* a hook re-evaluates
  relative to render is the specific failure mode to guard against, not
  a hypothetical)
- **Depends on**: none. Must NOT run concurrently with any other
  animation-timing work touching `src/animationTiming.ts`,
  `src/overlay-card.css`, `src/useDelayedSwap.ts`, or
  `src/components/StatusRailCard.tsx` — see the concurrent-session
  hazard above.
- **Category**: tech-debt
- **Planned at**: commit `2a840c4`, 2026-07-23

## Why this matters

`src/components/StatusRailCard.tsx` is 861 lines (verified by direct
read at `2a840c4`) — one function (`StatusRailCard`, `:106-861`) that
owns three genuinely separate concerns stacked on top of each other:
(1) the exit-choreography state machine that decides *when* the shell's
geometry/idle-furniture classes flip during a showing→idle transition,
(2) per-event-type content rendering (the live-match scorecard vs. the
news/general compact+manifest body), and (3) the JSX composition that
wires both together with the rail furniture (`FlankClock`, `StatusDots`,
`IdleHoverPeek`, `IdleFace`).

This is exactly the shape the thermo-nuclear code-quality review's
finding 8 named. The review's own text does not survive as a standalone
log (unlike plan 112's, `docs/review-logs/2026-07-22-plan-112-*.md`, or
plan 107's from `2026-07-22-plans-106-111.md`) — the only committed
record of finding 8 is the reservation row `plans/README.md:294` and the
reconciliation note directly below it, both landed in `5fca453`:

> `120 | Decompose StatusRailCard.tsx (useExitChoreography hook +
> content-branch components) | P3 | M | write AFTER the minimal-notch
> work merges (file is being reshaped) | TODO — reserved 2026-07-23
> (thermo-nuclear finding 8); plan text deliberately not written yet so
> its excerpts are cut against the post-minimal-notch file.`
>
> **Thermo-nuclear review reconciliation (2026-07-23):** ... FILED as
> plans: 119, 120 (decompositions).

The reservation's own gate — "write AFTER the minimal-notch work merges
(file is being reshaped)" — is now satisfied: the minimal-notch spec
(commits `f8a7b1f` merge, folding in the `.bare`/`railRevealed`
rework and the Switch restyle) landed before `2a840c4`, and this plan's
excerpts below are cut against the file as it exists post-merge, not the
pre-minimal-notch shape the reservation deliberately avoided citing.

Unlike plan 119 (SettingsApp.tsx, a flat god-file of independent
sections with no shared runtime state), StatusRailCard.tsx's three
concerns are *not* independent: the choreography hook's outputs
(`renderedShowing`, `expanded`, `bare`, `railRevealed`, `trueIdle`) are
consumed by essentially every other part of the render. This plan is
still test-neutral by construction (see Scope) — but the risk sits in
correctly threading hook outputs across a file boundary without
changing *which render* each value is read on, since that timing is the
whole point of `useDelayedSwap`'s freeze window.

## Current state (verified by direct read at `2a840c4`)

`src/components/StatusRailCard.tsx`, 861 lines total:

- **`:1-27`** imports.
- **`:35-43` `Crest`** — a small component (converts a filesystem crest
  path via `convertFileSrc`, sticky `broken` fallback to the text
  abbrev). Used ONLY inside the live-match branch (`:687`, `:695`).
- **`:52-56` `CELEBRATION_END_ANIMATION`** — `Record<Celebration,
  string>` mapping each live-match celebration class to the CSS
  `animationend` name that clears it. Read in TWO places: the
  live-match effect that sets `liveCelebration` (`:165-172`, uses
  `footballEventKindFor`/`eventKindPresentationFor` directly, NOT this
  map) and `clearPulseWhenItsAnimationEnds` (`:174-181`, the
  `onAnimationEnd` handler on the root `.card-assembly` — reads this map
  at `:178`). Because the handler lives at the `StatusRailCard`
  component level (bound to the root div, `:502`), this map must stay
  reachable from there — it does NOT move into the live-match content
  component even though it's celebration-specific.
- **`:58-83`** `isWxMarker`, `Detail` type, `visibleDetails`,
  `weatherArtFromDetails` — pure helpers for the weather-alert
  marker-leak guard (plan 082). Used at `:189` (`wxArt`) and `:405`
  (`liveVisibleDetails`), both StatusRailCard-level `const`s consumed in
  JSX outside any single content branch (`wxArt` feeds both
  `belowBlockClass`, `:378`, and the `wx-icon` `<img>`, `:623`, which
  sits as a JSX sibling to BOTH content branches, not inside either).
  **Out of scope for this plan** — see Scope below; noted here only so
  the executor doesn't mistake them for orphaned code to sweep up.
- **`:96-104`** `Pulse` type + `PULSE_END_ANIMATION` — same
  shape/reachability constraint as `CELEBRATION_END_ANIMATION` above
  (read by the same root-level `onAnimationEnd` handler); stays at
  `StatusRailCard` level.
- **`:106-861` `StatusRailCard`** — the target function. Internal shape,
  in render order:
  - **`:126-136`** live-slot derived flags: `showing`, `currentId`,
    `currentSignal`, `currentBody`, `news`, `isLiveCard`.
  - **`:138-181`** the pulse/celebration state machine: `pulse` state +
    effect (`:138-154`), `liveCelebration` state + effect
    (`:156-172`), `clearPulseWhenItsAnimationEnds` handler
    (`:174-181`). **Not part of this plan's hook extraction** — this is
    signal-triggered animation state (goal/red-card pulse, live-match
    celebration), orthogonal to the showing↔idle exit choreography
    below. Leave in `StatusRailCard.tsx`.
  - **`:183-195`** `wxArt`, `cmuxOrigin` — live-slot-derived, feed
    `belowBlockClass`/the wx icon. Out of scope (see above).
  - **`:196-332` — THE EXIT-CHOREOGRAPHY BLOCK.** This is the hook
    extraction target, verified symbol-by-symbol against `2a840c4`:
    - **`:208-210`**: `swapKey = showing ? slot.id : "idle"`;
      `{ value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey,
      SWAP_EXIT_MS)`; `renderedShowing = renderedSlot.state ===
      "showing"`.
    - **`:226`**: `belowBlockOpen = showing && renderedShowing`.
    - **`:246-251`**: `geometryPriority` — showing→live `slot.priority`;
      exiting (showing false but `renderedShowing` true)→
      `renderedSlot.priority`; else `"idle"`. `expanded` — showing→live
      `slot.expanded`; else `renderedShowing && renderedSlot.expanded`
      (a boolean, not tri-state, unlike `geometryPriority`).
    - **`:274`**: `shellExiting = !showing && renderedShowing` — the
      2026-07-23 wave-B addition (`.exiting` CSS class), deliberately
      narrower than raw `exiting` from the hook (which also fires
      during idle→showing promotions and must NOT drive this).
    - **`:289`**: `bare = restingState === "notch" && !renderedShowing
      && !exiting` — **bare-mode gating**: bare is true only in notch
      resting mode, AND the delayed swap has fully settled to idle
      (`!renderedShowing`), AND no exit animation is still in flight
      (`!exiting`). A still-exiting prior card finishes its normal exit
      before the shell ever goes bare.
    - **`:306`**: `railRevealed = !bare || hovered` — **the minimal-notch
      spec's reveal gate** (2026-07-23, folded in at `f8a7b1f`): true
      whenever NOT bare, or bare-but-hovered. `bare` is already false
      for the entire showing/exiting window (per its own definition
      above), so `railRevealed` is true throughout that window
      unconditionally — the `hovered` half of the OR only matters while
      genuinely bare-and-idle. This is what makes **dots persist
      through showing**: `railRevealed` (not `!bare` alone, and NOT
      `!renderedShowing`) is what gates both `FlankClock` (`:536`) and
      `StatusDots` (`:590`) now — the operator-requested "one continuous
      rail shape" behavior, not the pre-minimal-notch behavior where a
      showing card hid the dots.
    - **`:315`**: `trueIdle = !showing && !renderedShowing && !exiting
      && !hovered` — gates `IdleFace` (`:564`); the delayed-swap-settled
      basis, NOT live `showing` alone, so the face doesn't flash back
      mid-exit.
    - **`:332`**: `idleFaceEligible = useMemo(() =>
      presentationFacts().mode !== "notch", [])` — a boot-time-global
      read, memoized once (`[]` deps), gating whether `<IdleFace>`
      mounts AT ALL (real notch hardware never mounts it, so its
      internal timers never arm).
    - Also present in this block but **derived from the block's own
      outputs, not independent inputs** — `cardClass` (`:343-369`) and
      `belowBlockClass` (`:374-384`) consume `geometryPriority`,
      `expanded`, `bare`, `shellExiting` alongside signal-state
      (`pulse`/`liveCelebration`/`hovered`) and content-state
      (`news`/`wxArt`/`cmuxOrigin`). These two class-list `const`s
      themselves STAY in `StatusRailCard.tsx` (they mix
      choreography-hook outputs with signal-state the hook doesn't own)
      — only their choreography-sourced inputs move into the hook.
  - **`:398-432`** content-derivation `const`s used only inside the two
    content branches: `newsCategory`, `newsAge`, `liveVisibleDetails`
    (`:398-405`); `bodyContent` (`:415`, `useMemo` keyed on
    `currentBody`); `liveEspn`, `footballKind`, `eventPresentation`,
    `pillVariant`, `pillLabel`, `cardsClean` (`:422-432`, live-match-only).
  - **`:497-861`** the JSX return. Structure:
    - `:498-503` root `.card-assembly` div (class list, role/aria-live,
      `onAnimationEnd`).
    - `:518-519` notch-gill decorative spans.
    - `:520-548` `.flank-left` — `FlankClock`, `AnimatePresence`-gated
      on `railRevealed`.
    - `:555` `.synthetic-cutout`.
    - `:564` `IdleFace` (gated `idleFaceEligible`, prop `trueIdle`).
    - `:565-603` `.flank-right` — `StatusDots`, `AnimatePresence`-gated
      on `railRevealed`.
    - `:609` `IdleHoverPeek` (gated `!renderedShowing`).
    - `:610-845` — the `.below-block` `AnimatePresence` (gated
      `belowBlockOpen`), containing:
      - `:623` the `wx-icon` `<img>` (wxArt-gated, sibling to both
        branches below — stays in `StatusRailCard.tsx`, out of scope).
      - `:642-841` — inner `AnimatePresence mode="wait"`, the actual
        content swap (gated live `showing`), containing the **two
        content branches** this plan's second extraction target:
        - **`:660-710` LIVE-MATCH SCORECARD BRANCH**
          (`isLiveCard && liveEspn !== undefined`) — `.notif-block`:
          league chip, live/break/final pill + clock, crests + score,
          event line, cards line. No `Track`, no `Manifest`, no
          `TtlBar` (operator-locked sticky presence, per the branch's
          own comment `:661-673`).
        - **`:711-838` NEWS/GENERAL COMPACT + MANIFEST + TTLBAR
          BRANCH** (the `else` arm) — a fragment: `.compact` div
          (`:713-811`, itself branching `news ? ... : ...` at
          `:715/755`), `<Manifest>` (`:812-822`), `<TtlBar>`
          (`:827-837`).
    - `:846-858` the goal-pulse ripple (`.cele-ripple`, gated
      `!isLiveCard && pulse === "pulse-goal"`).

- **`src/components/StatusRailCard.test.tsx`**, 1517 lines: imports
  ONLY `{ StatusRailCard }` from `"./StatusRailCard"` plus wire types
  from `"../useSlotState"`/`"../useStatusState"`. No import of any
  internal helper, the hook, or either content branch — because none of
  those are exported today. This is stronger than plan 119's
  SettingsApp.test.tsx guarantee (which only promised import-path
  parity): **zero test-file changes are required by this plan, not even
  import paths**, because `StatusRailCard`'s own export path
  (`src/components/StatusRailCard.tsx`, named export `StatusRailCard`)
  does not move. See Done criteria.
  - The `"compact->idle geometry as one state machine (plan 107 Step
    B)"` describe block (`:1387-1516`) is the load-bearing regression
    suite for the exact choreography values this plan extracts —
    `SWAP_EXIT_MS` (175ms) timing, `.high`/`.expanded`/`.idle`/`.exiting`
    class transitions at t=0/174/175. These tests must pass **unchanged**
    after extraction; if the hook's timing relative to render changes by
    even one tick, this block is where it will show first.
- **`src/App.tsx:3,136`** — the sole consumer:
  `<StatusRailCard slot={slot} status={status} restingState=
  {restingState} hovered={hovered} />`. Named-export import only; no
  other symbol from this file is imported anywhere else in the repo
  (verified: `grep -rn "StatusRailCard" src --include='*.tsx'
  --include='*.ts'` outside the component/test files themselves returns
  only `App.tsx`).
- **`src/overlay-card.css`** (2121 lines) — the load-bearing selectors
  this plan's className list must not disturb: `.card-assembly`,
  `.card-assembly.idle/.expanded/.bare/.exiting`,
  `.card-assembly.bare:has(.idle-peek)`,
  `.card-assembly.bare.hovered`, `.flank-left`/`.flank-right`,
  `.card-assembly:not(:has(.below-block))` / `.card-assembly.exiting`
  (the two OR'd triggers for the ROUNDING LAW, `:234-303`), `.notch-gill`
  and its `data-notchtap-mode="hud"` variants (`:419-449`),
  `.synthetic-cutout`, `.idle-face*`, `.below-block`. None of these are
  edited by this plan — the decomposition must reproduce every one of
  them byte-for-byte on the same elements.
- **`src/animationTiming.ts`**: `SWAP_EXIT_MS = 175` (`:50`),
  `CONTENT_EXIT_MS = 105` (`:81`), `NOTCHTAP_EASE` (`:90`) — read via
  `useDelayedSwap(slot, swapKey, SWAP_EXIT_MS)` inside the choreography
  block and via the `motion.div` `transition.duration` props in the two
  content branches. Values only; this plan does not touch this file.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Install | `npm ci` | exit 0 |
| Tests | `npx vitest run` | all pass; StatusRailCard.test.tsx byte-unchanged and green; total count unchanged (this plan adds/removes no test cases — see the temporary parity harness below, which must NOT remain in the final count) |
| Typecheck | `npx tsc --noEmit` | 0 |
| Lint (enforcing gate — CLAUDE.md: not interchangeable with the scoped form) | `npx biome ci .` | 0 |
| Build | `npx vite build` | 0 |

## Scope

**In scope**: `src/components/StatusRailCard.tsx` (shrinks), one new
hook file `src/useExitChoreography.ts` (sibling of `src/useDelayedSwap.ts`
— hooks live at `src/` root in this repo, not under `components/`; see
`useSlotState.ts`, `useStatusState.ts`, `useDelayedSwap.ts`,
`prefersReducedMotion.ts`), two new component files under
`src/components/` (flat, matching the existing convention —
`FlankClock.tsx`, `IdleFace.tsx`, `Manifest.tsx`, `Stamp.tsx`,
`StatusDots.tsx`, `Track.tsx`, `TtlBar.tsx` are all flat siblings of
`StatusRailCard.tsx`, not nested in a subfolder): `LiveMatchScorecard.tsx`
and `NotificationBody.tsx` (naming rationale in Step 2). A temporary,
NOT-committed-at-the-end parity test (Step 0/3).

**Out of scope**: `src-tauri/**` (untouched); `src/overlay-card.css`
(zero edits — every className this plan moves must already exist there
unmodified); `src/animationTiming.ts` (values read-only); `src/App.tsx`
(its one call site is unaffected — `StatusRailCard`'s public props
don't change); `Crest`'s behavior, the pulse/celebration state machine
(`:138-181`), `wxArt`/`cmuxOrigin`/the wx-icon `<img>`, and the
`isWxMarker`/`visibleDetails`/`weatherArtFromDetails` helpers — all
explicitly left in `StatusRailCard.tsx` per the Current State notes
above (they're either shared across both content branches, bound to the
root-level `onAnimationEnd` handler, or otherwise not cleanly owned by
either extraction target); every existing component this file already
composes (`FlankClock`, `IdleFace`, `IdleHoverPeek`, `Manifest`,
`Stamp`, `StatusDots`, `Track`, `TtlBar`) — imported, not touched.

## Steps

### Step 0: Baseline parity harness (write BEFORE any production edit)

Add a temporary test file `src/components/StatusRailCard.parity.test.tsx`
(explicitly scaffolding — deleted in Step 4, never part of the permanent
suite). It must render `StatusRailCard` across every fixture already
defined in `StatusRailCard.test.tsx` — reuse the file's own fixtures by
value (copy the literal objects; do not import test-only fixtures across
files) — plus the choreography checkpoints the existing "compact->idle
geometry" describe block already exercises:

- Static fixtures (one `it` per row, `expect(container.innerHTML).
  toMatchSnapshot()`): `GOAL`, `RED_CARD`, `CMUX_NEEDS_INPUT`,
  `LIVE_MATCH`, `NEWS`, `{ ...NEWS, expanded: false }`, `CMUX_RICH`,
  `{ ...CMUX_RICH, expanded: false }`, `WEATHER_ALERT`, `liveSlot()`,
  `liveSlot({ expanded: true })`, `liveSlot({ signal: "yellow_card",
  body: "Yellow Card — B. Saka 54'", espn: { ...ESPN_BASE, homeCards:
  [1, 0], awayCards: [2, 0] } })`, `{ state: "empty" }` with
  `restingState="rail"`, `{ state: "empty" }` with `restingState="notch"`
  hovered `false`, `{ state: "empty" }` with `restingState="notch"`
  hovered `true`.
- Choreography-timing checkpoints (fake timers, mirroring
  `StatusRailCard.test.tsx:1414-1461`): render `GOAL`, rerender to
  `{ state: "empty" }`, snapshot `container.innerHTML` at t=0 (before
  any `advanceTimersByTime`), t=174ms, and t=175ms (post-swap).

Run `npx vitest run src/components/StatusRailCard.parity.test.tsx -u`
to write the baseline `.snap` file. **Verify**: the snapshot file now
exists under `src/components/__snapshots__/`; `git diff --stat` shows
only the two new files (test + snapshot), zero production changes yet.
Do not commit — this plan produces no commits itself, but keep both
files in the working tree through Steps 1-3.

### Step 1: Extract `useExitChoreography`

New file `src/useExitChoreography.ts`. Move (not copy — delete the
originals from `StatusRailCard.tsx` in the same edit) the block
identified in Current State as `:196-332` minus the two `const`s that
stay behind (`cardClass`, `belowBlockClass` — those consume the hook's
outputs alongside non-choreography state and remain call-site-local).
Concretely, the hook owns:

```
function useExitChoreography(
  slot: SlotState,
  restingState: "rail" | "notch",
  hovered: boolean,
): {
  renderedSlot: SlotState;
  exiting: boolean;
  renderedShowing: boolean;
  belowBlockOpen: boolean;
  // `Priority` (src/useSlotState.ts:29) is a local, NOT exported, type —
  // don't add an export just for this signature. Either inline
  // `Extract<SlotState, { state: "showing" }>["priority"] | "idle"`, or
  // let TS infer the return type and skip an explicit annotation
  // entirely (the hook body already produces the correct union at
  // StatusRailCard.tsx:246-250 — inference reproduces it exactly).
  geometryPriority: Extract<SlotState, { state: "showing" }>["priority"] | "idle";
  expanded: boolean;
  shellExiting: boolean;
  bare: boolean;
  railRevealed: boolean;
  trueIdle: boolean;
  idleFaceEligible: boolean;
}
```

Preserve every comment verbatim (they are load-bearing design record,
not filler — plan 107/105/minimal-notch attribution lines must move
with the code they document, not get deleted as "just comments"). The
hook takes `showing = slot.state === "showing"` as an internal
derivation (recompute it inside the hook from `slot`, matching
`StatusRailCard.tsx:126` exactly — do not thread `showing` in as a
separate parameter, since `slot` alone determines it and threading both
risks them drifting apart across the boundary). `StatusRailCard.tsx`
calls the hook once, near the top of the function body where
`:196-332` used to start, and destructures its return into the same
local names the rest of the function already references
(`renderedSlot`, `exiting`, `renderedShowing`, `belowBlockOpen`,
`geometryPriority`, `expanded`, `shellExiting`, `bare`, `railRevealed`,
`trueIdle`, `idleFaceEligible`) — this is a mechanical rename-free move;
every downstream reference in `cardClass`/`belowBlockClass`/the JSX
keeps its existing identifier.

**Verify**: `npx tsc --noEmit` clean (catches any accidental type
narrowing lost across the function boundary); `npx vitest run
src/components/StatusRailCard.test.tsx` green, all cases, byte-unchanged
file; `npx vitest run src/components/StatusRailCard.parity.test.tsx`
— zero snapshot diffs.

### Step 2: Extract the two content-branch components

Both extractions are **verbatim JSX moves**: the moved component
receives every external variable its JSX block references as a prop —
it does NOT re-derive any of them internally. This is deliberately the
lower-risk shape (matching plan 119's "moved as-is" discipline) over
having each component recompute its own inputs from `slot`, which would
change *where* a computation runs relative to render and reopen exactly
the timing-drift risk this plan exists to avoid introducing.

1. **`src/components/LiveMatchScorecard.tsx`** — move `Crest` (`:35-43`,
   its only consumer) and the JSX at `:660-710` verbatim. Props: every
   free variable the JSX block reads — `liveEspn: EspnMeta`
   (non-null; the call site already guards `liveEspn !== undefined`
   before rendering this component), `pillVariant`, `pillLabel:
   string`, `eventPresentation: EventKindPresentation | null`,
   `cardsClean: boolean`, `body: string` (was `slot.body` at `:702`).
   `CELEBRATION_END_ANIMATION` does **not** move (see Current State —
   it's read by the root-level `onAnimationEnd` handler, not by this
   branch's own JSX). The `liveEspn`/`footballKind`/`eventPresentation`/
   `pillVariant`/`pillLabel`/`cardsClean` **computation** at `:422-432`
   stays in `StatusRailCard.tsx` exactly where it is — only the JSX that
   consumes those values, plus `Crest`, moves.
2. **`src/components/NotificationBody.tsx`** — move the JSX fragment at
   `:711-838` verbatim (the `.compact` div including its internal
   `news ? ... : ...` branch, `<Manifest>`, `<TtlBar>`). Named
   `NotificationBody` rather than `CompactBody` because it is NOT just
   the `.compact`-classed div — it's the whole non-live-match content
   fragment (compact + manifest + ttl-bar together), and `.compact` is
   already a load-bearing CSS class name inside it that a component
   named `CompactBody` would collide with in the reader's head. Props:
   every free variable referenced inside — `news: boolean`, `slot:
   Extract<SlotState, { state: "showing" }>` (NOT the bare `SlotState`
   union — the branch reads many `slot.*` fields directly that only
   exist on the "showing" variant: `slot.source`, `slot.priority`,
   `slot.signal`, `slot.eventType`, `slot.title`, `slot.origin`,
   `slot.subtitle`, `slot.queueTotal`, `slot.queueDone`, `slot.body`,
   `slot.id`, `slot.ttlMs`, `slot.remainingMs` — the bare union type
   would fail `tsc` on every one of these since they're absent from the
   "empty" variant; narrowing at the prop boundary avoids an unreadable
   12-field individual-prop list of the kind `Manifest`/`TtlBar` already
   take today (both destructure individual fields, not a whole `slot`
   object — `NotificationBody` needs enough distinct fields that a
   single narrowed `slot` prop is more readable than following that same
   individual-field pattern here), `newsCategory: string |
   null`, `newsAge: string | null`, `bodyContent: ReactNode`,
   `expanded: boolean`, `liveVisibleDetails: Detail[]`, `hovered:
   boolean` (feeds `TtlBar`'s `hoverPaused`). `Detail`
   (`StatusRailCard.tsx:70`, `{ label: string; value: string }`) is
   local and NOT exported today — either `export type Detail = ...` in
   place (it's a two-field structural type; exporting it is a pure
   addition, not a behavior change) or duplicate the same structural
   shape inline in `NotificationBody.tsx`. Prefer exporting it once from
   `StatusRailCard.tsx` and importing it in the new file over
   duplicating the shape — one definition, not two that can drift.

Both new components import their own dependencies directly (`Crest`
needs `convertFileSrc`; `NotificationBody` needs `Manifest`, `TtlBar`,
`renderInlineMarkdown`'s type only if referenced, etc.) — do not
re-export anything through `StatusRailCard.tsx` as a pass-through.
`StatusRailCard.tsx`'s JSX at the two call sites becomes
`<LiveMatchScorecard liveEspn={liveEspn} pillVariant={pillVariant}
pillLabel={pillLabel} eventPresentation={eventPresentation}
cardsClean={cardsClean} body={slot.body} />` and `<NotificationBody
news={news} slot={slot} newsCategory={newsCategory} newsAge={newsAge}
bodyContent={bodyContent} expanded={expanded}
liveVisibleDetails={liveVisibleDetails} hovered={hovered} />`, each
still inside the same `motion.div key={swapKey}` wrapper at `:642-659`
that stays in `StatusRailCard.tsx` (the swap animation itself is not
part of either content branch).

**Verify (per sub-step, move one component, run, then the next — do not
batch both before checking)**: `npx tsc --noEmit` clean; `npx vitest run
src/components/StatusRailCard.test.tsx` green unchanged; `npx vitest run
src/components/StatusRailCard.parity.test.tsx` zero diffs.

### Step 3: Final parity + gates

Re-run the full parity harness once more after both extractions are in
place (not just per-step): `npx vitest run
src/components/StatusRailCard.parity.test.tsx` — zero snapshot diffs
across every fixture and every choreography checkpoint. Then the full
gate set: `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx
vite build` — all clean. Report `wc -l src/components/StatusRailCard.tsx`
(expect roughly 500-600 — down from 861; the exact number depends on how
much of the moved comment-heavy documentation goes with each block, so
don't hard-fail on missing an exact target, but STOP and re-examine if
it's still above ~750, since that would mean the extraction didn't
actually move the bulk of either target).

### Step 4: Delete the parity harness

Delete `src/components/StatusRailCard.parity.test.tsx` and its
generated `src/components/__snapshots__/StatusRailCard.parity.test.tsx.snap`
— it is scaffolding for this plan's own verification, not a permanent
addition to the suite (the existing `StatusRailCard.test.tsx` already
covers behavior at the assertion level; a byte-identical-HTML snapshot
alongside it would be redundant maintenance weight going forward, and
would need hand-updating on every future legitimate visual change).
**Verify**: `npx vitest run` — total test count matches what it was
before Step 0 (no net test-file added or removed); `git status --short`
shows no trace of the parity harness files.

## Test plan

- `StatusRailCard.test.tsx`: **zero changes** — not import paths, not
  assertions. `StatusRailCard`'s export path and public prop signature
  are both unchanged by this plan, so nothing in the test file has any
  reason to move. This is verifiable directly:
  `git diff --stat 2a840c4..HEAD -- src/components/StatusRailCard.test.tsx`
  must print nothing.
- The temporary parity harness (Steps 0/3) is the mechanical
  anti-regression net for the parts `StatusRailCard.test.tsx` doesn't
  already assert on byte-for-byte (e.g. exact DOM nesting/attribute
  order, which `container.innerHTML` catches and individual
  `querySelector` assertions don't).
- No new permanent test file. If, while extracting, the executor judges
  that `useExitChoreography` deserves its own unit-test file (isolating
  it from `StatusRailCard`'s render, the way `useDelayedSwap.test.ts`
  isolates that hook) — this is EXPLICITLY AUTHORIZED as an addition,
  not required; note the choice made and why in the final report either
  way.

## Done criteria

- [ ] `src/components/StatusRailCard.test.tsx` byte-unchanged
      (`git diff` against `2a840c4` is empty for this file)
- [ ] `src/useExitChoreography.ts` exists, exports the hook with the
      return shape in Step 1, and `StatusRailCard.tsx` calls it once
      near the top of the render function
- [ ] `src/components/LiveMatchScorecard.tsx` and
      `src/components/NotificationBody.tsx` exist; `StatusRailCard.tsx`
      renders each via a single call site with the exact props listed
      in Step 2
- [ ] `src/overlay-card.css` has zero diff against `2a840c4`
- [ ] The temporary parity harness (test + snapshot) is deleted before
      finishing — `git status --short` confirms
- [ ] `wc -l src/components/StatusRailCard.tsx` reported in the final
      report, meaningfully below 861
- [ ] All gates green: `npx vitest run`, `npx tsc --noEmit`,
      `npx biome ci .`, `npx vite build`
- [ ] `docs/TESTING_STRATEGY.md` §0 needs NO edit (no test file's case
      count changed) — confirm this explicitly in the report rather than
      silently skipping the doc

## STOP conditions

- The drift check shows `src/animationTiming.ts` or `src/overlay-card.css`
  changed relative to `2a840c4` — a concurrent animation-timing session
  is active; do not merge an extraction on top of a moving target (see
  the header's concurrent-session hazard).
- Any assertion in `StatusRailCard.test.tsx` needs to change to stay
  green — this plan is pure code motion; a required assertion change
  means behavior drifted, not just structure.
- The parity harness (Step 0/3) shows ANY snapshot diff, at any point —
  including a diff that "looks harmless" (e.g. attribute reordering).
  `container.innerHTML` diffing exists precisely to catch the kind of
  drift a human reviewer would rationalize away; treat every diff as
  real until proven to be a pre-existing snapshot-tooling artifact
  (e.g. React's own dev-mode comment nodes), and say explicitly which it
  was if so.
- `useExitChoreography`'s extraction requires changing the RELATIVE
  ORDER in which any two of its internal values are computed (e.g. if
  `bare` needs to read something `railRevealed` produces, or vice
  versa, in a way that didn't hold before) — report the specific
  ordering conflict rather than reordering silently; this is exactly
  the "desynced clocks" bug class the repo's animation memory warns
  about.
- `wc -l src/components/StatusRailCard.tsx` after Step 3 is still above
  ~750 lines — the extraction didn't move enough; re-examine whether a
  comment block or helper got left behind by mistake before reporting
  done.

## Maintenance notes

- Not pursued by this plan, left as explicitly-out-of-scope in Current
  State/Scope above, and worth a future look if this file's size still
  bothers a reviewer post-120: `isWxMarker`/`visibleDetails`/
  `weatherArtFromDetails` (`:58-83`) are pure functions with no
  choreography or root-level-handler coupling — they could move to
  `src/lib/weatherArt.ts` (which `weatherArtFromDetails` already calls
  into) in a follow-up, independent of this plan.
- Also spotted but explicitly not fixed here (would be a behavior-risk
  beyond this plan's pure-motion contract): `footballEventKindFor` is
  called twice on the same signal/body pair — once in the
  `liveCelebration` effect (`:170`) and once in the render-time
  `footballKind` computation (`:423`) that this plan moves alongside
  `LiveMatchScorecard`'s props. They're not obviously mergeable without
  either threading the effect's result down as a value (changing when
  it's computed relative to render) or duplicating the call
  deliberately (today's shape) — flag for a future plan, not this one.
- After this lands, `StatusRailCard.tsx` should read as: hook call →
  signal/pulse state machine → a handful of content `const`s → JSX
  composition of already-known components. Future event-type additions
  (a third content branch, e.g.) should extend the two-branch pattern
  Step 2 establishes rather than growing either existing branch or the
  parent file directly.
