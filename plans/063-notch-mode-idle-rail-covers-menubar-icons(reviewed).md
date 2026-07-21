# Plan 063: idle status rail (460px) overlaps other apps' menu bar icons in notch mode

> **Executor instructions**: The decision is **locked** (see "Grilling
> session resolved" below, 2026-07-20) — do not re-litigate Options 1-3
> in "Decision needed"; that section and "Recommendation" right after it
> are **superseded**, kept only as background context for why the locked
> decision looks the way it does. Read "Grilling session resolved" as
> the authoritative scope. The Scope/Steps/Test-plan/Done-criteria
> sections were added in the fourth review-plan pass (2026-07-20) — this
> plan is now execution-ready. Follow the steps in order, run every
> verification, and honor the STOP conditions.
>
> **Drift check (run first)**: `git diff --stat 9a954b0..HEAD -- src/styles.css src/components/IdleView.tsx src/App.tsx src-tauri/src/lib.rs src-tauri/src/presentation.rs src-tauri/tauri.conf.json`
> Any diff in these files means line refs below have shifted — re-read
> before editing. The `.src-rail`/`.src-chip` citations were re-verified
> in the fourth pass (they moved from `:488-506` to `:560-580` when plan
> 078 added ~100 lines of keyframes above them).

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
- **Fourth review-plan pass (2026-07-20, plan pass)**: the execution
  sections (Commands/Scope/Steps/Test plan/Done criteria/STOP
  conditions) are now written below, against commit `9a954b0`. Every
  citation re-verified by direct read: `.src-rail` moved to
  `styles.css:560-565` and `.src-chip` to `styles.css:567-580`
  (`white-space: nowrap` at `:576`) after plan 078's keyframe additions
  (the `:488-506` refs in the superseded sections below are stale —
  trust the new sections); `.rail-card*` `:22-53`, `presentation.rs`
  `:14-63`, and the `on_page_load` eval-splice site `lib.rs:394-460`
  unchanged. Confirmed by direct read that `mode` and `cutout` are
  already in scope inside the `on_page_load` closure (both `Copy`,
  destructured at `lib.rs:114`, the closure is `move`) — Step 1 needs
  no new plumbing channels, only a fourth eval block.
- **Review-plan pass (2026-07-21, post-merge)**: code executed and
  merged at `4fb3af9`; re-verified at HEAD `647f6d0` that the merged
  code still matches the locked mechanic exactly, despite plans
  080–086 landing on top of it. Confirmed live by direct read: the
  notch clamp is byte-exact to Step 3's spec
  (`:root[data-notchtap-mode="notch"] .rail-card.idle.status` →
  `calc(clamp(270px, var(--notchtap-cutout-width, 270px), 460px) *
  var(--card-scale))`, now `styles.css:60-62`); the HUD 460px rule is
  byte-identical to pre-063 (`styles.css:51-53`, diffed against
  `9a954b0`); `.src-rail` wraps with the plan-sanctioned merged
  `gap: 4px 6px` (`styles.css:597-603` — 082/084's additions shifted it
  from the step-time `:560` refs; rules intact, refs merely moved);
  `.src-chip.live` truncation + `.live-label` (`styles.css:623-643`,
  the max-width/min-width/flex block merged into the existing `.live`
  color rule — functionally identical to Step 4); `IdleView.tsx:20-25`
  has the `.live-label` span; `lib.rs` eval block at `:476-488`,
  `cutout_width_js_value` at `:611` with both Step-1 tests
  (`:826-837`); `presentationFacts.ts`/`.test.ts` and the
  `App.tsx:32-35` mount effect all present; `presentation.rs` has zero
  diff since `9a954b0` (out-of-scope honored;
  `tauri.conf.json`'s only post-063 diff is 083's crest
  `assetProtocol` — window dims untouched). 080–086 regression check:
  no conflict found. One interaction worth naming: plan 085's
  `resting_state: "notch"` early-null (`StatusRailCard.tsx:244-254`)
  skips the idle rail entirely when opted in — it bypasses, not
  breaks, this plan's clamp (default `"rail"` renders the clamped
  rail), but the Step 5 MacBook smoke-check must be run with
  `resting_state` at its default `"rail"`, otherwise checks (a)–(c)
  are unobservable. **Sole remaining work before DONE**: Step 5's
  operator-owed MacBook smoke-check (checks a–d; (d), the Mac
  mini/HUD byte-identity check, can be done locally), then flip the
  `plans/README.md` 063 row and rename this file `(done)`.
