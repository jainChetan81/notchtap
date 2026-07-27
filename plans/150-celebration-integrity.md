# 150 — Celebration integrity: rings that finish, repeats that replay

- **Status**: DONE (2026-07-27)
- **Commit**: 0c5ae11
- **Severity**: HIGH (both are broken-promise bugs, not tuning)
- **Category**: Interruptibility, Cohesion
- **Estimated scope**: 4 files (`src/components/StatusRailCard.tsx`, `src/overlay/choreography.css`, `src/overlay/idle-peek.css`, `src/components/StatusRailCard.test.tsx`), ~80 lines

## Problem

1. **The goal ripple is torn out mid-flight.** The `.cele-ripple` layer
   mounts only while `pulse === "pulse-goal"`
   (`src/components/StatusRailCard.tsx:1000-1006`), and `pulse` is
   cleared when `goal-overshoot` (1240ms) ends
   (`StatusRailCard.tsx:268-275`). But the rings run longer:

```css
/* src/overlay/choreography.css:234 — current */
animation: ripple-out 1440ms ease-out forwards;
/* :238 ring 2 delay 280ms → ends at 1720ms */
/* :242 ring 3 delay 560ms → ends at 2000ms */
```

Ring 3 is killed at 62% of its life, mid-expansion. The cut is
currently PINNED AS CORRECT by `StatusRailCard.test.tsx:282`
("unmounts the ripple when the goal pulse's animation ends") — the
test locks the bug in.

2. **A second goal during a celebration plays nothing.** The pulse
   effect (`StatusRailCard.tsx:240-248`) re-runs on a new item id, but
   `setPulse("pulse-goal")` while `pulse` is already `"pulse-goal"` is
   a React `Object.is` state bailout — the className string never
   changes, the DOM attribute is never rewritten, and the CSS animation
   neither restarts nor replays. The comment at `:236-238` ("a new item
   with the same signal must replay the pulse") documents an intent the
   code cannot deliver. Same failure for back-to-back `pulse-red` and
   for `cele-ring` re-triggers.

3. **Cohesion stragglers in the same files** (fix while here):
   `ripple-out` is the one celebration still on bare `ease-out`
   (`choreography.css:234`) — its siblings all migrated to
   `var(--ease-notchtap)` in plan 127; and `cele-ring` runs at 1800ms
   at `src/overlay/idle-peek.css:496` but 1400ms at `:533` — the same
   keyframe at two speeds with no comment.

## Target

1. The pulse gate lives until the LAST ripple ring finishes, not the
   first shell keyframe. In `handleAnimationEnd`
   (`StatusRailCard.tsx:268-275`): clear `pulse-goal` on
   `animationName === "ripple-out"` arriving from the LAST ring
   (count three `ripple-out` ends, or match the third span) instead of
   on `goal-overshoot`. Verify the shell's `goal-overshoot`/`goal-burst`
   /`goal-ring` all tolerate the class persisting ~760ms after they end
   (they are one-shot `forwards`/finite animations — confirm, and note
   it in a comment). `pulse-red` keeps its existing clear (no ripple in
   that path — confirm before assuming).
2. Same-signal replay: when the effect fires for a NEW `currentId` and
   the computed pulse class is unchanged, restart via
   clear-then-reapply on the next frame:

```tsx
/* StatusRailCard.tsx — target shape inside the pulse effect */
setPulse(null);
requestAnimationFrame(() => setPulse(nextPulse)); // remounts the class next frame → CSS animations restart
```

Wrap in the file's effect idiom (cleanup cancels the rAF). One blank
frame is invisible at 60fps and is the standard restart technique; a
comment must say exactly that and cite the React identity bailout it
defeats.
3. `choreography.css:234`: `ease-out` → `var(--ease-notchtap)`
   (duration stays 1440ms — the ring family deliberately outlives the
   1240ms shell; now the GATE respects that instead of truncating it;
   add that sentence as the comment).
4. `idle-peek.css:533`: `cele-ring` 1400ms → 1800ms so the shared
   keyframe runs one speed — UNLESS a comment at either site justifies
   the split (there is none today; if you find one, STOP and report).
5. `StatusRailCard.test.tsx:282`: rewrite the pinned test to the new
   contract — ripple unmounts after the THIRD `ripple-out` end, and a
   second same-signal goal (new id) restarts the pulse (assert the
   class disappears for a frame then returns, via rAF flushing per the
   test file's existing timer idioms).

## Repo conventions to follow

- `choreography.css`'s plan-127 comments (:125-131) are the exemplar
  for documenting curve migrations.
- Celebration tests already simulate `animationend` via
  `fireEvent.animationEnd(el, { animationName: … })` — extend, don't
  invent a new harness.

## Boundaries

- Only the four listed files.
- Do NOT retune ring scale/opacity/delays — lifetime gate + ease only.
- Do NOT touch LiveMatchScorecard (plan 151 owns it).
- No reduced-motion variants (permanent non-goal).
- If cited code has drifted, STOP and report.

## Verification

- **Mechanical**: `npx vitest run src/components/StatusRailCard.test.tsx` green; `npx tsc --noEmit`; `npx biome ci .`.
- **Feel check**: `just push` two goal-signal notifications ~1s apart (`./notchtap --title g1 --body b --signal goal --priority high` twice) — BOTH must visibly pulse; watch a single goal in slow motion (screen-record, step frames): all three rings now expand to completion, ring 3 included.
- **Done when**: gates green; no `ease-out` in choreography.css; one `cele-ring` duration; both feel checks pass.
