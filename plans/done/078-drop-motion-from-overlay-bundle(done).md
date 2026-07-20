# Plan 078: Replace `motion` with CSS transitions in the overlay path, and memoize inline markdown

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. This plan changes visible animation behavior — Step 8's manual
> visual check is **operator-owed**, matching this repo's own established
> practice for animation/visual changes (see plans 018, 023, 032). Do not
> mark this plan DONE without either running that check yourself or
> explicitly flagging it as owed. If anything in the "STOP conditions"
> section occurs, stop and report — do not improvise. When done, update the
> status row for this plan in `plans/README.md` — unless a reviewer
> dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat f6c2f46..HEAD -- src/App.tsx src/components/StatusRailCard.tsx src/components/Manifest.tsx src/styles.css`
> If any of these changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED — behavior-preserving in intent, but it's a hand-rolled
  replacement for a battle-tested animation library in the one part of
  the app (the always-on overlay) this repo cares most about the visual
  polish of; needs the operator's own eyeball, not just green tests.
- **Depends on**: none. **Supersedes plan 069** (memoize
  `renderInlineMarkdown`) — that fix is folded into this plan's Step 7
  rather than done separately, since both touch the same render paths.
  Mark `plans/069-memoize-inline-markdown.md`'s status as SUPERSEDED by
  this plan when you file it — do not execute 069 separately.
- **Category**: perf / tech-debt
- **Planned at**: commit `f6c2f46`, 2026-07-20
- **Review-plan pass (2026-07-20)**: own read (zero drift on all 4 cited
  frontend files) + a required fresh-context subagent cold-read
  (authored in-session). Found and fixed a real structural contradiction
  in Step 3: the plan claimed `Track`/`Stamp` "keep reading the live
  `slot`" while also being physically nested inside the exact JSX block
  Step 3 converts to read from `renderedSlot` — confirmed by reading the
  live `StatusRailCard.tsx` in full, `<Stamp>` (line 159) and `<Track>`
  (line 165) both sit inside the `<div className="compact">` that's
  inside the swapped `motion.div key={slot.id}`. That claim was simply
  wrong, not just ambiguous. Resolved: **everything** that was inside
  the swapped `motion.div` (the whole `compact` block including
  Stamp/Track/compact-hint, plus `<Manifest>`) now reads from
  `renderedSlot` uniformly — this doesn't regress the expand/collapse
  toggle (⌃⇧N), because `useDelayedSwap`'s own same-key branch already
  passes the live value straight through with no freeze when only
  `slot.expanded` changes (the key stays `slot.id`, unchanged); the
  freeze only ever applies across a genuine key change (a different
  item swapping in). Only `cardClass` (outer `.rail-card` div — priority
  accent, status-rail width, pulse-celebration classes, news-shade) and
  the `pulse` state/`useEffect` stay on live `slot`/`status`, exactly as
  they already do today — both are physically outside the swapped
  `motion.div` in the current code and were never part of
  `AnimatePresence`'s animated content, so this isn't a new behavior,
  just preserving the existing split. Step 3 below is rewritten with the
  full real JSX inlined (the previous draft elided it as "...unchanged
  content..." in two places, which the cold-read flagged as not actually
  reproducible without re-deriving it from scratch).
- **Second review-plan pass (2026-07-20, same day)**: re-verified drift
  first — zero changes on any of the 4 cited frontend files since the
  first pass (no other plan touches this part of the frontend). Re-ran
  `npx vite build` fresh (safe: `dist/` is gitignored, confirmed) and
  got byte-identical output to the plan's cited numbers, down to the
  same content hash (`StatusRailCard-DDBkFDcn.js`, 332.24 kB / 106.37 kB
  gzip) — the plan's central premise is still exactly accurate, not
  stale. Found and fixed two smaller gaps: (1) Step 2 sent the executor
  to check `useSlotState.test.ts`/`useStatusState.test.ts` for an
  existing fake-timer pattern that doesn't actually exist there — a
  repo-wide search (`grep -rln "useFakeTimers\|advanceTimersByTime"
  src/`) found zero usage anywhere in the frontend suite, so this is
  genuinely new territory, not a pattern to locate; rewrote to say so
  directly and specify the vitest API to use. (`renderHook` usage in
  `useSlotState.test.ts` was independently confirmed accurate, at
  `useSlotState.test.ts:1,38`.) (2) The CSS class-specificity hedge in
  Step 3 ("adjust as needed... not necessarily final") was unnecessary
  caution — worked through the specificity by hand and confirmed the
  four selectors already resolve correctly with no ambiguity (the
  `.idle.swap-exit` combined selector is strictly more specific than
  either alone, so the apparent tie between `.idle` and `.swap-exit`
  never actually matters); tightened to a confirmation so the executor
  doesn't go looking for a fix that isn't needed.
- **Third review-plan pass (2026-07-20, same day)**: zero drift again
  (same HEAD as the second pass — nothing else has touched this part of
  the frontend). Extended verification scope beyond the original
  drift-check's 4 files to every child component Step 3's rewritten JSX
  actually calls into — `IdleView`, `Stamp`, `Track`, `Manifest` — none
  of which were previously drift-checked directly. All four confirmed
  zero drift and exact prop-signature matches against what Step 3
  passes: `IdleView({ status }: { status?: StatusState })`,
  `Stamp({ priority, signal, eventType })`,
  `Track({ total, done }: { total: number; done: number })`, and
  `Manifest`'s 9-prop signature (`body`/`eventType`/`expanded`/`source`/
  `category`/`publishedAtMs`/`hasLink`/`subtitle`/`details`) — all
  verified directly against the live files, not assumed. No new issues
  found this pass; the plan is in the same verified state as after the
  second pass.

## Why this matters

A live production build (`npx vite build`) shows the overlay window's
main chunk at 332 KB / 106 KB gzipped:

```
dist/assets/StatusRailCard-DDBkFDcn.js  332.24 kB │ gzip: 106.37 kB
```

Grepping that built chunk for `motion` returns 22 hits and 0 for
`lucide` — the `lucide-react` icons (used only in `SettingsApp.tsx`)
correctly tree-shake to zero footprint in chunks that don't reference
them, but the `motion` animation library does not, because it's actually
used here. Every `motion`/`AnimatePresence` call site in the overlay
path — `App.tsx`'s `<MotionConfig>` wrapper, `StatusRailCard.tsx`'s
idle/showing swap and two news-pill entrances, `Manifest.tsx`'s
expand/collapse — does nothing more elaborate than an opacity/
translateY fade or a height transition. No drag, no gestures, no layout
animation, no springs are used anywhere in this codebase (confirmed by
reading every `motion.*`/`AnimatePresence` call site).

This repo has twice already made exactly this call for other
dependencies: plan 023 dropped `lottie-react` for hand-written CSS
confetti, and plan 018 replaced a JS-driven `background-position`
animation with a compositor-only CSS `transform`, both explicitly to cut
cost in this same always-on overlay. `motion` is the same category of
dependency those two plans already decided against — it just hadn't been
looked at from a bundle-composition angle until now. `SettingsApp.tsx`
keeps two of its own `motion.div` usages and its own `<MotionConfig>` —
those are out of scope here (see Scope); it's a secondary window opened
rarely, not the idle-cost-sensitive surface those two prior plans and
this one are about.

## Current state

- `package.json` — `"motion": "^12.42.2"` stays a dependency after this
  plan (`SettingsApp.tsx` still needs it); this plan only changes what
  the *overlay's* import graph pulls in, not the package list.

- `src/App.tsx` (full file, 57 lines) — the `<MotionConfig
  reducedMotion="user">` wrapper this plan removes:

  ```tsx
  import { MotionConfig } from "motion/react";
  // ...
  return (
    <MotionConfig reducedMotion="user">
      <StatusRailCard slot={slot} status={status} />
    </MotionConfig>
  );
  ```

  Note `MotionConfig`'s `reducedMotion="user"` only ever covered
  `motion.*` components — the plain-CSS pulse/celebration animations
  already in `styles.css` (goal burst, red-alert) do **not** get this for
  free; they already carry their own `@media (prefers-reduced-motion:
  reduce)` overrides, with a comment explaining exactly why
  (`styles.css:258-264`, right above the `pulse-goal`/`pulse-red`
  override block):

  ```css
  /* the two pulses above are plain CSS, not `motion` components, so
     <MotionConfig reducedMotion="user"> (App.tsx) does not cover them —
     they need this override in their own right. */
  @media (prefers-reduced-motion: reduce) {
    .rail-card.pulse-goal,
    .rail-card.pulse-goal::before,
    .rail-card.pulse-goal::after,
    .rail-card.pulse-red {
      animation: none;
    }
  }
  ```

  This is the exact precedent to follow for every new CSS animation this
  plan adds — each one needs its own `prefers-reduced-motion` override,
  the same way, since there will be no `<MotionConfig>` left to cover
  them implicitly.

- `src/components/StatusRailCard.tsx` — the full component as it
  currently stands (168 lines; every field referenced below — `slot`,
  `status`, `expanded`, `news`, `newsCategory`, `newsAge`, `pulse`,
  `cardClass` — is computed earlier in the same function, unchanged by
  this plan except where Step 3 says otherwise):

  ```tsx
  export function StatusRailCard({ slot, status }: { slot: SlotState; status?: StatusState }) {
    // ...showing/currentId/currentSignal/news/newsCategory/newsAge/pulse/
    // cardClass all computed here, unchanged by this plan...
    return (
      <div
        className={cardClass}
        role={showing ? "status" : undefined}
        aria-live={showing ? "polite" : undefined}
        onAnimationEnd={clearPulseWhenItsAnimationEnds}
      >
        <AnimatePresence mode="wait" initial={false}>
          {!showing ? (
            <motion.div
              key="idle"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.22 }}
            >
              <IdleView status={status} />
            </motion.div>
          ) : (
            <motion.div
              key={slot.id}
              initial={{ opacity: 0, y: -4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            >
              <div className="compact">
                <div className="copy">
                  {slot.eventType === "news_item" ? (
                    <>
                      <div className="masthead">
                        <span className="dot" />
                        {slot.source ?? "RSS"}
                      </div>
                      <div className="title headline">{slot.title}</div>
                      {(newsCategory !== null || newsAge !== null) && (
                        <div className="pills">
                          {newsCategory !== null && (
                            <motion.span className="pill category" /* ... */>
                              {newsCategory}
                            </motion.span>
                          )}
                          {newsAge !== null && (
                            <motion.span className="pill age" /* ... */>
                              {newsAge}
                            </motion.span>
                          )}
                        </div>
                      )}
                    </>
                  ) : (
                    <>
                      <div className="title">{slot.title}</div>
                      <div className="body">{renderInlineMarkdown(slot.body)}</div>
                      {!expanded &&
                        slot.details.length > 0 &&
                        slot.details.map((detail) => (
                          <div key={`${detail.label}:${detail.value}`}>
                            <div className="detail-label">{detail.label}</div>
                            <div className="detail-value">{detail.value}</div>
                          </div>
                        ))}
                    </>
                  )}
                </div>
                <Stamp priority={slot.priority} signal={slot.signal} eventType={slot.eventType} />
                {!expanded && (
                  <div className="compact-hint">
                    <kbd>⌃⇧N</kbd> more
                  </div>
                )}
                <Track total={slot.queueTotal} done={slot.queueDone} />
              </div>
              <Manifest
                body={slot.body}
                eventType={slot.eventType}
                expanded={expanded}
                source={slot.source}
                category={slot.category}
                publishedAtMs={slot.publishedAtMs}
                hasLink={slot.link !== null}
                subtitle={slot.subtitle}
                details={slot.details}
              />
            </motion.div>
          )}
        </AnimatePresence>
        {pulse === "pulse-goal" && (
          <div className="cele-ripple" aria-hidden="true">
            <span /><span /><span />
          </div>
        )}
      </div>
    );
  }
  ```

  **Load-bearing structural fact for Step 3**: `<Stamp>`, the
  `compact-hint`, and `<Track>` are all physically *inside* the swapped
  `motion.div key={slot.id}` (nested inside its child `<div
  className="compact">`), not outside it. `cardClass` (on the outer
  `.rail-card` div) and the `pulse`/`cele-ripple` celebration are the
  only things outside the swapped block.

  And the two news pills (same file, inside the "showing, news_item"
  branch), enter-only — neither is wrapped in its own `AnimatePresence`,
  so today they already have **no exit animation** (an unmounting
  `motion.span` outside `AnimatePresence` just vanishes instantly; `exit`
  props are only honored inside an `AnimatePresence`) — only the enter
  needs replicating:

  ```tsx
  <motion.span
    className="pill category"
    initial={{ opacity: 0, y: 3 }}
    animate={{ opacity: 1, y: 0 }}
    transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
  >
    {newsCategory}
  </motion.span>
  {/* ...newsAge pill: identical shape, transition adds `delay: 0.07` */}
  ```

  Existing regression test to preserve exactly (`StatusRailCard.test.tsx:447-461`,
  `"updates the queue slider on a waiting-count change without remounting
  the card"`): re-rendering with the **same** `slot.id` but a changed
  `queueTotal`/`queueDone` must NOT remount the card — asserted via
  `expect(trackAfter).toBe(trackBefore)` (referential DOM node identity).
  Whatever replaces `AnimatePresence`/`motion.div key={slot.id}` must
  preserve this: same key ⇒ same DOM node, in-place update, no
  animation replay.