- **Review-plan pass (2026-07-21, at `958c2f7`)**: re-verified after
  plan 087 (hover primitive) and plans 068/072/074 merged. The merged
  063 code is intact and byte-identical where it matters: the HUD 460px
  rule (`styles.css:51-53`) and the notch clamp (`:60-62`) are at the
  same lines and unchanged; `presentation.rs` still has zero diff since
  `9a954b0`. Pure line shifts from 087's additions (content verified
  identical): `.src-rail` wrap `:597-603`→`:608-614`, `.src-chip.live`
  truncation `:623`→`:635`, `.live-label` →`:647`; `lib.rs` eval block
  `:476-488`→`:563-571`, `cutout_width_js_value` `:611`→`:696`, its two
  tests `:826-837`→`:977-988`; `App.tsx` mount effect `:32-35`→`:36-39`
  (087's hover-changed listener landed above it); `IdleView.tsx`
  `.live-label` span now `:22`. **One genuinely new coupling, added to
  Maintenance notes below**: plan 087's `src-tauri/src/hover.rs:27-33`
  hardcodes rust mirrors of this plan's width constants
  (`IDLE_STATUS_WIDTH = 460.0`, `NOTCH_CLAMP_MIN = 270.0`,
  `NOTCH_CLAMP_MAX = 460.0`, citing `styles.css` by line) for the
  hover-rect math. If the operator MacBook smoke-check (the sole
  remaining work) leads to adjusting any width/clamp number, the CSS
  and `hover.rs` must change in the SAME commit — `hover.rs:402-405`'s
  named-constant tripwire test pins the rust side only; nothing
  automated catches a CSS-only change. Sole remaining work is
  unchanged: Step 5's operator-owed MacBook smoke-check with
  `resting_state` at its default `"rail"`.

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

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Clippy | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust format | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint/format | `npx biome ci .` | exit 0 |
| Full gate (optional, mirrors CI) | `just test-all` | all green (`just` needs `brew install just` on this machine) |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/lib.rs` — add the presentation-facts eval block in `on_page_load` (Step 1) + its unit-testable helper.
- `src/lib/presentationFacts.ts` — **create** (Step 2).
- `src/lib/presentationFacts.test.ts` — **create** (Step 2's tests).
- `src/App.tsx` — consume the facts, set the root dataset attribute + CSS var (Step 2).
- `src/styles.css` — mode-aware width cap, `.src-rail` wrap, chip truncation rules (Steps 3–4).
- `src/components/IdleView.tsx` — wrap the live chip's label text in a truncatable span (Step 4).
- The IdleView render test file that covers the live chip, if one exists (`src/components/IdleView.test.tsx` or the IdleView section of `src/App.test.tsx`/`src/components/StatusRailCard.test.tsx` — find it with `grep -rn "src-chip\|IdleView" src/*.test.* src/components/*.test.*` and extend the live-chip case there).

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/presentation.rs` — `detect_mode`/`CutoutGeometry` already return everything needed and are fully tested; no changes.
- `src-tauri/src/lib.rs` `position_window` (`:581-600`) — positioning already consumes `mode`/`cutout`; untouched.
- The live-match chip's wire shape (`status.football.live = { label, minute }`) — a joined-string change is plan 079/083 territory, explicitly deferred by the locked decision.
- Any visual restyle of the chips (colors, typography, padding) — "no visual restyle" is part of the locked decision.
- `src/settings/preview-overlay.css` — the mirror law does **not** apply here: the preview renders promoted cards, not the idle rail's `.status` width mechanics; the `.src-chip` base styles are untouched (only `.src-chip.live`/`.live-label` additions). If you find yourself editing a rule that exists in both files, STOP and re-read this paragraph.
- HUD-mode behavior — it stays at 460px by construction; any diff that changes the HUD render is a bug in the step, not a choice.

## Steps

### Step 1: rust — splice `__NOTCHTAP_MODE__` and `__NOTCHTAP_CUTOUT_WIDTH__` at page load

In `src-tauri/src/lib.rs`'s `.on_page_load` closure (`:394-460`), add a fourth block right after the appearance block (`:445-460`). `mode` and `cutout` are already in scope — both are `Copy`, destructured at `lib.rs:114` (`let (mode, inset, cutout) = presentation::detect_mode(&config);`), and the closure is `move`, so it captures its own copy with no signature changes anywhere.

Follow the appearance block's pattern exactly, but note the values are a constant string and a JSON number-or-null, so no `escape_for_eval_splice` is needed (the mode string is rust-generated, never user data):

```rust
// plan 063: presentation facts for the frontend — the mode boolean and
// the numeric cutout width, one eval, same page-load site as the other
// boot facts. plan 060 will consume __NOTCHTAP_MODE__ when it lands.
{
    let mode_str = match mode {
        presentation::Mode::Notch => "notch",
        presentation::Mode::Hud => "hud",
    };
    let width_json = cutout_width_js_value(cutout);
    let _ = webview.eval(format!(
        "window.__NOTCHTAP_MODE__ = \"{mode_str}\"; window.__NOTCHTAP_CUTOUT_WIDTH__ = {width_json};"
    ));
}
```

Add the helper as a free function near `position_window`, so it's unit-testable without a webview:

```rust
fn cutout_width_js_value(cutout: Option<presentation::CutoutGeometry>) -> String {
    match cutout {
        Some(c) => format!("{}", c.width),
        None => "null".into(),
    }
}
```

(`cutout_width <= 0.0` cannot occur — `presentation.rs:52-63`'s `cutout()` already normalizes it to `None`, per the third review-plan pass. Do not add a second zero-guard.)

**Verify**: `cd src-tauri && cargo build` → compiles; `cargo test --locked` → all existing tests pass (no behavior change yet).

### Step 2: frontend — read the facts and expose them to CSS

Create `src/lib/presentationFacts.ts`, following the seed-read pattern of `useSlotState.ts:130-132` (validate the global, fall back safely, never throw on malformed input):

```ts
// plan 063: boot-time presentation facts spliced by the rust core at
// page load (lib.rs's on_page_load). Mode gates notch-only CSS; the
// cutout width feeds the idle status rail's width clamp (styles.css).
export type PresentationMode = "notch" | "hud";

declare global {
  interface Window {
    __NOTCHTAP_MODE__?: unknown;
    __NOTCHTAP_CUTOUT_WIDTH__?: unknown;
  }
}

export function presentationFacts(): { mode: PresentationMode; cutoutWidth: number | null } {
  const mode: PresentationMode =
    window.__NOTCHTAP_MODE__ === "notch" ? "notch" : "hud";
  const w = window.__NOTCHTAP_CUTOUT_WIDTH__;
  const cutoutWidth = typeof w === "number" && Number.isFinite(w) && w > 0 ? w : null;
  return { mode, cutoutWidth };
}
```

In `src/App.tsx` (it already reads the `__NOTCHTAP_APPEARANCE__` seed at `:20`), add a mount effect that applies both facts to the document root:

```ts
useEffect(() => {
  const { mode, cutoutWidth } = presentationFacts();
  document.documentElement.dataset.notchtapMode = mode;
  if (cutoutWidth !== null) {
    document.documentElement.style.setProperty("--notchtap-cutout-width", `${cutoutWidth}px`);
  }
}, []);
```

**Verify**: `npx tsc --noEmit` → exit 0; `npx vitest run` → all pass (new tests from the Test plan below included).

### Step 3: CSS — mode-aware width cap, floor, and `.src-rail` wrap

In `src/styles.css`:

1. `.src-rail` (`:560-565`) — add wrapping. Locked decision: chips wrap onto extra rows and the card grows downward into the window's unused height, instead of clipping or overflowing:

```css
.src-rail {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  row-gap: 4px;
  gap: 6px; /* keep the existing column gap — merge into `gap: 4px 6px` if biome prefers */
  min-width: 0;
}
```

2. `.rail-card.idle.status` (`:51-52`) — keep the existing 460px rule exactly as-is (it is now the HUD-mode path and the no-facts fallback), and add the notch-mode clamp right after it:

```css
/* plan 063: in notch mode the idle status rail caps at exactly the
   physical cutout width, zero outward margin — menu-bar icons live
   outside that span. 270px does double duty: fallback when the cutout
   width never arrived, and absolute floor for very narrow notches.
   460px ceiling: never wider than today's HUD width. */
