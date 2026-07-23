# Plan 126: settings-window motion polish (tokens, transition-all, ActionStatus + list motion)

> Filed 2026-07-23 from the /improve-animations audit (findings #8,
> #10 + missed opportunities: ActionStatus fade, Queue/History list
> motion). Authored at master `304d078`. Scope: settings-side files
> ONLY — `src/settings/SettingsApp.tsx`, `src/settings/actionStatus.tsx`,
> `src/settings/sections/QueueSection.tsx`,
> `src/settings/sections/HistorySection.tsx`,
> `src/components/ui/button.tsx`, `src/components/ui/badge.tsx`,
> their test files, and the §0 frontend count line. Do NOT touch
> src/components/* overlay files, overlay-card.css, or
> animationTiming.ts beyond importing existing exports.

## Changes

1. **Import the guarded ease (finding #8).**
   `SettingsApp.tsx:381`:
   `transition={{ duration: 0.16, ease: [0.22, 1, 0.36, 1] }}` →
   `ease: NOTCHTAP_EASE` imported from `../animationTiming`. No other
   change on that line.
2. **Kill the leftover `transition-all` (finding #10).**
   - `src/components/ui/button.tsx:8`: `transition-all` →
     `transition-[color,background-color,border-color,box-shadow,transform]`
     (the press feedback `active:translate-y-px` needs transform; the
     rest matches what switch.tsx's remediation kept). If the repo's
     tailwind-merge config chokes on the arbitrary list, fall back to
     `transition-colors transition-transform` equivalent utilities —
     verify the press still animates.
   - `src/components/ui/badge.tsx:8`: `transition-all` →
     `transition-colors`.
   These are locally-owned shadcn copies (CLAUDE.md: components copied
   + owned) — editing them is in-pattern; keep the diff minimal.
3. **ActionStatus messages animate like their sibling ErrorPanel
   (missed opp).** `actionStatus.tsx:141` currently returns `null` /
   bare `<div>` with no mount motion while `SettingsApp.tsx:199-219`'s
   ErrorPanel wraps the same banner shape in `AnimatePresence` with
   `{opacity, y:-3}` enter/exit. Wrap ActionStatus's rendered state in
   the SAME pattern (`AnimatePresence` + `motion.div`,
   `initial={{ opacity: 0, y: -3 }}`, `animate={{ opacity: 1, y: 0 }}`,
   `exit={{ opacity: 0, y: -3 }}`, duration 0.16, `NOTCHTAP_EASE`) —
   copy the ErrorPanel exemplar's shape, don't invent a new one. The
   2.5s ok-auto-clear (`:98-100`) now plays an exit fade instead of
   blinking out. aria-live semantics must NOT change: keep the
   announcement element/role exactly as-is (motion wrapper goes around
   or inside such that screen-reader behavior and the plan-108
   transition-only rules are untouched — the existing actionStatus
   tests are the guard).
4. **Queue + History list motion (missed opp).**
   - First give queue rows stable keys: `QueueSection.tsx:74`'s
     biome-ignored `key={\`${index}:${item.title}\`}` →
     `key={\`${item.priority}:${item.source}:${item.title}:${occurrenceIndex}\`}`
     where `occurrenceIndex` counts duplicate identical summaries
     (compute with a running Map — stable across a refetch that
     returns the same list; remove the biome-ignore).
   - Wrap both lists' rows in `AnimatePresence initial={false}` +
     `motion.li` with height+opacity collapse:
     `initial={{ opacity: 0, height: 0 }}`,
     `animate={{ opacity: 1, height: "auto" }}`,
     `exit={{ opacity: 0, height: 0 }}`, duration 0.18,
     `NOTCHTAP_EASE`, `style={{ overflow: "hidden" }}`.
     `initial={false}` means the first render (section open, Refresh
     wholesale swap) does NOT play a cascade — only genuine row
     appearance/removal animates. Skip current then reads as "that one
     row left"; Clear collapses the rows it removes.
   - HistorySection (`:296-297`): same treatment, same values (its
     entries already have stable-enough identity — reuse whatever the
     row map keys on today if unique, else derive as above).
5. **Tests.** Existing ones stay green unmodified (the ActionStatus
   transition-only/announce tests are the contract — if any asserts
   exact DOM nesting that the motion wrapper changes, repointing the
   selector is fine, weakening an assertion is not). Add: queue row
   keys are stable across a refetch of an identical list (no remount —
   assert via element identity or motion-mount spy); button keeps its
   press transform property in the transition list (string-level pin).

## Verification

`npx tsc --noEmit`, `npx vitest run`, `npx biome ci .`, `npx vite
build` — all clean. §0 frontend line only. Visual feel of the list
collapse is operator-verified — flag it.

## STOP conditions

- ActionStatus's aria-live/announce test contract can't survive the
  wrapper without weakening.
- height:"auto" animation janks structurally in jsdom tests (assert
  presence/props, not pixels — if a test needs real layout, pin
  structure instead).