- `src/components/Manifest.tsx` (full file, 126 lines) — the
  expand/collapse this plan replaces:

  ```tsx
  <AnimatePresence initial={false}>
    {expanded && (
      <motion.div
        className="manifest"
        initial={{ height: 0, opacity: 0 }}
        animate={{ height: "auto", opacity: 1 }}
        exit={{ height: 0, opacity: 0 }}
        transition={{ duration: 0.24, ease: [0.22, 1, 0.36, 1] }}
      >
        {/* ...manifest-inner content, unchanged... */}
      </motion.div>
    )}
  </AnimatePresence>
  ```

  There is **no separate `Manifest.test.tsx`** — its behavior is
  exercised through `StatusRailCard.test.tsx` (confirmed via
  `find src -iname "Manifest*"`, which returns only the component file
  itself). Any new test coverage for the expand/collapse behavior belongs
  in `StatusRailCard.test.tsx`, following its existing pattern for
  asserting on `expanded`-driven content.

- Repo convention for cubic-bezier easing: every `motion` call site above
  uses `ease: [0.22, 1, 0.36, 1]` — this translates directly to the CSS
  `cubic-bezier(0.22, 1, 0.36, 1)` function; use it verbatim, don't
  approximate with a named easing keyword.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Lint/format | `npx biome ci .` | exit 0 (or matches pre-existing failure baseline exactly) |