:root[data-notchtap-mode="notch"] .rail-card.idle.status {
  width: calc(clamp(270px, var(--notchtap-cutout-width, 270px), 460px) * var(--card-scale));
}
```

Note the clamp is **inside** the `--card-scale` multiplication — the floor/cap apply to the unscaled design width, the user's appearance scale still applies on top. Do not reorder.

**Verify**: `npx biome ci .` → exit 0; `npx vite build` → succeeds.

### Step 4: live-match chip ellipsis truncation

`.src-chip` (`styles.css:567-580`) has `white-space: nowrap` (`:576`) and is `display: inline-flex` — a bare text node inside it cannot ellipsis on its own, so this is markup + CSS:

1. `src/components/IdleView.tsx:19-23` — wrap the joined label in its own span:

```tsx
<span className="src-chip live">
  <span className="live-dot" aria-hidden="true" />
  <span className="live-label">{live.label} · {live.minute}</span>
</span>
```

2. `src/styles.css` — let the live chip (and only it) shrink below its content width, and truncate the label span:

```css
/* plan 063: the live-match chip is the only chip with unbounded text
   (real team names) — it may shrink and ellipsis; every other chip
   keeps its natural width. Locked: truncate, never wrap internally. */
.src-chip.live {
  max-width: 100%;
  min-width: 0;
  flex: 0 1 auto;
}
.src-chip .live-label {
  display: block;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

Do not add `max-width`/`text-overflow` to the base `.src-chip` — the locked decision scopes truncation to the live chip.

**Verify**: `npx biome ci .` → exit 0; `npx vitest run` → all pass (IdleView live-chip test updated per the Test plan).

### Step 5: full gate + operator hardware check

**Verify** (all must pass before handoff):
- `cd src-tauri && cargo test --locked` → all pass
- `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` → exit 0
- `cd src-tauri && cargo fmt --check` → exit 0
- `npx vitest run` → all pass
- `npx tsc --noEmit` → exit 0
- `npx biome ci .` → exit 0

Then hand to the operator for the **MacBook smoke-check** (manual, required by the locked decision): run the built app on the notch MacBook with a realistic menu-bar icon load and confirm (a) the idle status rail never extends past the cutout's own width, (b) chips wrap onto a second row instead of clipping when they don't fit, (c) a long live-match label ellipsizes, (d) the Mac mini (HUD mode) is byte-for-byte unchanged — still 460px, single row. That check is operator-owed; do not mark the plan DONE in `plans/README.md` until it passes.

## Test plan

- **Rust** (`src-tauri/src/lib.rs`'s existing `#[cfg(test)]` module, or wherever `position_window`'s tests live — match that location): `cutout_width_js_value(Some(CutoutGeometry { left_x: 480.5, right_x: 799.5, width: 319.0 }))` returns `"319"`; `cutout_width_js_value(None)` returns `"null"`. No other rust tests — no behavior change beyond one eval string.
- **Frontend, new file** `src/lib/presentationFacts.test.ts` — model after `src/useStatusState.test.ts`'s seed tests (`:84-90` set/delete the global between cases): (a) `__NOTCHTAP_MODE__ = "notch"` + width `319` → `{ mode: "notch", cutoutWidth: 319 }`; (b) mode `"hud"` or garbage/missing → `"hud"`; (c) width `0`, `-5`, `"319"`, missing → `cutoutWidth: null`.
- **IdleView live chip** — in whichever existing test file renders IdleView with a live status (find it per the Scope section): assert the live chip now contains a `.live-label` span whose text is `{label} · {minute}`. Extend the existing case; do not weaken any existing assertion.
- **CSS behavior (width clamp, wrap, ellipsis)** is manual-only per `docs/TESTING_STRATEGY.md` §5 — no jsdom layout assertions. The MacBook smoke-check in Step 5 is the verification.
- Verification: `npx vitest run` → all pass, including the new `presentationFacts` suite (+4 tests) and the extended IdleView case; `cd src-tauri && cargo test --locked` → all pass (+2 new).

## Done criteria

- [ ] `window.__NOTCHTAP_MODE__` and `window.__NOTCHTAP_CUTOUT_WIDTH__` are eval-spliced in `lib.rs`'s `on_page_load` (one new block, no other rust changes; `git diff src-tauri/src/presentation.rs` is empty)
- [ ] `document.documentElement.dataset.notchtapMode` is set on mount; `--notchtap-cutout-width` is set only when the splice carried a positive number
- [ ] `:root[data-notchtap-mode="notch"] .rail-card.idle.status` clamps to `clamp(270px, var(--notchtap-cutout-width, 270px), 460px)`; the plain `.rail-card.idle.status` 460px rule is byte-identical
- [ ] `.src-rail` wraps (`flex-wrap: wrap` + row gap); the live chip truncates via `.live-label`, base `.src-chip` untouched
- [ ] `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` all exit 0
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] Operator MacBook smoke-check passed (all four checks in Step 5) — plan stays TODO/IN PROGRESS in `plans/README.md` until then
- [ ] `plans/README.md` status row for 063 updated

