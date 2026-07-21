# Plan 091: The cutout card shape + time-and-dots idle (079 items 1+2)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **HARD PREREQUISITE — plan 090 must be DONE and merged before dispatch.**
> This plan builds on 090's decided geometry rules (cutout exempt from
> `--card-scale`; every card width capped at the window). If
> `src/styles.css:61` still multiplies the cutout term by
> `var(--card-scale)`, 090 has not landed — STOP.
>
> **Drift check (run first)**:
> `git diff --stat <re-stamp after 090 merges>..HEAD -- src/styles.css src/components/IdleView.tsx src/components/StatusRailCard.tsx src/App.tsx src-tauri/src/hover.rs src/settings/preview-overlay.css`
> Expected: empty. On any diff, compare the "Current state" excerpts
> against live files; on a mismatch, STOP.

## Status

- **Priority**: P2 — the visual identity the whole 079 revamp exists for;
  every other remaining visual item (19, hover consumers, timeline) builds
  on this shell.
- **Effort**: L — the largest CSS restructure this repo has attempted;
  plus the `hover.rs` mirror and an `IdleView` rewrite.
- **Risk**: MED-HIGH — touches the shell of every card a user sees, both
  modes. Mitigations: content DOM is deliberately NOT restyled (item 19),
  the suite pins content behavior, and the shape has a locked, twice-photo-
  corrected reference (`prototype/notch-states.html`).
- **Depends on**: **plan 090 (hard, see above)**. 085 (`resting_state`,
  DONE — interplay specified below). 087 (hover primitive, DONE — its
  `active_card_rect` must be re-derived here, same-commit lockstep).
- **Category**: direction
- **Planned at**: commit `<fill and re-verify at dispatch, after 090>`,
  2026-07-21

## Why this matters

Plan 079's most-locked decision — confirmed across two rounds of real
macOS photos and an operator-approved prototype — is that the overlay
stops being a floating rounded rectangle and becomes a **permanent notch
cutout**: app-drawn black blocks that flow out of the (real or synthetic)
notch, with the notch's rectangle itself never touched by content. The
idle state becomes glanceable furniture: time on one side, three status
dots on the other. This is the difference between "a notification window
near the notch" and "the notch is the product." Everything shipped in
plans 080-087 (news card, TTL bar, weather art, scorecard, hover) renders
*content*; this plan finally builds the *shell* they all sit in. Item 19
(restyling the general card content into this language) is explicitly a
separate follow-up — this plan moves existing content into the new shell
byte-preserved wherever possible.

## Decisions of record (all operator-locked — do not re-litigate)

1. **Shape** (079 item 1, final form): three plain blocks — never
   clip-path. `.flank-left`/`.flank-right` sit either side of the cutout
   at the notch's own height; `.below-block` (when present) carries
   everything underneath, full width. The cutout's own base corners stay
   perfectly **square** (sharp rectangular bite). Rounding exists ONLY at
   the two true outer ends of the visible shape — on the flanks when they
   are the bottom edge (idle), on `.below-block` when it exists.
   **Rounding both at once is the documented double-curve gap bug — see
   the prototype's own comment block at `prototype/notch-states.html:25-31`
   and `:112-119`.**
2. **Fill** (2026-07-21 session): the shape is **always opaque `#000`**.
   `card_opacity` stops affecting the card shell entirely (the config
   field stays; Appearance-UI retirement is a later cleanup, out of scope
   here). Recorded in plan 090's Decision section.
3. **Scale** (plan 090 Q1a/Q2b, landed): `--card-scale` never multiplies
   `--notchtap-cutout-width`; every assembly width is capped at the
   window (`min(…, 100%)`).
4. **Idle content** (079 item 2, operator: "nailed"): time on one flank,
   three status dots (Football green / News red / Weather yellow; glow +
   pulse when active, dim flat when the source is disabled) on the other.
   The old idle furniture — text pills, day-progress timeline — is
   REMOVED from resting idle. The timeline's future home is the
   hover-expanded idle state (item 18 decision), which is NOT built here.
5. **Three idle-family states**: collapsed (bare cutout — this is 085's
   `resting_state: "notch"` today, unchanged), idle (time+dots), and
   expanded-on-hover (weather-mood scene) — **the hover state is NOT in
   this plan** (087-consumer follow-up); build collapsed + idle only.
