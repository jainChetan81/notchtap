# Plan 063: idle status rail (460px) overlaps other apps' menu bar icons in notch mode

> **Executor instructions**: The decision is **locked** (see "Grilling
> session resolved" below, 2026-07-20) — do not re-litigate Options 1-3
> in "Decision needed"; that section and "Recommendation" right after it
> are **superseded**, kept only as background context for why the locked
> decision looks the way it does. Read "Grilling session resolved" as
> the authoritative scope. **This plan does not yet have a Steps/Scope/
> Test-plan/Done-criteria section** — see the third review-plan pass note
> in Status below for what's still missing before this is fully
> execution-ready; don't treat the absence of those sections as
> permission to improvise the implementation shape yourself.

## Status

- **Priority**: P1 — this actively obscures other apps' functional menu
  bar icons on real hardware, not a cosmetic nit.
- **Effort**: M (build) — decision is locked (was "S decision / S build,
  once decided" while still a spike; the locked mechanic — flex-wrap
  layout change, new rust→frontend plumbing for two facts, ellipsis
  truncation, shared-channel coordination with plan 060 — is bigger than
  the original "S build" estimate assumed a simple width cap would be)
- **Risk**: MED — same notch-mode-needs-hardware-verification caution as
  plan 060, but here the current behavior is already confirmed broken
  (operator screenshot on the MacBook), not just theoretically risky.
- **Depends on**: none (sibling to plan 060 — that one covers the
  notchless/HUD-mode top-spacing collision and consumes this plan's
  `__NOTCHTAP_MODE__` channel once it lands; this plan ships first).
  Plan 079 (overlay visual revamp) explicitly depends on *this* plan
  shipping first — its own "current state" baseline is this plan's
  shipped result, not a redo.
- **Category**: bug / direction
- **Planned at**: commit `40b4b0e`, 2026-07-19 — filed from an operator
  screenshot on the MacBook (real notch hardware): the idle rail's chip
  row (Football/News Paused/Weather off/Clear) extends far enough right
  that it visibly sits on top of — and the trailing "Clear" chip gets
  clipped by — the real macOS menu bar's own icon tray (several
  third-party app icons visible, partially obscured).
- **Review-plan pass (2026-07-20)**: own read + a fresh-context
  subagent cold-read, cross-checked. Every existing `file:line` citation
  in this plan was independently re-verified against live code and holds
  up exactly (`.rail-card.idle.status` at `styles.css:51-52`,
  `CutoutGeometry` at `presentation.rs:26-30`, the monitor-bounds
  fallback at `lib.rs:590-600`, window dims in `tauri.conf.json`,
  `center_x()` confirmed as `CutoutGeometry`'s only consumer anywhere in
  `src-tauri/`). Found and fixed three real gaps: (1) Options 2 and 3
  were written as if they might need no new plumbing, but both are
  explicitly notch-mode-specific behavior and the frontend has *zero*
  mode awareness today (per sibling plan 060) — both need the same new
  mode-boolean fact threaded to the frontend that Option 1 needs, just
  not Option 1's additional numeric cutout-width; the "Decision needed"
  and "Recommendation" sections below are rewritten to say so plainly,
  so the operator isn't misled into thinking 2/3 are zero-plumbing paths
  (only the separately-described universal "immediate stopgap" is).
  (2) Added a previously-unraised risk: `.src-chip` (`styles.css:495-506`)
  has `white-space: nowrap` and no `text-overflow`/`max-width` anywhere
  nearby — a live-match chip's label (`{label} · {minute}`, e.g. real
  team names) is unbounded and untruncated, so a single long live-match
  chip could challenge or exceed a "conservative" 300–320px cap on its
  own, undermining Option 2's "2 chips not 4" sizing assumption for
  exactly the content (a live score) this feature exists to surface —
  flagged as a caveat that applies regardless of which option is picked.
  (3) Fixed an uncited claim (the "270px" plain-clock width now cites
  `styles.css:43-44`) and a vague antecedent ("this README" → the
  specific file, `plans/README.md`). Also added a matching forward
  reference to plan 060 (which previously had no pointer back to this
  plan), so whoever builds 060 first knows a second consumer of the same
  mode-plumbing is coming and can build one shared channel instead of
  two single-purpose ones.
