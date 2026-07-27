# 149 — Agent Board motion vitals: bounded pulse, accent morph, hero swap, adaptive tick

- **Status**: DONE (2026-07-27)
- **Commit**: 0c5ae11 (depends on plan 148's `DISCLOSURE_SPRING` export)
- **Severity**: HIGH
- **Category**: Performance, Purpose & frequency, Missed opportunities
- **Estimated scope**: 3 files (`src/components/AgentBoard.tsx`, `src/overlay/agent-board.css`, `src/components/AgentBoard.test.tsx`), ~120 lines

## Problem

1. **Unbounded pulse.** `src/overlay/agent-board.css:190-191`:

```css
.card-root .agent-dot.pulse {
  animation: agent-dot-breathe 2.2s ease-in-out infinite;
}
```

`pulse` is bound to working/waiting states, and `waiting_for_input` can
persist for hours — a 60fps opacity loop inside the card's
`filter: drop-shadow` subtree, running while nobody looks. Plan 105
already removed exactly this class of always-on pulse from the status
dots ("read as distracting in peripheral vision", `src/overlay/status-dots.css:73-76`)
— the information value of a pulse is "this just changed", not "this is
still true 40 minutes later".

2. **Accent snaps.** `.agent-dot` gets its colour from
`var(--agent-accent)` rebound by a state-class flip — working→waiting
(blue→amber) changes in a single frame, the exact moment the operator
most needs to notice.

3. **Hero teleports.** `src/components/AgentBoard.tsx:307-322` — the
resting hero renders `primary`'s five lines with no key and no
AnimatePresence; when a different session becomes primary, everything
swaps in one frame with nothing to distinguish "new session" from
"same session, new state".

4. **Per-second full-board re-render, all day.** `src/components/AgentBoard.tsx:31`
`NOW_TICK_MS = 1000` + `useNowTick` re-renders the whole board every
second, but `elapsedLabel` (`src/lib/presentation.ts:376-388`) only has
second-granularity below 60s — above a minute, 60 consecutive renders
produce byte-identical output.

5. **Spring copies.** Three byte-identical
`{ type: "spring", stiffness: 480, damping: 37, opacity: { duration: 0.15 } }`
literals at `src/components/AgentBoard.tsx:216-220`, `:346-350`,
`:404-408` — plan 148 exports `DISCLOSURE_SPRING` from
`src/animationTiming.ts` for exactly these.

## Target

1. Bounded breathe + settle: the dot breathes for ~4 cycles after each
   STATE CHANGE, then rests steady. Key the dot span on the session
   state so re-entering/`changing` state restarts the bounded run:

```css
/* agent-board.css — target */
.card-root .agent-dot.pulse {
  /* bounded: the pulse means "this just changed", not "still true an
     hour later" — plan-105 precedent (status-dots.css:73-76). 4 cycles
     ≈ 8.8s of breathing per state change, then steady. */
  animation: agent-dot-breathe 2.2s ease-in-out 4;
}
```

```tsx
/* AgentBoard.tsx — every .agent-dot span (AgentRow, ExpandedAgentRow,
   hero), target shape: */
<span key={session.state} className={`agent-dot ...`} aria-hidden="true" />
```

2. Accent morph + state tick on the same dots:

```css
/* agent-board.css — target additions */
.card-root .agent-dot {
  transition: background-color var(--hover-ms, 160ms) var(--ease-notchtap);
}
/* one-shot "state changed" tick — runs on the state-keyed remount,
   stacked before the bounded breathe */
.card-root .agent-dot.pulse {
  animation:
    agent-dot-state-tick 240ms var(--ease-notchtap),
    agent-dot-breathe 2.2s ease-in-out 4;
}
@keyframes agent-dot-state-tick {
  from { transform: scale(1.35); }
  to { transform: scale(1); }
}
```

(Non-pulse states get only the background-color morph — no tick
keyframe outside `.pulse`, to keep completed/stale transitions quiet.)

3. Hero swap keyed on identity:

```tsx
/* AgentBoard.tsx hero — target: wrap the hero's inner content */
<AnimatePresence initial={false} mode="wait">
  <motion.div
    key={primary.id}
    initial={{ opacity: 0, y: 6 }}
    animate={{ opacity: 1, y: 0 }}
    exit={{ opacity: 0, y: -6 }}
    transition={{ duration: 0.16, ease: NOTCHTAP_EASE }}
  >
    …existing hero content…
  </motion.div>
</AnimatePresence>
```

Keyed by `primary.id` ONLY — a state change within the same session
must NOT animate (it morphs in place via item 2); only an identity
change swaps. `mode="wait"` is correct here (the hero is a single
block; a 160ms out/in reads as a swap, and overlap would double the
block's height mid-flight).

4. Adaptive tick: `useNowTick` runs at 1000ms while ANY session's live
   elapsed is under 60s, else 15000ms. Derive the interval from the
   sessions prop + capturedAtMs; re-evaluate when sessions change.
   Keep the hook's existing doc-comment style and extend it with why
   (elapsedLabel is minute-granular past 60s; 3,600 identical
   re-renders/hour otherwise).

5. Replace the three spring literals with
   `import { DISCLOSURE_SPRING } from "../animationTiming"` — including
   REMOVING the `opacity: { duration: 0.15 }` overrides (plan 148
   documents why: interrupted flips must keep height and opacity on one
   spring clock).

## Repo conventions to follow

- `ROW_TRANSITION` at the top of AgentBoard.tsx is the exemplar for a
  shared, doc-commented transition const.
- The dedup/desynced-clocks comment style: state the constraint, cite
  the precedent (plan 105, apple-design derivation).
- Tests pin structure and const values, not mid-flight styles (see the
  existing "row removal/insertion/reorder fluidity" describe block).

## Steps

1. `agent-board.css`: bounded breathe (iteration count 4), the
   `agent-dot-state-tick` keyframes, the base `transition` on
   `.agent-dot`.
2. `AgentBoard.tsx`: state-key every `.agent-dot` span; hero
   AnimatePresence wrapper; adaptive `useNowTick`; `DISCLOSURE_SPRING`
   imports (3 sites).
3. `AgentBoard.test.tsx`: hero remounts on primary-id change and does
   NOT remount on same-id state change; dot span key flips with state;
   adaptive tick — with all sessions past 60s elapsed the board does
   not re-render between two 1s fake-timer advances (or pin the
   interval-selection helper directly if extracted); spring literals
   gone (assert `DISCLOSURE_SPRING` identity via import equality if the
   file's patterns allow).

## Boundaries

- Do NOT touch `ROW_TRANSITION` or the row enter/exit/layout work.
- Do NOT touch `src/animationTiming.ts` (plan 148 owns it; it must land
  first — if `DISCLOSURE_SPRING` doesn't exist yet, STOP and report).
- No reduced-motion variants (permanent repo non-goal).
- If cited code has drifted, STOP and report.

## Verification

- **Mechanical**: `npx vitest run src/components/AgentBoard.test.tsx` green; `npx tsc --noEmit`; `npx biome ci .` clean.
- **Feel check** (reviewer, live overlay): start an agent session — the dot breathes ~9s then holds steady; answer/change state — colour morphs over ~160ms with a small tick, no snap; with two sessions, finish the primary — the hero swaps with a 160ms fade+6px slide; leave the board idle >1 min — Activity Monitor's WindowServer/notchtap CPU should drop vs before.
- **Done when**: gates green + no `stiffness: 480` literal and no `infinite` on `agent-dot-breathe` remains.