6. **Both modes, no branch**: the shape is identical in notch and HUD
   mode. In notch mode the hardware provides the black cutout; in HUD
   mode (no hardware notch — the mac mini) the app draws a synthetic
   cutout block, pure `#000`, same rectangle, so the assembly is visually
   identical. (This is the only reading consistent with the locked
   "permanent cutout + no mode branch" record and the superseded "neck"
   paragraph's synthetic-cap-in-HUD idea; the cutout rectangle obviously
   cannot be a transparent hole on a notchless screen. If the operator
   contradicts this at review, STOP — but do not ask preemptively.)

## Current state (verify each before editing)

- `src/styles.css:25,39,44,52,61` — the five width rules (post-090 form:
  scale-capped, cutout exempt). `:29` — the shell fill
  `rgba(5, 6, 7, var(--card-opacity))` this plan replaces with `#000`.
  `:13` — container `overflow: hidden`.
- `src/styles.css` — `.rail-card` is the single rounded-rect shell every
  state renders into today; `.idle-view` + `.timeline` (~`:574`) are the
  old idle furniture this plan deletes from the resting state.
- `src/components/IdleView.tsx` (53 lines) — renders the old text-pill
  rail + timeline; rewritten here to time + dots. Its test file pins the
  old furniture and will be rewritten with it.
- `src/components/StatusRailCard.tsx` — `:253-263` the 085 early-return
  (comment `:253-260`, the
  `if (!renderedShowing && !exiting && restingState === "notch") return null;`
  at `:261-263`): **keep byte-identical**; it IS the collapsed state. The showing/exiting branches render content
  components (news/weather/live/compact/manifest) that move INSIDE
  `.below-block` unchanged.
- `src/App.tsx:13` sets `--card-scale`; `:36-42` sets
  `--notchtap-cutout-width` only when the measured cutout is non-null
  (notch mode). HUD mode currently leaves it unset → CSS fallback.
- `src-tauri/src/hover.rs` — `active_card_rect` mirrors the CSS widths
  via named constants (`BASE_WIDTH` 400, `EXPANDED_WIDTH` 500,
  `IDLE_WIDTH` 270, `IDLE_STATUS_WIDTH` 460 at `:30-36`), with the
  CSS↔rust lockstep rule at `:101-103` and the named-constant tripwire
  test. Post-090 the `Mode::Notch` arm is scale-exempt.
- `src/settings/preview-overlay.css` — the mirror-law copy of card CSS
  (same-commit rule; see the `mirror` greps in recent plans).
- The locked reference: `prototype/notch-states.html` — shape CSS at
  `:36-60` (three blocks, sizing off `--cw`/`--nw`/`--nh`), idle markup
  at `:211-241` (§2), dot styling at `:64-74`, the promoted-phase flank
  behavior in §4's `.flow-card` rules (time stays on the left flank;
  status dots HIDE during compact/expanded), and the two rounding-bug
  comment blocks named above.

## Geometry contract (the numbers, so CSS and hover.rs cannot diverge)

All as CSS custom properties AND matching named constants in `hover.rs`:

