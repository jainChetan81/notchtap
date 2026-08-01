# 157 — Replace StatusRailCard's y/scale shorthand with full transform strings

- **Status**: DONE (2026-07-31) — `contentExitVariants`'s interrupt branch and the entrance `initial`/`animate` props now use `transform` strings; `StatusRailCard.test.tsx`'s interrupt-exit test updated to match. `npx vitest run` 573/573, `npx tsc --noEmit` clean.
- **Commit**: ef91a0f, then 9855bfe (follow-up /review-animations fixes)
- **Severity**: HIGH
- **Category**: Performance (AUDIT.md §5)
- **Estimated scope**: 1 file, ~4 call sites

## Problem

`src/components/StatusRailCard.tsx` drives its content-swap animations (the
app's single busiest animation site — it plays on every notification
promotion, rotation, and interrupt) using Motion's `x`/`y`/`scale` shorthand
props instead of a full `transform` string. Per AUDIT.md §5: "Framer Motion
`x`/`y`/`scale` shorthands are not hardware-accelerated — they run on the
main thread and drop frames under load. Target: the full transform string,
`animate={{ transform: "translateX(100px)" }}`."

Current code, `src/components/StatusRailCard.tsx:149-163` (the exit variant
function):

```tsx
export const contentExitVariants = {
  exit: (custom: { isRotation: boolean; isInterrupt: boolean }) => {
    if (custom.isInterrupt) {
      return {
        opacity: 0,
        y: 8,
        scale: 0.96,
        transition: { duration: INTERRUPT_EXIT_MS / 1000, ease: INTERRUPT_EASE },
      };
    }
    return custom.isRotation
      ? { opacity: 0, transition: { duration: ROTATION_EXIT_MS / 1000, ease: NOTCHTAP_EASE } }
      : { opacity: 0, transition: { duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE } };
  },
};
```

Current code, `src/components/StatusRailCard.tsx:1007-1008` (the entrance,
inside the `motion.div` JSX):

```tsx
initial={enterAsPromotion ? { opacity: 0, y: -4 } : { opacity: 0 }}
animate={enterAsPromotion ? { opacity: 1, y: 0 } : { opacity: 1 }}
```

## Target

Every `y`/`scale` value becomes a `transform` string. No numeric or timing
value changes — this is a pure property-shape rewrite, the animation must
look and time identically, only run on the compositor instead of the main
thread.

`contentExitVariants` (interrupt branch only carries a transform; the two
non-interrupt branches are unchanged, they were already pure-opacity):

```tsx
export const contentExitVariants = {
  exit: (custom: { isRotation: boolean; isInterrupt: boolean }) => {
    if (custom.isInterrupt) {
      return {
        opacity: 0,
        transform: "translateY(8px) scale(0.96)",
        transition: { duration: INTERRUPT_EXIT_MS / 1000, ease: INTERRUPT_EASE },
      };
    }
    return custom.isRotation
      ? { opacity: 0, transition: { duration: ROTATION_EXIT_MS / 1000, ease: NOTCHTAP_EASE } }
      : { opacity: 0, transition: { duration: CONTENT_EXIT_MS / 1000, ease: NOTCHTAP_EASE } };
  },
};
```

Entrance (JSX props):

```tsx
initial={enterAsPromotion ? { opacity: 0, transform: "translateY(-4px)" } : { opacity: 0 }}
animate={enterAsPromotion ? { opacity: 1, transform: "translateY(0px)" } : { opacity: 1 }}
```

## Repo conventions to follow

- This exact swap (shorthand → full `transform` string) is already the
  pattern used elsewhere in the same file for CSS-side transforms — e.g.
  `src/overlay/card-chrome.css`'s hover breathe uses a raw `transform:
  scale(1.02)` CSS property, not a JS shorthand. This plan brings the
  `motion`-driven JS side in line with the same "always a real transform
  string/property" discipline.
- Keep every existing comment in the file attached to the code it
  describes — do not delete the surrounding doc comments (e.g. the
  `INTERRUPT_EASE`/`plan 146b` explanation above `contentExitVariants`);
  only change the object shape of the animated values themselves.

## Steps

1. In `src/components/StatusRailCard.tsx`, locate `contentExitVariants`
   (currently lines 149-163). In the `custom.isInterrupt` branch, replace
   the separate `y: 8, scale: 0.96` fields with a single `transform:
   "translateY(8px) scale(0.96)"` field, keeping `opacity: 0` and the
   `transition` object exactly as they are.
2. Locate the `motion.div` JSX block containing the `initial`/`animate`
   props (currently around lines 1007-1008). Replace `y: -4` with
   `transform: "translateY(-4px)"` in the `initial` branch's
   `enterAsPromotion` case, and `y: 0` with `transform: "translateY(0px)"`
   in the `animate` branch's `enterAsPromotion` case. The non-promotion
   branches (`{ opacity: 0 }` / `{ opacity: 1 }`) are already pure-opacity
   and stay unchanged.
3. Search the same file for any other `motion.div`/`AnimatePresence` usage
   with bare `y:`, `x:`, or `scale:` fields you may have missed in this
   plan's citations (the file is large) — if you find any, apply the same
   transform-string conversion, but do NOT touch fields that are already
   `transform:` strings or pure `opacity:`.

## Boundaries

- Do NOT touch `contentExitVariants`'s `transition` objects, `duration`
  values, or `ease` values — only the animated-value shape changes.
- Do NOT touch other components (`AgentBoard.tsx`, `LiveMatchScorecard.tsx`,
  `IdleHoverPeek.tsx`) — those have their own similar findings tracked
  separately and are out of scope here.
- Do NOT change the `enterAsPromotion` branching logic itself, only the
  values inside each branch.
- If the current code at the cited line numbers doesn't match what's quoted
  above (drift since commit `58cccd9`), STOP and report instead of
  improvising — re-locate by searching for `contentExitVariants` and
  `enterAsPromotion` first.

## Verification

- **Mechanical**: `npx vitest run` (repo root) → `StatusRailCard.test.tsx`
  must stay green, including any test that inspects
  `data-rotation-swap`/`data-interrupt-swap` attributes or the rendered
  transition. `npx tsc --noEmit` → no type errors.
- **Feel check**: run the app, trigger a Priority Preemption interrupt
  (highest-priority notification arriving while a lower-priority one is
  showing) and an ordinary promotion:
  - In DevTools' Animations panel, set playback to 10% and confirm the
    interrupt "yank" (translateY + scale down together) and the promotion
    slide-in look pixel-identical in trajectory/timing to before this
    change — only the underlying property changed, not the motion.
  - Confirm no visual jump/snap was introduced at either the start or end
    of either animation.
- **Done when**: `contentExitVariants` and the entrance `initial`/`animate`
  props use `transform` strings instead of `y`/`scale` shorthand, `npx
  vitest run` and `npx tsc --noEmit` are clean, and the feel-check confirms
  no visible timing/trajectory change.
