# Plan 102: Slow the card's expand/contract morphs slightly (320→400ms, 240→300ms)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in "STOP conditions" occurs, stop and
> report. The reviewer maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight (run first)**: `git log --oneline master ^HEAD`;
> if it prints anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check (run second)**: plan 100 just changed these same files
> (TTL bar + celebration durations). This plan was stamped AFTER that
> merge — `git log --oneline -5` must show `Merge plan 100`. Locate
> targets by CONTENT (the values below), not by line number.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plan 100 (merged — same files)
- **Category**: polish
- **Planned at**: commit `9893543`, 2026-07-21

## Why this matters

Operator feedback (2026-07-21, same session as plans 100/101): the
card's size morphs — width changes between idle/showing/expanded, the
corner-rounding handoff, and the manifest expand/collapse — read
slightly too fast at real size. Slightly slower, not sluggish: +25%.

## Current state

- `src/styles.css`:
  - `.card-assembly` rule: `transition: width 320ms cubic-bezier(0.22, 1, 0.36, 1);`
  - the two flank rounding rules
    (`.card-assembly:not(:has(.below-block)) .flank-left` / `.flank-right`):
    `transition: border-radius 320ms ease;`
  - `.manifest-wrap` (plan 078 expand/collapse): a `transition:` shorthand
    whose first line is `grid-template-rows 240ms cubic-bezier(0.22, 1, 0.36, 1),`
    followed by its sibling property line(s) (opacity et al.) — every
    duration inside THIS one shorthand changes 240→300ms.
- `src/settings/preview-overlay.css` — mirror file: `width 320ms` (~:29),
  two `border-radius 320ms` (~:66/:70), and the `.manifest-wrap` mirror
  if present (grep `240ms`). MIRROR LAW: identical values in both files,
  same commit.
- Idle-peek open/close (260ms animations), `.chip` background 260ms,
  `card-enter-showing 220ms`, and every celebration duration (plan 100's
  doubles) are NOT part of this — sizes only, the four rules above.

## Commands you will need

`npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` / `npx vite build`
from the worktree root — all must pass/exit 0.

## Scope

**In scope**: `src/styles.css`, `src/settings/preview-overlay.css` —
duration values in the four named rules only.
**Out of scope**: every other duration, all easings, all keyframes, all
TS/TSX, `docs/TESTING_STRATEGY.md` (no test changes possible here).

## Steps

### Step 1: Retime
In both files: `width 320ms` → `width 400ms` (card-assembly rule only);
both `border-radius 320ms` → `border-radius 400ms` (flank rules only);
every `240ms` inside `.manifest-wrap`'s transition shorthand → `300ms`.
Easing curves untouched. Add a one-line plan-102 comment on the
card-assembly rule ("+25% pacing, operator feedback").

**Verify**:
- `grep -n "width 400ms" src/styles.css src/settings/preview-overlay.css` → 1 hit each
- `grep -c "border-radius 400ms" src/styles.css` → 2 (and mirror → 2)
- `grep -n "320ms" src/styles.css src/settings/preview-overlay.css` → remaining hits (if any) are NOT in the four named rules — list each with context in your report
- `grep -n "240ms" src/styles.css` → no hit inside `.manifest-wrap`'s transition

### Step 2: Gates
`npx vitest run` (214 expected), `npx tsc --noEmit`, `npx biome ci .`,
`npx vite build` → all clean; `git status` → only the two CSS files.

## Done criteria

- [ ] All Step 1 greps as stated; Step 2 gates clean
- [ ] Only the two CSS files modified

## STOP conditions

- The `Merge plan 100` commit is absent from master (dependency unmet).
- A rule named above can't be found by content (drift).
- Any duration change would land outside the four named rules.

## Maintenance notes

- These four durations are the card's "morph tempo"; if the later
  idle-transition choreography work (unfiled) adopts motion/layout
  animations, these values become its baseline feel to match.
- Reviewer: mirror-law diff of the two files; confirm celebration
  durations from plan 100 are untouched.
