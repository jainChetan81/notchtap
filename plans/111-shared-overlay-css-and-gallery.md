# Plan 111: Kill the CSS mirror — one shared overlay stylesheet + a representative preview gallery

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report. The reviewer
> maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it prints
> anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check**: this plan REQUIRES 107 and 110 merged (they edit
> both CSS files under the mirror law; landing 111 first would force
> them to re-do their mirroring against a moved target). Verify:
> `ls plans/done/ | grep -E "^(107|110)"` → both present. Then diff
> your excerpts against post-110 master; on mismatch, re-read before
> proceeding — this plan's excerpts were written at `870cdeb` and WILL
> have drifted (that's expected; the structure, not the line numbers,
> is the contract).

## Status

- **Priority**: P2 (structural: every visual change today is a
  two-file synchronization exercise enforced only by convention)
- **Effort**: L
- **Risk**: MED-HIGH (restructures how both entry points load the
  card CSS; the preview must keep rendering the overlay faithfully)
- **Depends on**: 107, 110 (merged). 108/109 recommended merged too
  (settings.css churn).
- **Category**: refactor + tooling
- **Planned at**: commit `870cdeb`, 2026-07-22 (operator chose the
  REAL refactor over a parity-check stopgap)

## Why this matters

`src/settings/preview-overlay.css` (1,378 lines) is a hand-maintained
copy of the card-shape blocks of `src/styles.css` (1,762 lines),
scoped under `.appearance-preview`, governed by a "mirror law"
(DESIGN.html:1202: "change one, change both, in the same commit").
Plans 100/102/105/107/110 each paid the double-edit tax. Nothing but
review discipline detects a missed mirror. Separately, the Appearance
gallery previews only four expanded cards (`PREVIEW_SAMPLES`,
SettingsApp.tsx:1460) — no compact, live, weather, or idle states —
so the states most sensitive to CSS drift are exactly the ones the
preview can't show.

## Current state (verified 2026-07-22 at `870cdeb`; expect drift)

- `preview-overlay.css` header comment: "mirror of styles.css's
  cutout card shape block (mirror law)… selectors here are identical,
  scoped under .appearance-preview". No `@import`; imported directly
  at `SettingsApp.tsx:19`.
- `styles.css` also contains overlay-ONLY rules that the preview does
  NOT mirror (boot/window-level rules, hover tracking areas, etc.) —
  the mirror is a SUBSET. The split between "mirrored card shape" and
  "overlay-only" is implicit today; making it explicit is most of
  this plan's work.
- `.card-assembly` appears 41× in styles.css vs 31× in
  preview-overlay.css — the delta is the unmirrored overlay-only
  subset plus any accumulated drift (Step 0 finds out which).
- `PREVIEW_SAMPLES` (`:1460`): four fixtures, all `expanded: true`,
  all `details: []`, no espn meta: Goal, Red card, Generic cmux
  alert, News headline.
- Vite: two entry points (overlay `index.html` → `src/main.tsx`;
  settings entry importing `SettingsApp`). CSS is plain (no
  preprocessor). **(Corrected at review round 2)**: biome does NOT
  lint CSS here — `biome.json` has no css configuration, so no lint
  gate sees these files at all. That absence RAISES the value of
  this plan's structural checks: they are the only automated
  authority over the card CSS.

## Commands you will need

Frontend gates from root. Rust untouched (STOP otherwise). For the
parity proof in Step 0: plain node scripting in the scratchpad is
fine (not committed).

## Scope

**In scope**: `src/styles.css`, `src/settings/preview-overlay.css`
(deleted at the end), a NEW shared stylesheet (suggested:
`src/overlay-card.css`), `src/App.tsx`/`src/main.tsx` (import + root
class), `src/settings/SettingsApp.tsx` (import + preview wrapper
class + gallery fixtures), `src/settings/settings.css` (preview
frame chrome), affected test files, `DESIGN.html` (the mirror-law
paragraph), `docs/TESTING_STRATEGY.md` §0 last.