- **Second review-plan pass (2026-07-20, same day)**: re-verified drift
  after further concurrent landings elsewhere in the repo (plan 076).
  This plan's actual citations are unaffected: `styles.css`,
  `presentation.rs`, and `tauri.conf.json` (its primary evidence) show
  zero diff since planning; `lib.rs:590-600` (the monitor-bounds
  fallback comment, the one `lib.rs` citation this plan makes) is still
  byte-identical despite `lib.rs` gaining +8 lines elsewhere from plan
  076 — all five insertion points land either before or after the cited
  range in a way that doesn't shift it. Also cross-checked the shared
  `window.__NOTCHTAP_MODE__`/`__NOTCHTAP_CUTOUT_WIDTH__` channel naming
  against plan 060's copy — still exactly consistent, no divergence. No
  new issues found this pass; the plan is unchanged from the first pass.
- **Third review-plan pass (2026-07-20, same day)**: substantial change
  since the second pass — a concurrent `/grilling` session added the
  "Grilling session resolved" section below, locking a real decision
  (a hybrid mechanic none of the original 3 options fully described:
  `flex-wrap` on `.src-rail` instead of a single-row width cap,
  zero-margin `CutoutGeometry.width` as the notch-mode cap, ellipsis
  truncation for the live-match chip, no visual restyle) and filing a
  new sibling, plan 079. This left the file self-contradictory: the
  banner still said "decision spike," the Status block still said
  "S (decision)," and "Decision needed"/"Recommendation" below still
  presented the 3 options as open — while the locked section below them
  said the decision was already made and described a mechanic the
  options never listed. Fixed: banner and Status updated to reflect the
  locked state; "Decision needed"/"Recommendation" marked superseded
  below. Verified the locked decision's own technical claims directly:
  `.src-rail` (`styles.css:488-493`) confirmed to have no `flex-wrap`
  today (real, needed work, not already-present); `.rail-card`
  (`styles.css:22-36`) confirmed to have no fixed height (auto-sizes to
  content, supporting the "grows downward" claim); plan 079 confirmed to
  reference this plan correctly and bidirectionally. Found one small
  imprecision, fixed in place below: the claim that 270px covers both
  "`notchtap-detect` fails" and "reports `width: 0`" as if they were two
  cases — they're the same case. `presentation.rs`'s `cutout()`
  (`presentation.rs:52-63`) already normalizes `cutout_width <= 0.0` to
  `None` before this plan's code would ever see it, so there is no
  `Some(CutoutGeometry { width: 0.0, .. })` to additionally handle.
  **Biggest remaining gap, not fixed this pass**: the locked decision has
  no Scope/Steps/Test-plan/Done-criteria section at all — it reads as a
  design decision, not yet an executable plan. Flagged prominently in
  the banner rather than drafted here, since specifying it properly
  (exact wrap-height CSS, the precise eval-splice code shape, IdleView.tsx's
  exact chip-rendering structure) is real, substantial work in its own
  right — recommend a follow-up `plan` pass (not another `review-plan`
  pass) to write those sections before this is handed to an executor.

## Why this matters

This is the exact "idle manual checks operator-owed" gap flagged when
plan 034 shipped (`plans/README.md`'s 034 row: *"idle manual checks
operator-owed"*) — nobody had checked the widened status rail against a
real notch MacBook with a normal menu-bar icon load until now. The
result isn't just an aesthetic nit like plan 060 (HUD-mode top-spacing) —
it's the overlay actively covering *other applications'* UI, which is
close to the opposite of what a "permanent overlay" notification tool
should do. If a user can't see their own menu-bar icons (network status,
password manager, etc.) because notchtap is sitting on top of them,
that's a real usability regression, not a style preference.

## Current state

- `.rail-card.idle.status` (`src/styles.css:51-52`): `width: calc(460px
  * var(--card-scale))` — a single hardcoded width, used identically in
  both notch mode and HUD mode, with **zero knowledge of the actual
  notch width or available clearance**.
- `src-tauri/src/presentation.rs:26-30`: `CutoutGeometry { left_x,
  right_x, width }` — the real notch width **is already known** on the
  rust side, reported by `notchtap-detect` and used today only to
  compute `center_x()` for window positioning (`lib.rs`'s
  `position_window`). It is never surfaced to the frontend.
- `src-tauri/src/lib.rs:590-600` (`position_window`'s notch branch)
  already has a monitor-bounds safety fallback — *"if a result outside
  the current monitor falls back to top-center"* — but this only
  guards against the window rendering fully off-screen. It does not
  and cannot know where other apps' menu-bar icons currently are (no
  public macOS API exposes that — same category of wall as the
  Focus/DND rejection already recorded in `plans/README.md`'s
  "Operator-requested filing" note at the top of that file).
- The window itself is fixed at 500x300 (`tauri.conf.json:18-19`,
  `resizable: false`); card width is a CSS-only concern layered inside
  that fixed canvas via `margin: 0 auto`.

## Decision needed (operator) — SUPERSEDED, kept for background only