- `--notchtap-cutout-width`: measured hardware value in notch mode
  (unchanged); in HUD mode App.tsx now sets it to the **synthetic cutout
  constant 200px** (new; today it's unset in HUD). One source of truth
  for both modes; the CSS fallback stays as a belt-and-suspenders.
- Notch height `--notchtap-cutout-height`: **the height source is
  `safe_area_top_inset`** — `CutoutGeometry`
  (`src-tauri/src/presentation.rs:26-30`) has NO height field, but
  `DetectOutput.safe_area_top_inset` (`presentation.rs:41`, returned by
  `detect_mode` at `:66`) IS the notch height in points; expose it
  through `lib.rs`'s existing eval-splice block exactly the way
  `cutout_width_js_value` (`lib.rs:719` area) exposes width. HUD
  synthetic height: 32px, set by App.tsx like the width.
- Flank width (idle): `85px * var(--card-scale)` each (prototype's
  320-vs-150 proportion) → **idle assembly width =
  cutout + 2 × 85px × scale**, capped at the window.
- Showing (compact) assembly: `--cw` =
  `min(max(400px * scale, cutout + 2 * 60px * scale), 100%)` — the 60px
  minimum flank is scaled like every other design width (only the cutout
  term is scale-exempt, per 090).
- Expanded assembly: same formula with a 500px base.
- Flank height in every state = `--notchtap-cutout-height` (the
  prototype's `height: var(--nh)`).
- **The `100%` cap**: the assembly's parent must span the full overlay
  window — mount the assembly directly under the existing full-window
  container (`src/styles.css:13`'s `overflow: hidden` element). In
  `hover.rs` the same cap is `WINDOW_WIDTH` (500.0), stated explicitly
  in the rewritten doc comment.
- `hover.rs` mirrors these formulas exactly, in the same commit. The OLD
  width constants (`BASE_WIDTH`, `EXPANDED_WIDTH`, `IDLE_WIDTH`,
  `IDLE_STATUS_WIDTH`, `NOTCH_CLAMP_MIN`/`MAX`, `hover.rs:30-36`) are
  **REMOVED**, replaced by `FLANK_IDLE=85.0`, `MIN_FLANK_SHOWING=60.0`,
  `BASE_SHOWING=400.0`, `BASE_EXPANDED=500.0`, `HUD_CUTOUT_W=200.0`,
  `HUD_CUTOUT_H=32.0`; the named-constant tripwire test is rewritten
  around the new set. The idle/idle-status width split (plan 034's
  270/460 distinction) **deliberately collapses** — the new idle has one
  width formula regardless of status chips, because the dots replace the
  chip rail entirely. Say this in the commit message so plan-034
  archaeology doesn't read it as an accident.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass (re-derive baseline live at dispatch) |
| Rust lint/fmt | `cargo clippy --locked --all-targets -- -D warnings` / `cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass; several rewritten (see Test plan) |
| Typecheck / lint / build | `npx tsc --noEmit` / `npx biome ci .` / `npx vite build` | exit 0 |

`cargo` may need `export PATH="$HOME/.cargo/bin:$PATH"`.

## Scope

**In scope**:
- `src/styles.css` (major restructure of the shell; content-component
  rules untouched except where the shell requires reparenting context)
- `src/settings/preview-overlay.css` (mirror, same commit)
- `src/components/IdleView.tsx` + `IdleView.test.tsx` (rewrite)
- `src/components/StatusRailCard.tsx` (shell markup only: wrap existing
  content components in the three-block assembly; content components'
  internals untouched)
- `src/App.tsx` (HUD synthetic cutout var; height var)
- `src-tauri/src/lib.rs` (ONLY if `CutoutGeometry` height needs exposing
  through the existing eval-splice block — mirror how width is done)
- `src-tauri/src/hover.rs` (rect re-derivation + tests, same commit as
  the CSS)
- `src/App.test.tsx`, `src/components/StatusRailCard.test.tsx` (update
  shell-level assertions only)
- `docs/TESTING_STRATEGY.md` §0 (counts)

**Out of scope** (do NOT touch):
- Content components: `Manifest.tsx`, `TtlBar.tsx`, `Track`, the
  news/weather/live-match render branches' internals — item 19 restyles
  them later; here they are moved, not modified. If a content component
  *breaks* inside `.below-block` (layout assumption on the old shell),
  STOP and report rather than restyling it.
- The hover-expanded idle state, weather-peek, scorecard reveal, TTL
  hover-pause — all 087-consumer follow-ups.
- `src/components/StatusRailCard.tsx:244-254` — 085's collapsed
  early-return stays byte-identical.
- `capabilities/`, `build.rs`, all pollers/queue/engine — no backend
  behavior change (only the optional height eval-splice line).
- `prototype/` — reference only, never edited.
- The old `.timeline` must be DELETED, not hidden — but its *data
  plumbing* (day-progress calculation, if any lives outside IdleView)
  is left in place for the item-18 hover build to reuse.

## Steps

### Step 1: Shape machinery in CSS (no consumers yet)

Port the prototype's three-block system into `src/styles.css` as new
rules (`.card-assembly`, `.flank-left`, `.flank-right`, `.below-block`,
`.synthetic-cutout`), using the Geometry contract's formulas, fill
`#000`, and the rounding law (outer ends only; square bite). The
`.synthetic-cutout` block renders ONLY in HUD mode
(`:root[data-notchtap-mode="hud"]`); in notch mode the same rectangle is
simply empty space the hardware fills. Port the prototype's two comment
blocks about the double-curve gap — future editors need the warning, not
just the rule.

**Demolition scope (explicit — this replaces, it does not coexist):**
- `.rail-card` and its width rules (`src/styles.css:25-61`) are
  **deleted**, superseded by the assembly. Non-geometry properties it
  carried (typography, text color, the priority-accent classes, any
  box-shadow) migrate onto the new blocks — grep for every
  `.rail-card`-scoped selector and re-home each one deliberately; list
  the migrations in your report.
- **Outer-end rounding uses `var(--card-radius, 14px)`** — the user's
  radius setting stays meaningful on the new shape (it is cosmetic,
  unlike opacity which the Decision record retires from the shell).
- Any selector left referencing `.rail-card` after this step is a bug:
  `grep -c "rail-card" src/styles.css` must reach 0 by Step 4's end
  (component classnames change with it).

**Verify**: `npx vite build` exit 0; `grep -c "clip-path" src/styles.css`
→ 0.

### Step 2: HUD cutout vars in App.tsx (+ height exposure if needed)

App.tsx sets `--notchtap-cutout-width: 200px` and
`--notchtap-cutout-height: 32px` when mode is HUD; notch mode sets both
from measured geometry (width already does; height per the Geometry
contract — check `CutoutGeometry` first, STOP if height is absent).

**Verify**: `npx vitest run` — existing presentationFacts/App tests pass
(update any that pin the old "unset in HUD" behavior).

### Step 3: Rewrite IdleView to time + dots

Left flank: `HH:MM` (24h, minute-tick via the existing clock source in
IdleView — reuse its timer pattern). Right flank: three dots in fixed
order Football/News/Weather, coloring and states per the prototype
(`active` = glow + 1.8s pulse; `dim` = 0.22 opacity flat). Active/dim
derives from the SAME per-source status the old rail used
(`useStatusState` — enabled sources glow, disabled dim; do not invent new
semantics). Delete the pill rail and `.timeline` from resting idle.
Respect `prefers-reduced-motion`: pulse animation off, glow static.

**Verify**: `npx vitest run` — rewritten `IdleView.test.tsx` passes (see
Test plan).

### Step 4: Reparent showing/exiting states into the assembly

**Ownership (explicit — resolves who renders the flanks):**
`StatusRailCard` owns the `.card-assembly` in ALL states — it renders
the flanks and (when showing/exiting) `.below-block`. The flank-time
element becomes a small new `FlankClock` component (extract the timer
pattern from today's `IdleView`) rendered by StatusRailCard in both the
idle and showing branches — one component, two call sites, so the clock
is genuinely shared rather than duplicated. `IdleView` shrinks to the
right-flank dots content (rename to `StatusDots` if that's cleaner —
executor's call, but StatusRailCard is the single assembly owner either
way, and `IdleView.test.tsx`'s rewrite follows whatever shape you pick).

Showing/exiting states: left flank = `FlankClock`, right flank =
**empty** (the prototype hides the dots during compact/expanded; the
stamp/priority furniture that replaces them there is item-19 content,
not built here — leave a one-line comment marking the item-19 hook).
Content renders inside `.below-block`; flanks go square (below-block owns
the outer rounding, per the rounding law). The 085 collapsed early-return
stays untouched. Exiting animation continues to work (the existing exit
classes apply to `.below-block` now).

**Verify**: `npx vitest run` — StatusRailCard suite passes with updated
shell selectors; all content-level assertions (news meta, TTL bar,
scorecard, weather mood classes) pass UNCHANGED — if a content assertion
has to change, that's a scope smell: STOP and re-read the Out-of-scope
list.

### Step 5: Re-derive `active_card_rect` in hover.rs (same commit as CSS)

New formulas per the Geometry contract; update the doc comment, the
named-constant tripwire, and every affected test. Idle rect = cutout +
2×85×scale capped; showing/expanded per their formulas; collapsed
(`resting_state: notch`) unchanged (would-be footprint rule from 087).
Scale-exemption of the cutout term carries over from 090 — do not
reintroduce it.

**Verify**: `cd src-tauri && cargo test --locked hover::` all pass, with
the updated tests present and at least one new test per changed formula.

### Step 6: Mirror to preview-overlay.css

Same-commit mirror of the new shell rules so the Settings preview shows
the new shape. (The preview still has no cutout-var plumbing — 090 noted
this; give the preview fixed `--notchtap-cutout-width: 200px` so it
renders the HUD form. One line, not a preview overhaul.)

**Verify**: `grep -c "flank-left" src/styles.css src/settings/preview-overlay.css`
→ ≥1 in each.

### Step 7: Full gates + counts

All seven commands; update `docs/TESTING_STRATEGY.md` §0 (frontend row
will move; rust row moves only if hover tests changed count).

**Verify**: all exit 0; §0 matches live counts exactly.

## Test plan

- `IdleView.test.tsx` (rewrite): renders time; three dots in order; per-
  source active/dim from status state; no `.timeline` in resting idle;
  reduced-motion disables the pulse.
- `StatusRailCard.test.tsx` (shell-level updates only): showing content
  renders inside `.below-block`; flank time present during showing;
  dots absent during showing; 085 collapsed still renders nothing
  (existing test must keep passing untouched).
- `App.test.tsx`: HUD mode sets the synthetic cutout vars; notch mode
  uses measured.
- `hover.rs`: per-formula tests incl. scale-invariance of the cutout
  term (090's test carries forward) and the extended named-constant
  tripwire.
- Content pins (news/weather/live/TTL) must pass UNCHANGED — they are
  this plan's regression harness proving the move didn't restyle.

## Done criteria

- [ ] All seven gates exit 0; §0 counts match live
- [ ] `grep -c "clip-path" src/styles.css` → 0
- [ ] `grep -c "flank-left" src/styles.css src/settings/preview-overlay.css` → ≥1 each
- [ ] `grep -n "card-opacity" src/styles.css` shows NO use on the shell
      fill (pure `#000`); the CSS var may still exist elsewhere/unused
- [ ] `grep -c "timeline" src/components/IdleView.tsx` → 0 (or the file
      is renamed/absorbed per Step 4, in which case: no `timeline` in any
      component)
- [ ] `grep -c "rail-card" src/styles.css src/settings/preview-overlay.css` → 0 in each
- [ ] The 085 early-return (the `restingState === "notch"` null return,
      at StatusRailCard.tsx:261-263 pre-plan) survives byte-identical —
      locate it by content, not line number, since this plan moves code
      around it
- [ ] `hover.rs` and `styles.css` changed in the same commit
- [ ] All content-component test assertions pass unchanged
- [ ] No files outside Scope modified; `plans/README.md` row updated

## STOP conditions

- Plan 090 not merged (the styles.css:61 check in the header).
- `safe_area_top_inset` turns out NOT to reach the point in `lib.rs`
  where the eval-splice is built (i.e. exposing it needs new plumbing
  beyond mirroring the width path) — report the actual data flow; do not
  restructure `presentation.rs`.
- Any content component breaks inside `.below-block` such that fixing it
  requires editing its internals.
- The rounding law forces a case not covered by the prototype (a state
  where neither flanks nor below-block is unambiguously the bottom edge).
- The 085 early-return needs modification for any reason.
- Exit animations cannot be preserved without restructuring
  `useDelayedSwap` — report; that hook is load-bearing and pinned.
- Operator contradicts the HUD synthetic-cutout reading (Decision 6).

## Maintenance notes

- Item 19 builds ON this shell (content into the new language); the
  hover-consumers plan builds the third idle state (weather-mood
  expanded, timeline's new home) on the same assembly. Neither should
  need to touch the block machinery itself.
- The CSS↔hover.rs lockstep now covers the Geometry contract's four new
  constants — anyone changing flank/cutout numbers must change both files
  in one commit (the tripwire test enforces the rust side).
- The MacBook smoke for this plan supersedes 063's owed checks (a)-(c) —
  the new idle assembly replaces the clamped rail those checks describe.
  Verify at the next MacBook sitting: assembly hugs the real cutout at
  scale 1.0 AND at "Large" (cutout must not move), square bite, outer
  rounding only, pure black seamless against hardware, HUD form on the
  mini identical in language.
- **Test-harness honesty note (from this plan's cold read)**: the
  "content moved not modified" guarantee is only as strong as the content
  tests — `.below-block`'s padding/inheritance context differs from the
  old shell, so content can shift visually without any test failing. The
  operator eye-pass at the smoke sitting is the real verification of
  content fidelity; the tests only prove DOM/behavior survived.

**Review-plan pass (2026-07-21, at authoring)**: fresh-context cold read
ran before first dispatch and found 5 issues, all fixed in the body: the
height source (CutoutGeometry has no height field — `safe_area_top_inset`
at `presentation.rs:41`/`:66` is the height, now specified with an
exposure path and a narrowed STOP); the 085 early-return citation was
wrong (:244-254 → :253-263, and the done criterion now locates it by
content); the demolition scope was unspecified (now explicit: `.rail-card`
deleted, old hover.rs constants removed, plan-034's idle/idle-status
split deliberately collapses, radius keeps `var(--card-radius, 14px)`);
Step 4's flank ownership was contradictory (now: StatusRailCard owns the
assembly, `FlankClock` extracted and shared); and the 60px min-flank now
scales like every other design width. Remaining known placeholders: the
drift-check SHA and Planned-at are deliberately unstamped until plan 090
merges — stamping them is the dispatch precondition.