**Out of scope**: any VISUAL change to the overlay (this is a
refactor — the overlay's rendered CSS must be equivalent);
`src-tauri/` entirely; adding a CSS preprocessor or build tooling
(plain CSS + a scoping root class only — the toolchain-adoption plan
is separate and unwritten).

## Steps

### Step 0: diff the mirror (the map before the surgery)
Script: normalize both files (strip comments/whitespace), extract
(selector, declarations) pairs from preview-overlay.css with the
`.appearance-preview` prefix stripped, and diff against styles.css.
Output three lists: (1) identical pairs — the true shared set;
(2) preview-only divergences — each is either accumulated DRIFT (a
bug — report it) or a DELIBERATE preview adaptation (e.g. position
fixed→absolute, animation suppression, sizing for the preview frame);
(3) styles.css rules absent from the preview.
Classify every item in (2) AND every item in (3) by reading it.
**(Widened at plan review, 2026-07-22)** — list (3) is NOT
automatically the overlay-only set: a card-shape rule that the
mirror law required but that never got copied to the preview lands
in (3) looking identical to a genuinely overlay-only rule
(window-level, boot, hover-tracking). Classifying only (2) would let
one-sided drift stay in styles.css — preserving exactly the defect
this refactor exists to kill. So each (3) item gets a verdict:
**overlay-only** (stays in styles.css, with the section noted) or
**missing-mirror drift** (a mirror-law violation — joins the shared
set AND is reported as a found bug). **These classified lists are
the plan's most important artifact — paste them in the report.**
**STOP** if (2) OR (3) contains items you cannot classify, or if
classification reveals the preview deliberately diverges so broadly
that a shared file would need per-context overrides on most rules
(that would mean the mirror is really a fork, and the operator should
re-decide with that fact).

### Step 1: extract the shared stylesheet
1. Create `src/overlay-card.css` holding set (1) + set-(3)'s
   missing-mirror-drift items (+ set-(2) deliberate adaptations
   resolved — see Step 2), selectors rooted
   under a neutral class, suggested `.card-root` (NOT
   `.appearance-preview`, NOT anything overlay-specific). Keep the
   file organized in styles.css's existing section order with its
   comments — history and rationale comments move WITH the rules.
   **`.card-root` placement, made explicit at review round 2 and
   corrected at cold-read (2026-07-22)**: it goes on ONE wrapper
   element that is a strict ancestor of `.card-assembly` in BOTH
   entry points.
   - **Overlay**: `App.tsx:129-131` returns `<StatusRailCard …/>`
     DIRECTLY — there is no existing container, and
     `StatusRailCard`'s own root element IS `.card-assembly`. So a
     NEW wrapper element around `<StatusRailCard>` in `App.tsx` is
     REQUIRED (a plain `<div className="card-root">`); do not hunt
     for an existing one, it does not exist.
   - **Preview**: two candidate elements exist and they are NOT the
     same thing — `.appearance-preview` (`SettingsApp.tsx:1664`) is
     the SINGLE outer container hosting today's scoped copy;
     `.preview-row`/`.preview-stage` (`:1666-1668`) are the
     PER-SAMPLE wrappers. Put `.card-root` on each **per-sample
     `.preview-stage`** (each sample gets its own scope, matching
     the overlay's one-wrapper-per-card shape); `.appearance-preview`
     stays as the frame chrome only.
   The wrapper must contain EVERYTHING the shared selectors target
   (`.card-assembly` and all descendants, `.status-dots`, the
   below-block tree). Shared selectors become
   `.card-root <original-selector>` mechanically — no selector is
   rewritten beyond the prefix. State the final chosen elements for
   both entries in the report.
2. `styles.css`: delete the moved rules, `@import "./overlay-card.css"`
   (or import in `main.tsx` — match how CSS is imported today).
   **Import order, made explicit at review round 2**: the shared
   file loads BEFORE each context's own rules (overlay:
   `overlay-card.css` then `styles.css` residue; settings:
   `overlay-card.css` then `settings.css` then the override block —
   the override block does not exist yet, Step 2 creates it), so
   context overrides win by both order and specificity. Pin the
   order with a comment at each import site.
3. Settings: preview wrapper renders `.card-root` inside
   `.appearance-preview`; import the shared file; delete
   `preview-overlay.css` entirely.
### Step 2: per-context overrides, explicit and tiny
The deliberate adaptations from Step 0(2) become a SHORT block in
settings.css scoped `.appearance-preview .card-root { … }`, each with
a comment saying why the preview diverges (e.g. "preview frame is
position:static; the real overlay window is edge-anchored"). Target:
every override earns its line; if an adaptation can die instead
(preview tolerates the real rule), kill it.
**Verify (Steps 1+2)**: re-run the Step 0 script against the NEW
structure: shared-set parity is now structural (one file) — the
script instead asserts (a) `preview-overlay.css` no longer exists,
(b) styles.css contains no `.card-assembly`/card-shape rules outside
the import, (c) the override block is ≤ the deliberate-adaptation
count from Step 0. Commit a SMALL version of this check as a vitest
test so the mirror can never silently return —
**hardened at review round 2**: a naive substring test matches
`.card-assembly` inside COMMENTS (styles.css is comment-dense by
house style) and would false-positive; the committed check must
strip `/* … */` comments first, then match SELECTOR-shaped
occurrences only (the token appearing in a rule prelude — text
between a `}` or file start and the next `{`). Still string-level,
no CSS parser dependency; ~15 lines.

**Rendered-equivalence evidence (added at review round 2)** — the
"zero visual change" claim needs artifacts, not assertion:
1. BEFORE starting: `npx vite build` on the clean base and save
   `dist/`'s CSS asset(s) to the scratchpad.
2. AFTER Steps 1+2: rebuild and extract from both builds every rule
   whose selector touches the card tree (`.card-assembly`,
   `.status-dot`, `.below-block`, the flanks…), normalize
   (strip comments/whitespace, sort declarations within each rule),
   and diff. The ONLY expected differences: the `.card-root` prefix
   on shared selectors and rule-order moves that Step 2's import
   order makes non-observable. Any declaration-level delta is a
   defect — fix or classify it explicitly in the report.
3. If a GUI is available, a before/after screenshot of the same
   promoted card is welcome extra evidence but does not replace the
   built-CSS diff (screenshots can't cover every state).

### Step 3: gallery becomes representative
Extract `PREVIEW_SAMPLES` from SettingsApp.tsx into
`src/settings/previewFixtures.ts`. Extend to cover, at minimum:
- the four existing expanded samples (unchanged),
- one COMPACT (collapsed) card,
- one live ESPN card (fixture with espn meta + details — crib from
  an existing test fixture rather than inventing shapes; locate with
  `grep -rln "espn" src --include="*.test.*"` and copy the meta
  shape from whichever StatusRailCard/App test carries a live-match
  fixture),
- one weather alert card (wx details incl. `wx.is_day` from 110),
- one news card in compact state (exercises 110's single-timestamp).
Idle rail / idle hover / bare-notch / reduced-motion are OUT (they
are window-level states the preview frame can't honestly host —
note this in a comment so the review's fuller "state matrix" idea
has a recorded boundary).
Group the gallery UI by state (a labelled row per sample is enough —
no tabs needed at five-to-eight samples).
**Verify**: gallery test renders every fixture without error;
compact fixture asserts `.compact` presence; live fixture asserts
its live chip.

### Step 4: gates + §0 + DESIGN.html
Full frontend gates → clean. Rewrite DESIGN.html's mirror-law
paragraph: the law is dead; `overlay-card.css` is the single source,
the vitest structural check is the enforcement. §0 updated with
attribution.

## Done criteria

- [ ] `preview-overlay.css` deleted; `overlay-card.css` exists;
      overlay + preview both render through `.card-root`
- [ ] Step 0's classified diff in the report — BOTH directions:
      preview-only divergences (list 2) and styles-only rules
      (list 3, each verdicted overlay-only vs missing-mirror drift);
      drift items in either direction called out explicitly as found
      bugs
- [ ] Preview overrides ≤ deliberate-adaptation count, each commented
- [ ] Anti-mirror vitest check committed and passing —
      comment-stripped, selector-shaped matching (a `.card-assembly`
      mention inside a comment must NOT trip it; add that as a test
      case of the check itself)
- [ ] Normalized built-CSS diff (base vs refactor) in the report:
      only `.card-root` prefixes and non-observable order moves;
      declaration-level deltas fixed or explicitly classified
- [ ] Gallery covers compact/live/weather/news-compact + the original
      four; fixtures live outside SettingsApp.tsx
- [ ] `git diff master -- src-tauri/` → empty (this diffs against
      POST-110 master — 110 legitimately touched `src-tauri/`; the
      preflight `git merge --ff-only master` is what keeps the base
      current, so run it before trusting this check); zero intended
      visual change to the overlay (rendered-CSS equivalence argued
      in the report from the Step 0/Step 2 lists)
- [ ] All gates clean; §0 matches observed counts

## STOP conditions

- Step 0's classification stalls (unclassifiable divergences).
- The shared-file import order changes computed styles in the overlay
  (cascade order matters; if a moved rule loses a specificity war it
  previously won by source order, report rather than nudge
  specificity ad hoc).
- The live-ESPN fixture requires inventing wire shapes not found in
  any existing test fixture.
- 107/110 not merged (preflight).

## Maintenance notes

- After this, a card-CSS change is ONE edit; the preview inherits it
  by construction. The residual duty is the override block: keep it
  small, keep it commented.
- The gallery's recorded boundary (no window-level states) is the
  honest line between "preview" and the review's grander living-
  gallery idea; if the operator later wants that, it's a dev-only
  page (new plan), not more settings-window scope.
- The frontend toolchain spike (TS7/Vite8/Vitest4, GO but unadopted)
  would make this nicer with CSS modules — deliberately NOT taken
  here; adoption remains its own plan.