| Build + bundle check | `npx vite build` | exit 0; inspect the overlay's main JS chunk size/name in the output |
| Bundle composition check | `grep -c "motion" dist/assets/<overlay-chunk>.js` | 0 (confirms `motion` no longer reaches the overlay's chunk) |

## Scope

**In scope**:
- `src/App.tsx` (remove `<MotionConfig>`/`motion` import)
- `src/components/StatusRailCard.tsx` (swap + pills + memoized markdown)
- `src/components/Manifest.tsx` (expand/collapse + memoized markdown)
- `src/styles.css` (new keyframes/transitions + `prefers-reduced-motion` overrides)
- A new hook: `src/useDelayedSwap.ts` + `src/useDelayedSwap.test.ts`
- `src/components/StatusRailCard.test.tsx` (extend, don't rewrite, the existing swap/expand assertions if needed to cover the new mechanism — see Step 9)

**Out of scope — do not touch**:
- `src/settings/SettingsApp.tsx` and its two `motion.div` usages (~line
  525-539, ~1480-1531), and its own separate `<MotionConfig>` — this
  plan is about the overlay's bundle weight specifically; the Settings
  window is a secondary, rarely-opened window where this tradeoff
  doesn't apply the same way. Removing `motion` from Settings too is a
  plausible future follow-up, not this plan's job.
- `package.json` — `motion` stays a dependency (Settings still needs
  it); do not remove the package.
- `src/components/IdleView.tsx`, `src/components/Stamp.tsx`,
  `src/components/Track.tsx` — none of these import `motion` themselves;
  no change needed.
- The pure-CSS pulse/celebration animations already in `styles.css`
  (`pulse-goal`, `pulse-red`, `cele-ripple`) — untouched, already
  correct and already carry their own reduced-motion overrides.

## Steps

### Step 1: Add the `useDelayedSwap` hook

Create `src/useDelayedSwap.ts`, matching this repo's existing top-level
hook file convention (`useClock.ts`, `useSlotState.ts`). This hook
replaces `AnimatePresence mode="wait"`'s "freeze old content, wait for
exit, then swap to new content" behavior for a keyed value:

```ts
import { useLayoutEffect, useState } from "react";

// Small stand-in for `AnimatePresence mode="wait"` (plan 078 dropped
// `motion` from the overlay bundle — see styles.css for the CSS half of
// this). Freezes `value` at its last snapshot while `key` has changed
// but the exit animation hasn't finished, then swaps to the new
// value/key together once `exitDurationMs` elapses. A same-key update
// (the content changed but the key didn't — e.g. a queue-counter tick
// on the still-visible item) is synced immediately, in place, with no
// timer and no animation replay.
export function useDelayedSwap<T>(
  value: T,
  key: unknown,
  exitDurationMs: number,
): { value: T; exiting: boolean } {
  const [shown, setShown] = useState<{ key: unknown; value: T }>({ key, value });
  const [exiting, setExiting] = useState(false);

  useLayoutEffect(() => {
    if (key === shown.key) {
      return;
    }
    setExiting(true);
    const id = window.setTimeout(() => {
      setShown({ key, value });
      setExiting(false);
    }, exitDurationMs);
    return () => window.clearTimeout(id);
    // biome-ignore lint/correctness/useExhaustiveDependencies: `value` is
    // deliberately excluded — only a `key` change should (re)start the
    // exit timer. A same-key value update is synced below, at render
    // time, not through this effect.
  }, [key, shown.key, exitDurationMs]);

  // same key: pass the live value straight through (no state update, no
  // re-render caused by this hook) — this is what makes the existing
  // "no remount on same-key content update" test hold.
  const liveValue = key === shown.key ? value : shown.value;
  return { value: liveValue, exiting };
}
```

`useLayoutEffect` (not `useEffect`) matters here: it sets `exiting =
true` synchronously before the browser paints the key-changed render, so
the "exiting" CSS class is present from the very first painted frame
instead of one frame late.

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 2: Test the hook directly

Add `src/useDelayedSwap.test.ts`, modeled on `useSlotState.test.ts`'s use
of `@testing-library/react`'s `renderHook` (confirmed:
`useSlotState.test.ts:1,38` imports and uses it exactly this way). For
the timer-dependent assertions, this is genuinely new territory for this
test suite — confirmed via a repo-wide search
(`grep -rln "useFakeTimers\|advanceTimersByTime" src/`), zero existing
usage anywhere, so there's no in-repo pattern to copy for that part.
Use vitest's standard API directly: `vi.useFakeTimers()` in a
`beforeEach`, `vi.advanceTimersByTime(ms)` to move past
`exitDurationMs`, `vi.useRealTimers()` in an `afterEach`. Cover:
- Same key, changed value → `value` updates immediately, `exiting` stays `false`, no timer involved.
- Different key → `value` stays at the OLD snapshot and `exiting` becomes `true` immediately (synchronously, per the `useLayoutEffect`); after `exitDurationMs` elapses, `value` updates to the new value and `exiting` returns to `false`.
- A second key change before the first timer fires → the first timer is cleaned up (cleared), only the latest key's timer fires (this is what the effect's cleanup function guarantees — write a test that changes key twice in quick succession and asserts only one swap happens, landing on the final key's value).

**Verify**: `npx vitest run src/useDelayedSwap.test.ts` → all new tests pass.

### Step 3: Rewire `StatusRailCard.tsx`'s idle/showing swap

Replace the `AnimatePresence`/`motion.div` swap with the hook:

```tsx
import { useDelayedSwap } from "../useDelayedSwap";
// remove: import { AnimatePresence, motion } from "motion/react";

// ...inside the component, in place of the current AnimatePresence block:
const swapKey = showing ? slot.id : "idle";
const { value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey, 220);
const renderedShowing = renderedSlot.state === "showing";
```

Render based on `renderedSlot`/`renderedShowing` for **everything that
was inside the swapped `motion.div`** — that means the whole `compact`
block (including `Stamp`, the compact-hint, and `Track` — all three were
already nested inside it, per the "Load-bearing structural fact" note in
"Current state" above) and `<Manifest>`, all reading `renderedSlot`'s
fields instead of `slot`'s. This is safe for the expand/collapse hotkey
(⌃⇧N): toggling `expanded` never changes `slot.id`, so `useDelayedSwap`'s
same-key branch passes the live value straight through with zero delay —
nothing about that interaction changes. Only `cardClass` (outer div,
already outside the swapped block) and the `pulse`/`cele-ripple`
celebration stay on live `slot`/`status`, exactly as they already do
today — this isn't a new split, it's the one that already exists between
what's inside vs. outside `motion.div` in the current code.

Compute a `renderedSlot`-scoped set of the same derived values the
component already computes from live `slot` (`news`, `newsCategory`,
`newsAge`, `expanded`), then swap every reference inside the
`compact`/`Manifest` block from `slot.*` to `renderedSlot.*`:

```tsx
const swapKey = showing ? slot.id : "idle";
const { value: renderedSlot, exiting } = useDelayedSwap(slot, swapKey, 220);
const renderedShowing = renderedSlot.state === "showing";
const renderedExpanded = renderedShowing && renderedSlot.expanded;
const renderedNews = renderedShowing && renderedSlot.eventType === "news_item";
const renderedNewsCategory = renderedNews ? categoryLabel(renderedSlot.category) : null;
const renderedNewsAge = renderedNews ? ageLabel(renderedSlot.publishedAtMs, Date.now()) : null;
```

```tsx
<div
  key={swapKey}
  className={`card-content${!renderedShowing ? " idle" : ""}${exiting ? " swap-exit" : ""}`}
>
  {!renderedShowing ? (
    <IdleView status={status} />
  ) : (
    <>
      <div className="compact">
        <div className="copy">
          {renderedSlot.eventType === "news_item" ? (
            <>
              <div className="masthead">
                <span className="dot" />
                {renderedSlot.source ?? "RSS"}
              </div>
              <div className="title headline">{renderedSlot.title}</div>
              {(renderedNewsCategory !== null || renderedNewsAge !== null) && (
                <div className="pills">
                  {renderedNewsCategory !== null && (
                    <span className="pill category">{renderedNewsCategory}</span>
                  )}
                  {renderedNewsAge !== null && (
                    <span className="pill age">{renderedNewsAge}</span>
                  )}
                </div>
              )}
            </>
          ) : (
            <>
              <div className="title">{renderedSlot.title}</div>
              <div className="body">{bodyContent /* Step 7 memoizes this */}</div>
              {!renderedExpanded &&
                renderedSlot.details.length > 0 &&
                renderedSlot.details.map((detail) => (
                  <div key={`${detail.label}:${detail.value}`}>
                    <div className="detail-label">{detail.label}</div>
                    <div className="detail-value">{detail.value}</div>
                  </div>
                ))}
            </>
          )}
        </div>
        <Stamp priority={renderedSlot.priority} signal={renderedSlot.signal} eventType={renderedSlot.eventType} />
        {!renderedExpanded && (
          <div className="compact-hint">
            <kbd>⌃⇧N</kbd> more
          </div>
        )}
        <Track total={renderedSlot.queueTotal} done={renderedSlot.queueDone} />
      </div>
      <Manifest
        body={renderedSlot.body}
        eventType={renderedSlot.eventType}
        expanded={renderedExpanded}
        source={renderedSlot.source}
        category={renderedSlot.category}
        publishedAtMs={renderedSlot.publishedAtMs}
        hasLink={renderedSlot.link !== null}
        subtitle={renderedSlot.subtitle}
        details={renderedSlot.details}
      />
    </>
  )}
</div>
```

(The news pills lose their `motion.span` wrapper here — Step 4 replaces
them with plain `span`s + CSS, shown separately below so this step's
diff stays focused on the swap mechanism.)

The `key={swapKey}` on this wrapper is load-bearing: it's what makes
React actually create a fresh DOM node (and therefore replay the CSS
enter `animation`) exactly when the item changes, while leaving the node
alone (no replay) for a same-key content-only update — mirroring
`motion.div key={slot.id}`'s behavior exactly, and required by the
existing `"updates the queue slider on a waiting-count change without
remounting the card"` test.

Add the matching CSS to `src/styles.css` (near the existing `.compact`/
`.rail-card` rules):

```css
.card-content {
  animation: card-enter-showing 220ms cubic-bezier(0.22, 1, 0.36, 1);
}
.card-content.idle {
  animation: card-enter-idle 220ms ease-out;
}
.card-content.swap-exit {
  animation: card-exit-showing 220ms cubic-bezier(0.22, 1, 0.36, 1) forwards;
}
.card-content.idle.swap-exit {
  animation: card-exit-idle 220ms ease-out forwards;
}

@keyframes card-enter-showing {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes card-exit-showing {
  from { opacity: 1; transform: translateY(0); }
  to { opacity: 0; transform: translateY(-4px); }
}
@keyframes card-enter-idle {
  from { opacity: 0; }
  to { opacity: 1; }
}
@keyframes card-exit-idle {
  from { opacity: 1; }
  to { opacity: 0; }
}

@media (prefers-reduced-motion: reduce) {
  .card-content,
  .card-content.idle,
  .card-content.swap-exit,
  .card-content.idle.swap-exit {
    animation: none;
  }
}
```

The four selectors' specificity already resolves correctly as written —
verified by hand: `.card-content.idle` and `.card-content.swap-exit` are
equal specificity (0,2,0), but whenever both `idle` and `swap-exit` are
present together, `.card-content.idle.swap-exit` (0,3,0) is strictly
more specific than either and wins outright, so there's no tie-breaking
ambiguity to worry about. Don't reorder these rules looking for a
specificity fix — there isn't one needed. Keep the four keyframes'
`from`/`to` values exact, since those encode the animation this plan
must visually match.

**Verify**: `npx tsc --noEmit` → exit 0; `npx vitest run src/components/StatusRailCard.test.tsx` → all pass, including the existing no-remount-on-same-key test.

### Step 4: Replace the two news-pill enter animations

Replace `motion.span` with a plain `span` plus a CSS `animation` (no hook
needed — these are enter-only, as established in "Current state"):

```tsx
<span className="pill category">{newsCategory}</span>
{/* ...newsAge similarly, with its own class for the delay... */}
<span className="pill age">{newsAge}</span>
```

```css
.pill.category,
.pill.age {
  animation: pill-enter 220ms cubic-bezier(0.22, 1, 0.36, 1) both;
}
.pill.age {
  animation-delay: 70ms;
}

@keyframes pill-enter {
  from { opacity: 0; transform: translateY(3px); }
  to { opacity: 1; transform: translateY(0); }
}

@media (prefers-reduced-motion: reduce) {
  .pill.category,
  .pill.age {
    animation: none;
  }
}
```

(`both` fill-mode keeps the pill at its `from` state — invisible —
before the delayed animation starts, matching `motion`'s `initial` prop
holding the pre-animation state.)

**Verify**: `npx tsc --noEmit` → exit 0; `npx vitest run` → all pass.

### Step 5: Replace `Manifest.tsx`'s expand/collapse

Use a CSS Grid `grid-template-rows: 0fr → 1fr` transition — this is the
standard technique for animating to/from an unknown ("auto") height
without JS measuring a pixel value, and is well-supported in WebKit
(Safari has supported animatable CSS Grid track sizes since well before
this app's minimum target). No delayed-unmount is needed here — unlike
the card swap, the manifest content can simply stay in the DOM at all
times with zero height when collapsed, which is simpler and avoids
re-mounting the whole subtree (and re-running `renderInlineMarkdown`) on
every expand/collapse toggle:

```tsx
// remove: import { AnimatePresence, motion } from "motion/react";
// ...
return (
  <div className={`manifest-wrap${expanded ? " expanded" : ""}`} aria-hidden={!expanded}>
    <div className="manifest">
      {/* ...unchanged content (eventType === "news_item" ? ... : ...)... */}
    </div>
  </div>
);
```

```css
.manifest-wrap {
  display: grid;
  grid-template-rows: 0fr;
  opacity: 0;
  transition:
    grid-template-rows 240ms cubic-bezier(0.22, 1, 0.36, 1),
    opacity 240ms cubic-bezier(0.22, 1, 0.36, 1);
}
.manifest-wrap.expanded {
  grid-template-rows: 1fr;
  opacity: 1;
}
.manifest-wrap > .manifest {
  overflow: hidden;
  min-height: 0;
}

@media (prefers-reduced-motion: reduce) {
  .manifest-wrap {
    transition: none;
  }
}
```

The `aria-hidden={!expanded}` on the outer wrapper replaces the
accessibility behavior `AnimatePresence` gave for free by fully removing
collapsed content from the DOM (screen readers shouldn't announce
zero-height, visually-hidden manifest content) — this is a real
behavioral difference from the `motion` version worth calling out
explicitly, not an incidental detail: confirm it with a test (Step 6).

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 6: Test Manifest's collapsed-content accessibility

Add (or extend, in `StatusRailCard.test.tsx`, per "Current state"'s note
that there's no separate Manifest test file) a test asserting that when
`expanded` is `false`, the manifest wrapper carries `aria-hidden="true"`,
and when `true`, it does not.

**Verify**: `npx vitest run` → new/extended test passes.

### Step 7: Memoize `renderInlineMarkdown` (folds in plan 069)

In both `StatusRailCard.tsx` and `Manifest.tsx`, wrap the
`renderInlineMarkdown` call in `useMemo`, keyed on the string being
rendered:

```tsx
// StatusRailCard.tsx — keyed on renderedSlot.body (not the live slot.body,
// now that Step 3 introduced the frozen-during-exit renderedSlot)
const bodyContent = useMemo(
  () => renderInlineMarkdown(renderedSlot.state === "showing" ? renderedSlot.body : ""),
  [renderedSlot],
);
// Manifest.tsx — keyed on `body` (the existing prop, unaffected by this plan)
const messageContent = useMemo(() => renderInlineMarkdown(body), [body]);
```

(Adjust the exact `renderedSlot`-based key to whatever shape Step 3
actually landed on — the point is memoizing on the string content, not
recomputing it on every unrelated re-render, exactly as plan 069
originally specified for the pre-Step-3 `slot.body`.)

**Verify**: `npx tsc --noEmit` → exit 0; `npx vitest run` → all pass, same count as before this step (pure optimization, no behavior change).

### Step 8: Manual visual check (operator-owed)

Run `npm run tauri dev` (or `npx vite build && npx vite preview` if a
full Tauri dev loop isn't available in your environment) and eyeball:
idle→showing swap on a real push (`./notchtap --title t --body b`),
showing→idle after ttl expiry, expand/collapse via the hotkey, a
news-item card's category/age pills appearing, and a goal celebration
(unrelated pure-CSS animation — confirm it still works unaffected). If
you can't drive the GUI yourself, say so explicitly and leave this as
operator-owed in your completion report rather than claiming it passed —
this is exactly the class of check plans 018/023/032 all left to the
operator for the same reason (animation *feel* isn't something a test
suite verifies).

### Step 9: Bundle composition + full suite verification

**Verify**:
- `npx vite build` → exit 0; note the new overlay-chunk filename and size (should drop substantially from 332.24 kB / 106.37 kB gzip — record the new numbers in your completion report)
- `grep -c "motion" dist/assets/<new-overlay-chunk-name>.js` → 0
- `npx tsc --noEmit` → exit 0
- `npx vitest run` → all pass
- `npx biome ci .` → exit 0 (or matches pre-existing failure baseline exactly)

## Test plan

- `src/useDelayedSwap.test.ts` (new): same-key immediate sync, key-change delayed swap, rapid double key-change only fires once (Step 2).
- `StatusRailCard.test.tsx` (extended): the existing no-remount-on-same-key test must still pass unmodified; add an `aria-hidden` assertion for Manifest's collapsed state (Step 6).
- Verification: `npx vitest run` → all pass; re-confirm the exact pre/post test count delta in your completion report (this plan adds a new hook test file plus at least one new/extended assertion — no test should be *removed*).

## Done criteria

- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx vitest run` exits 0; the pre-existing no-remount-on-same-key test (`StatusRailCard.test.tsx`) passes unmodified
- [ ] `npx biome ci .` exits 0 (or matches pre-existing baseline)
- [ ] `npx vite build` exits 0; the overlay's main chunk is meaningfully smaller than 332.24 kB / 106.37 kB gzip (record the new number)
- [ ] `grep -c "motion" <the new overlay chunk>` returns 0
- [ ] `grep -rn "from \"motion/react\"" src/App.tsx src/components/StatusRailCard.tsx src/components/Manifest.tsx` returns no matches
- [ ] `grep -n "from \"motion/react\"" src/settings/SettingsApp.tsx` still returns matches (untouched, per Scope)
- [ ] Every new CSS animation/transition has a matching `@media (prefers-reduced-motion: reduce)` override, following the `styles.css:258-264` precedent
- [ ] Manifest's collapsed state carries `aria-hidden="true"`, verified by a test
- [ ] Manual visual check (Step 8) done, or explicitly flagged operator-owed
- [ ] `plans/069-memoize-inline-markdown.md`'s status marked SUPERSEDED by this plan in `plans/README.md`
- [ ] No *source* files outside the Scope section modified (`git status` — `plans/README.md` and `plans/069-memoize-inline-markdown.md` are expected to change too, per the two bullets above; everything else is out of scope)
- [ ] `plans/README.md` status row for 078 updated

## STOP conditions

- Any of the cited files don't match the excerpts above (drift since
  planning, especially if plan 069 was already independently executed —
  check its status in `plans/README.md` first; if it's DONE, its
  `useMemo` is already in place and Step 7 becomes a no-op verification
  rather than a fresh change).
- The `grid-template-rows` technique doesn't animate smoothly in the
  actual Tauri WKWebView (only discoverable via Step 8's manual check) —
  if the manual check reveals this doesn't work as expected, STOP and
  report rather than reaching for a JS-measured-height fallback
  unilaterally; that's a bigger change than this plan scoped.
- The rapid-double-key-change test (Step 2) reveals the timer cleanup
  doesn't behave as expected — this is exactly the kind of subtle timing
  bug worth stopping over rather than patching around blindly.

## Maintenance notes

- If `SettingsApp.tsx`'s two `motion.div` usages are ever migrated away
  too (not required by this plan), `motion` can be fully removed from
  `package.json` at that point — not before, since it's still a real
  dependency of the Settings window after this plan lands.
- `useDelayedSwap` is written generically (`<T>`) but only has one real
  caller today (`StatusRailCard`'s idle/showing swap) — if a second swap
  site ever needs the same "freeze old content during exit" behavior,
  reuse this hook rather than writing a second one.
- The four new keyframes (`card-enter-showing`/`card-exit-showing`/
  `card-enter-idle`/`card-exit-idle`) intentionally mirror the `motion`
  values byte-for-byte (same duration, same easing curve, same opacity/
  translateY deltas) — if the visual design ever changes intentionally,
  update both the "enter" and "exit" pair together, the same way a
  `motion` `initial`/`animate`/`exit` trio would have needed to move
  together.