## STOP conditions

Stop and report back (do not improvise) if:

- The code at any "Current state"/Steps citation doesn't match (drift since `9a954b0`) — re-read and re-verify before editing; on a real mismatch, stop.
- `mode`/`cutout` are no longer in scope inside the `on_page_load` closure (e.g. detection moved into a managed-state struct) — the plumbing shape changes; stop and report rather than inventing a new channel.
- The wrapped idle rail grows taller than the 300px window can hold at the 270px floor with a realistic chip load (4 chips + a live match) — that contradicts the locked "grows downward into unused vertical space" mechanic; stop and report the measured heights.
- You find the 460px default rule needs to change to make the notch clamp work — that means the fallback structure is wrong; the HUD path must stay byte-identical.
- Any test outside this plan's own new/extended cases needs its assertions changed to pass.

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
- **Post-087 coupling (2026-07-21)**: `src-tauri/src/hover.rs:27-33`
  now mirrors this plan's width numbers as rust constants (270 floor,
  460 cap, plus the cutout clamp shape) for the tracking-area rect. Any
  future change to `.rail-card.idle.status`'s widths or the notch
  clamp in `styles.css` MUST update `hover.rs`'s constants in the same
  commit, or the hover rect silently diverges from the rendered card.
  The tripwire test (`hover.rs:402-405`) pins the rust constants but
  cannot see CSS — the lockstep is manual review discipline.