> The three options below were superseded by the `/grilling` session
> recorded in "Grilling session resolved" further down — that section
> locked a hybrid mechanic none of these three fully describe. Read this
> section only to understand *why* the locked decision looks the way it
> does (e.g. why wrapping beat a pure width cap); do not implement any
> of Options 1-3 as written.

All three options below are notch-mode-specific behavior — none of them
is a pure CSS-only change. The frontend has **zero** notch-vs-HUD mode
awareness today (confirmed: no file under `src/` references presentation
mode at all — same fact sibling plan 060 established). Concretely, this
means every option needs *at least* a new boolean fact ("am I in notch
mode?") threaded from rust to the frontend before it can apply
selectively; they differ in how much *more* than that boolean they need:

1. **Cap the rail width in notch mode specifically**, using the real
   `CutoutGeometry.width` to compute a safe max width (e.g. notch width
   + some fixed margin on each side), falling back to today's 460px only
   in HUD mode where there's no notch to respect. This preserves chip
   richness on wider notches (newer MacBook Pro models have wider
   notches than older MacBook Air models) instead of a
   one-size-fits-none constant. **Needs**: the mode boolean, *plus* the
   numeric `CutoutGeometry.width` value — the larger plumbing lift of
   the three.
2. **Never widen past a conservative fixed cap in notch mode**
   (e.g. 300-320px — a rough estimate, not a computed figure; see the
   live-match-chip caveat below), accepting that some idle chips may
   need to scroll/cycle or simply not all show at once on narrow-notch
   machines. **Needs**: the mode boolean only, *not* the numeric
   cutout-width — simpler than Option 1, but still real new plumbing,
   not a zero-plumbing change. (The genuinely zero-plumbing path is the
   separate "immediate stopgap" described below, which applies the cap
   universally in both modes rather than gating it on notch mode.)
3. **Don't widen the idle rail in notch mode at all** — keep it at the
   270px plain-clock width (`.rail-card.idle`, `src/styles.css:43-44`)
   always in notch mode, and only show the richer status chips in HUD
   mode (where there's no notch to respect, so 460px is genuinely safe)
   or when a card is actually promoted (compact/expanded already don't
   have this problem — they're positioned by the same notch-anchor logic
   but the operator hasn't reported an overlap complaint about those,
   only the idle rail). **Needs**: the same mode boolean as Option 2 —
   this option is not a pure-CSS "just don't apply the `.status` class
   modifier" change; the frontend still has to *know* it's in notch mode
   to withhold the modifier there specifically.

**Caveat that applies to Option 2 (and to a lesser extent 1) regardless
of the exact number chosen**: `.src-chip` (`src/styles.css:495-506`) has
`white-space: nowrap` and no `text-overflow`/`max-width` anywhere near
it — chip text never truncates. The live-match chip's label
(`{live.label} · {live.minute}`, built from real team names in
`IdleView.tsx`) is unbounded, unlike the other three chips' fixed short
labels ("Football off," "News paused," etc.) — a real matchup ("Manchester
City 2-1 Arsenal · 78'") could be long enough on its own to challenge or
exceed a 300-320px cap, which would either overflow the cap anyway
(defeating the fix) or need truncation logic this plan doesn't currently
scope. Whichever option is picked, decide explicitly whether the
live-match chip needs its own truncation/ellipsis handling — don't
assume the fixed labels' typical lengths represent the worst case.

## Recommendation — SUPERSEDED, kept for background only

Option 1 is the most correct long-term fix (uses the information the
app already has, degrades gracefully across different MacBook notch
widths) but requires the larger new rust→frontend plumbing (mode
boolean *plus* `CutoutGeometry.width` — or a computed safe-max-width —
through the existing one-shot boot-time eval-splice pattern already used
for other static-at-boot facts). Option 2 is a same-day mitigation if
the operator wants relief before the real fix lands and is willing to
build the smaller mode-boolean plumbing now: pick one conservative
width, ship it, revisit properly later — but see the live-match-chip
caveat above before picking a number. Option 3 is the safest but most
visually regressive (loses the idle-rail feature's whole value in the
one mode — notch — where it's most likely to be seen), and still needs
the same mode-boolean plumbing as Option 2, not less.

**If the operator wants an immediate stopgap while this plan is
pending**: the single lowest-risk change is narrowing
`.rail-card.idle.status`'s width from 460px to something conservative
(e.g. 320px) universally (both modes) — small, immediate, testable on
this Mac mini in HUD mode, but still needs the MacBook check before
calling it actually safe. Not applied here; this plan intentionally
stops at "decision needed," matching this session's operator-set
scoping instruction to hold off on anything needing hardware
verification only the operator can do.

## Grilling session resolved (2026-07-20)

A `/grilling` session against this file (initially a straight walk through
Options 1-3 above) reached a locked decision that supersedes the three
options — worth stating plainly since it's a hybrid of Option 1 with a
mechanic none of the three original options described:

- **Direction**: Option 1's spirit (cutout-aware, degrades across
  hardware) but via **wrapping**, not a single-row width cap. `.src-rail`
  gets `flex-wrap`; the card grows downward into the window's already-
  unused vertical space (the window is a fixed 300px tall, `resizable:
  false`; `.rail-card` itself has no fixed height — it's auto-sized to
  content, currently ~38-50px) instead of trying to fit every chip on
  one line at a computed width.
- **Width cap in notch mode**: exactly `CutoutGeometry.width`, **zero**
  outward margin — the card is never wider than the physical notch
  cutout itself, since menu-bar icons live entirely outside that span.
  (An earlier draft of this session considered a 20px/side margin; that
  was only useful when trying to cram a full row of chips into one
  line, which wrapping makes unnecessary.)
- **270px does double duty**: it's both (a) the fallback width when
  `CutoutGeometry` is unavailable, and (b) an absolute floor — even a
  real, very narrow notch's exact width never shrinks the card below
  270px. One constant, two jobs; wrapping means being conservative here
  costs nothing but an extra row. (Precision check, third review-plan
  pass: "unavailable" is the *only* case (a) needs to handle — a report
  of `width: 0` is not a second, separate scenario. `presentation.rs`'s
  `DetectOutput::cutout()` already normalizes any `cutout_width <= 0.0`
  to `None` before this plan's code ever sees it — `Some(CutoutGeometry
  { width: 0.0, .. })` cannot occur.)
- **Live-match chip truncation**: the live chip stays today's single
  joined label string (no wire-shape change in this plan — that's
  deferred to plan 079). When it alone is wider than the capped card,
  it gets ellipsis-truncated (`text-overflow: ellipsis` + `max-width:
  100%` on `.src-chip`), not wrapped onto a second line internally.
- **No visual restyle** — this plan ships using today's existing
  small-monospace-pill-chip design exactly as-is. The bolder-typography/
  team-badge/structured-match-data direction the reference image
  prompted is real, but out of scope here — filed separately as plan
  079 (a consolidated overlay-visual-revamp decision session), so this
  plan stays a narrow mechanics fix, not a redesign.
- **HUD mode**: untouched, stays at today's 460px (no notch to respect).
- **Shared plumbing ownership**: since this plan (P1) ships before plan
  060 (P2, HUD-mode visual merge) and both need
  `window.__NOTCHTAP_MODE__`, this plan builds that shared boolean
  itself (plan 060's already-pinned shape) plus
  `window.__NOTCHTAP_CUTOUT_WIDTH__`, via the existing `.on_page_load`
  eval-splice call site (`lib.rs:393-457`). Plan 060 consumes the
  boolean once it lands, rather than inventing its own.
- Still needs the real MacBook smoke-check with a realistic menu-bar
  icon load before calling this done, per the existing maintenance note
  below.

## Maintenance notes

- Coordinates directly with plan 060 — both are instances of "the
  frontend has zero awareness of notch-vs-HUD mode or actual screen
  geometry beyond a fixed card width," and (per the review-plan pass
  above) both independently need the same new mode-boolean fact
  threaded to the frontend. Whichever of 060/063 lands first should
  build ONE shared "presentation facts" channel (mode boolean, with room
  to add the cutout-width number later if Option 1 is picked) rather
  than each inventing its own single-purpose one-shot boot fact —
  plan 060 has been given a matching forward-pointer to this plan for
  the same reason. **Concrete shape now pinned down in plan 060's own
  review-plan pass (2026-07-20)**: `window.__NOTCHTAP_MODE__ = "notch" |
  "hud"`, delivered via the existing `.on_page_load` eval-splice call
  site (`lib.rs:393-457`) alongside `__NOTCHTAP_SLOT_STATE__`/
  `__NOTCHTAP_APPEARANCE__`. Per the "Grilling session resolved" section
  below, this plan builds **both** `__NOTCHTAP_MODE__` and
  `__NOTCHTAP_CUTOUT_WIDTH__` unconditionally (the locked decision needs
  the numeric cutout width, not just the mode boolean) — extend that
  same eval call with both, rather than inventing a second channel; see
  plan 060 for the full rationale on the mode-boolean half.
- Whoever builds this needs a real MacBook smoke-check with a realistic
  menu-bar icon load (not a nearly-empty menu bar) before calling it
  done — same discipline plan 060 and the hardware-only checks in
  plans 010/012/018/023/032/033/034 already require.
