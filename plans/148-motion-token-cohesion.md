# 148 — Motion token cohesion: crossfade ease, disclosure spring, idle-face durations

- **Status**: DONE (2026-07-27)
- **Commit**: 0c5ae11
- **Severity**: HIGH (App crossfade) / MEDIUM (rest)
- **Category**: Easing & duration, Cohesion & tokens, Interruptibility
- **Estimated scope**: 5 files, ~60 lines

## Problem

1. The outermost transition in the product — the board↔card crossfade —
   bypasses both the house ease and the token file:

```tsx
/* src/App.tsx:164 and :185 — current (both sites) */
transition={{ duration: 0.18 }}
```

No `ease`, so motion's default tween easing applies; every other motion
call in the repo passes `ease: NOTCHTAP_EASE`. `0.18` is a hand literal
absent from `src/animationTiming.ts`.

2. One spring config is hand-copied into four call sites across two
   files, with a separate fixed opacity tween that desyncs on
   interrupted hover flips (box still collapsing while already
   transparent, or arrived at height 0 still partly opaque):

```tsx
/* src/components/IdleHoverPeek.tsx:424 — current; byte-identical copies
   at src/components/AgentBoard.tsx:216-220, :346-350, :404-408 */
transition={{ type: "spring", stiffness: 480, damping: 37, opacity: { duration: 0.15 } }}
```

3. `IdleFace` hand-types two durations that sit between existing tokens
   with no comment (near-misses, not deliberate distinctions):

```tsx
/* src/components/IdleFace.tsx:163 — current */
transition={{ duration: 0.24, ease: NOTCHTAP_EASE }}
/* src/components/IdleFace.tsx:193 — current */
transition: `transform 200ms cubic-bezier(${NOTCHTAP_EASE.join(", ")})`,
```

## Target

In `src/animationTiming.ts` (each with a doc comment in the file's
existing voice — every constant there explains WHY its value differs
from its neighbours):

```ts
export const SURFACE_SWAP_MS = 180;
export const DISCLOSURE_SPRING = { type: "spring", stiffness: 480, damping: 37 } as const;
export const IDLE_REVEAL_MS = 240;
export const IDLE_GLANCE_MS = 200;
```

- `SURFACE_SWAP_MS`: the top-level board↔card crossfade (matches the
  previous 0.18s feel; tokenized + house-eased).
- `DISCLOSURE_SPRING`: the shared hover-disclosure spring (ζ ≈ 0.84 —
  slight overshoot is deliberate: these disclosures follow a hover
  gesture). It drives ALL properties including opacity — the old
  `opacity: { duration: 0.15 }` override is REMOVED so an interrupted
  flip retargets height and opacity on the same spring clock (Apple's
  interruptibility rule: one animation, one clock; browsers clamp
  opacity >1 so the slight overshoot is harmless). Doc-comment this
  decision on the const.
- `IDLE_REVEAL_MS` / `IDLE_GLANCE_MS`: name the idle face's two
  durations (values unchanged — this is tokenization, not retuning).

Applied:

```tsx
/* src/App.tsx both sites — target */
transition={{ duration: SURFACE_SWAP_MS / 1000, ease: NOTCHTAP_EASE }}

/* src/components/IdleHoverPeek.tsx:424 — target */
transition={DISCLOSURE_SPRING}

/* src/components/IdleFace.tsx:163 — target */
transition={{ duration: IDLE_REVEAL_MS / 1000, ease: NOTCHTAP_EASE }}
/* src/components/IdleFace.tsx:193 — target */
transition: `transform ${IDLE_GLANCE_MS}ms cubic-bezier(${NOTCHTAP_EASE.join(", ")})`,
```

Do NOT touch AgentBoard.tsx's three copies — plan 149 owns that file
and imports `DISCLOSURE_SPRING` from here.

## Repo conventions to follow

- All timing constants live in `src/animationTiming.ts` with prose doc
  comments (see `EXPAND_MS`'s comment for the house style: it exists
  precisely because a near-miss duration with no documented reason is a
  bug class here).
- `src/animationTiming.test.ts` pins constant values — extend it.
- `src/applyAnimationTiming.ts` maps SOME constants to CSS vars — the
  new constants are motion-side only; do NOT add CSS vars for them.

## Steps

1. Add the four exports + doc comments to `src/animationTiming.ts`.
2. Update `src/App.tsx` (both transitions), `src/components/IdleHoverPeek.tsx:424`,
   `src/components/IdleFace.tsx:163` and `:193` as shown.
3. Extend `src/animationTiming.test.ts`: pin the four new values and,
   for `DISCLOSURE_SPRING`, assert it has NO `opacity` key (the desync
   regression guard).

## Boundaries

- Do NOT touch `src/components/AgentBoard.tsx` (plan 149's file).
- Do NOT retune any value — 0.18→0.180 via token, 0.24/200 unchanged.
- Do NOT add reduced-motion variants (permanent repo non-goal).
- If the code at a cited line differs from the excerpt, STOP and report.

## Verification

- **Mechanical**: `npx vitest run src/animationTiming.test.ts src/App.test.tsx` green; `npx tsc --noEmit` clean; `npx biome ci .` clean.
- **Feel check** (reviewer, live overlay): trigger an agent test event so the board↔card swap fires — the crossfade should now decelerate on the house curve instead of the flat default; hover the idle peek and flick the cursor away mid-open — height and opacity must move together (no ghost box, no early-transparent collapse).
- **Done when**: gates green + no `duration: 0.18` or `stiffness: 480` literal remains in App.tsx/IdleHoverPeek.tsx/IdleFace.tsx.
