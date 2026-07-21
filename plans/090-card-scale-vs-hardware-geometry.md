# Plan 090: `--card-scale` breaks notch geometry and overflows the overlay window

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. **Step 0 is an operator decision — do NOT proceed past
> it on your own judgment.** If anything in the "STOP conditions"
> section occurs, stop and report. When done, update the status row for
> this plan in `plans/README.md` — unless a reviewer dispatched you and
> told you they maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat e09725c..HEAD -- src/styles.css src-tauri/src/hover.rs src-tauri/src/settings.rs src-tauri/tauri.conf.json src/App.tsx`
> Expected: empty. On any diff, re-read the "Current state" excerpts
> against the live files before editing; on a mismatch, treat it as a
> STOP condition.

## Status

- **Priority**: P1 — one click in Settings ("Card shape → Scale →
  Large") reinstates the exact menu-bar-overlap bug plan 063 was filed
  as P1 to fix, and independently clips cards against the window edge
  **in both modes, including the Mac mini**.
- **Effort**: S (fix) once Step 0 is decided; the decision itself is the
  real work.
- **Risk**: MED — touches the CSS/`hover.rs` duplicated-constants pair
  that has an explicit lockstep rule, and changes two currently-green
  tests. Notch-mode results need MacBook verification.
- **Depends on**: nothing. **Blocks**: plan 079's item 1 (the cutout
  shape rebuild) — see "Why this matters" §4. Related: plan 063, which
  shipped the defect and whose own smoke-check is still owed.
- **Category**: bug
- **Planned at**: commit `e09725c`, 2026-07-21

## Why this matters

`--card-scale` is a user-facing cosmetic preference (Settings → Card
shape → Scale). It is currently multiplied into **every** card width,
including two that are not cosmetic dimensions at all. Three concrete
defects follow, all reachable from the shipped UI.

### 1. A hardware measurement is multiplied by a cosmetic preference

`src/styles.css:61` caps the notch-mode idle rail to the physical notch
cutout, then scales the result:

```css
width: calc(clamp(270px, var(--notchtap-cutout-width, 270px), 460px) * var(--card-scale));
```

`--notchtap-cutout-width` is a *measured hardware value* — it comes from
`NSScreen` via the `notchtap-detect` subprocess, through
`CutoutGeometry`, and is set in `src/App.tsx:39-41`. The notch does not
get wider when the user picks a bigger card.

Plan 063's locked intent (`plans/README.md`'s 063 row) was "exactly
`CutoutGeometry.width`, **zero margin**". At the 319px cutout that
plan uses as its own fixture (`src-tauri/src/lib.rs:983`):

| `card_scale` | rendered rail | overhang per side |
|---|---|---|
| 1.0 | 319px | 0 — as designed |
| **1.15** (Settings "Large") | 366.9px | **~24px** |
| 1.4 (max via IPC) | 446.6px | ~64px |

Roughly an icon's width of menu-bar overlap per side at one click, and
at the ceiling the fix retains about a tenth of its value. Plan 063 knew
about the ordering — its `:506` says *"the clamp is **inside** the
`--card-scale` multiplication … Do not reorder"* — but no risk entry,
STOP condition, done criterion, or smoke-check step in that plan
mentions the consequence, and its smoke-check runs only at 1.0 where
the behavior is correct.

### 2. Scaled cards overflow the fixed overlay window — in BOTH modes

The overlay window is fixed at **500×300** (`src-tauri/tauri.conf.json:18-19`),
never resized at runtime (the only `inner_size` call, `src-tauri/src/lib.rs:906`,
is the *settings* window), and its container is `overflow: hidden`
(`src/styles.css:13`). Card widths (`src/styles.css:25,39,44,52`) are
all `Npx * var(--card-scale)`:

| rule | base | at 1.15 ("Large") | at 1.4 |
|---|---|---|---|
| `.rail-card` `:25` | 400 | 460 ✓ | 560 ✗ |
| `.rail-card.expanded` `:39` | 500 | **575 ✗** | 700 ✗ |
| `.rail-card.idle` `:44` | 270 | 310 ✓ | 378 ✓ |
| `.rail-card.idle.status` `:52` | 460 | **529 ✗** | 644 ✗ |

✗ = wider than the 500px window, so clipped left and right.

This is **not** notch-specific — it applies to HUD mode, i.e. the Mac
mini. `.rail-card.idle.status` is the resting idle rail whenever status
chips are present, so at "Large" the everyday resting state is clipped
by ~15px per side. `src-tauri/src/hover.rs:134` confirms the geometry:
`x_min = (WINDOW_WIDTH - width) / 2.0` goes negative.

This defect is **independent of the notch issue** and would survive a
notch-only fix.

### 3. The Rust hover mirror reproduces both, and argues for it

`src-tauri/src/hover.rs:126-132` applies `* scale` to every branch
including `Mode::Notch => cutout_width.clamp(...)`, and its doc comment
at `:113-117` states the multiply as a *requirement*:

> `scale` is `Config.appearance.card_scale` … EVERY width below is
> multiplied by it in `styles.css` via `var(--card-scale)`, so this
> function must do the same or the rect silently drifts from the
> rendered card

That reasoning is correct *as a mirror rule* — hover must match what is
rendered. So a CSS-only fix would desynchronize the two. **CSS and
`hover.rs` must change in the same commit**, which is also the standing
rule at `src-tauri/src/hover.rs:101-103` and in plan 063's maintenance
notes.

Two currently-green tests **encode** the wrong behavior and must be
updated, not worked around:

- `src-tauri/src/hover.rs:340-343` `notch_clamp_ceils_wide_cutout_at_scale_1_25`
  asserts a **575px** notch-mode card.
- `src-tauri/src/hover.rs:322-325` `notch_clamp_floors_narrow_cutout_at_scale_1`
  asserts a 200px cutout renders **270px** — 35px overhang at scale 1.0.

### 4. Why this blocks plan 079

Plan 079's most-locked decision (its `:124-140`) is a permanent notch
cutout, *"unconditional across both modes now — no remaining
notch-mode/HUD-mode branch"*. Today the scaled-cutout rule is at least
fenced behind `:root[data-notchtap-mode="notch"]` (`src/styles.css:60`).
Removing that branch **promotes this defect to the only geometry**,
including HUD mode where `--notchtap-cutout-width` is never set
(`src/App.tsx:39` sets it only when non-null) and the 270px fallback
would draw a "notch" unmoored from any hardware. 079 never mentions
`card_scale` anywhere in its 517 lines. Deciding this first is a
prerequisite for that build, not a footnote.

### Bound, and why it is weaker than it looks

`src-tauri/src/settings.rs:221-223` rejects `card_scale` outside
`0.8..=1.4` — but only on the two IPC write paths (`set_appearance`
`:815`, `save_config_and_relaunch` `:737`). **Boot validation
warns and continues by design**: `src-tauri/src/lib.rs:134-142` logs
*"config.toml value out of range — running with it anyway"* rather than
exiting, deliberately, so a bad value cannot brick an always-on login
item (plan 013). A hand-edited `config.toml` therefore renders at any
scale. Whether to tighten that is part of Step 0.

## Current state

- `src/styles.css:13` — `overflow: hidden;` on the card container.
- `src/styles.css:25,39,44,52,61` — the five width rules, quoted in the
  tables above. Verified live at `e09725c`.
- `src/App.tsx:13` — `root.style.setProperty("--card-scale", String(scale));`
- `src/App.tsx:36-42` — mount effect; `:39` guards
  `--notchtap-cutout-width` behind a non-null check.
- `src-tauri/tauri.conf.json:18-19` — `"width": 500`, `"height": 300`.
- `src-tauri/src/hover.rs:23-24` — `WINDOW_WIDTH: f64 = 500.0`,
  `WINDOW_HEIGHT: f64 = 300.0`.
- `src-tauri/src/hover.rs:30-36` — `BASE_WIDTH` 400, `EXPANDED_WIDTH`
  500, `IDLE_WIDTH` 270, `IDLE_STATUS_WIDTH` 460, and
  `NOTCH_CLAMP_MIN`/`NOTCH_CLAMP_MAX` aliasing the 270/460 pair.
- `src-tauri/src/hover.rs:126-132` — the `match` + `* scale`.
- `src-tauri/src/settings.rs:221-223` — the `0.8..=1.4` check.
- `src/settings/preview-overlay.css` — mirrors the card widths but has
  **no** notch-clamp rule (`grep -c notchtap-cutout-width` → `0`). The
  Settings preview therefore cannot show the constraint the scale
  control is about to break. Noted as context; changing the preview is
  **out of scope** here.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass |
| Rust lint | `cd src-tauri && cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Rust fmt | `cd src-tauri && cargo fmt --check` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Frontend lint | `npx biome ci .` | exit 0 |

`cargo` may need `export PATH="$HOME/.cargo/bin:$PATH"`.

## Scope

**In scope**:
- `src/styles.css` (the width rules only)
- `src-tauri/src/hover.rs` (the `Mode::Notch` arm, its doc comment, and
  the two affected tests)
- `src-tauri/tauri.conf.json` (only if Step 0 picks the window-growth
  option)
- `src-tauri/src/settings.rs` / `src-tauri/src/lib.rs` (only if Step 0
  decides to tighten boot validation)

**Out of scope**:
- `src/settings/preview-overlay.css` — the preview's inability to show
  the notch clamp is real but separate; do not fix it here.
- Plan 079's cutout redesign. This plan makes that buildable; it does
  not start it.
- `src/components/` — no component logic changes.
- Plan 063's owed MacBook smoke-check. Still owed, still separate.

## Step 0: OPERATOR DECISION — do not skip, do not guess

Two questions must be answered before any code changes. Present them,
get explicit answers, and record them in this file before proceeding.

**Q1. Should `--card-scale` apply to hardware-derived geometry (the
notch cutout width)?**

- **(a) No — exempt it.** Cutout-derived width is used raw; `--card-scale`
  continues to apply to every other card. *Recommended.* Matches 063's
  locked "zero margin" intent and is the only option that holds at every
  scale. Cost: at "Large" the notch rail visibly does not grow, which a
  user may read as the setting being ignored.
- **(b) Re-clamp after scaling** — `min(scaled, cutout)`. Same visible
  result as (a) at scale > 1.0, but still shrinks below 1.0. More
  moving parts for an identical outcome above 1.0.
- **(c) Keep current behavior, accept the overlap.** Only defensible if
  the overlap is judged cosmetically acceptable — but it directly
  contradicts a P1 whose evidence was an operator screenshot.

**Q2. How should cards be prevented from exceeding the 500px window?**

- **(a) Grow the window** to fit the largest card at max scale
  (`.rail-card.expanded` 500 × 1.4 = 700, so ≥720 with margin). Cleanest
  conceptually — the window becomes big enough for what it must hold.
  Cost: `WINDOW_WIDTH` in `hover.rs:23` and the positioning math must
  move together, and a wider transparent always-on-top window needs a
  click-through sanity check.
- **(b) Cap each card at the window** — add `max-width: 100%` (or
  `min(…, 100%)`) to the width rules. Minimal and safe. Cost: "Large"
  stops actually enlarging the widest cards, so the setting silently
  saturates.
- **(c) Lower the `card_scale` ceiling** so `500 × max ≤ 500`, i.e. max
  1.0. This disables upscaling entirely and effectively removes the
  feature. Listed for completeness; not recommended.

**Q3 (smaller). Should boot validation reject out-of-range
`card_scale` instead of warning?** Current behavior is deliberate
(`lib.rs:134-142`, plan 013 — don't brick a login item). *Recommended:
leave as-is*, and instead make the geometry safe at any value, which
Q1(a) + Q2(a or b) achieves. Changing it is a separate decision about
boot semantics, not about geometry.

**STOP** and report if the operator's answers don't cleanly map to the
options above, or if they want a behavior not listed. Do not synthesize
a fourth option yourself.

**Verify**: the chosen answers to Q1/Q2/Q3 are written into this file
under a "Decision" heading before any file is edited.

## Decision — operator session, 2026-07-21 (Step 0 SATISFIED)

All three questions answered by the operator; this plan is now
dispatchable:

- **Q1 → (a) Exempt the cutout.** Cutout-derived width is used raw —
  `--card-scale` must NOT multiply `--notchtap-cutout-width`. Scale
  continues to apply to every other card width. The accepted cost is
  explicit: at "Large" the notch rail visibly does not grow.
- **Q2 → (b) Cap each card at the window** — `min(<base> * var(--card-scale), 100%)`
  (or `max-width: 100%`) on the five width rules. "Large" saturating on
  the two widest cards is accepted. Window growth was considered and
  deferred — it can be revisited with plan 079 item 19 if the redesign
  wants the room; note 087's finding (window is fully click-through)
  removes the click-risk objection if that day comes.
- **Q3 → leave boot validation as-is** (warn-and-continue, plan 013).
  Q1+Q2 make geometry safe at any value, which is the robust fix.

Also decided in the same session, for the plan-079 item-1 build that
this plan unblocks (recorded here because this file flagged it): the new
card shape is **always opaque `#000`** — `card_opacity` stops affecting
the card shell entirely (stays in config unchanged; Appearance-UI
retirement is a later cleanup).

## Step 1: Implement the Q2 (window overflow) fix

Scale-independent and mode-independent, so do it first and verify it in
isolation. Apply whichever option Step 0 selected, to all five width
rules in `src/styles.css` (`:25`, `:39`, `:44`, `:52`, `:61`).

If Q2(a) — window growth — was chosen, `src-tauri/tauri.conf.json:18`
and `src-tauri/src/hover.rs:23` (`WINDOW_WIDTH`) must change in the
**same commit**; they are a duplicated-constants pair and `hover.rs`'s
own tests will catch a mismatch.

**Verify**: `cd src-tauri && cargo test --locked` passes, and for each
of the four HUD widths, `width × 1.4` is ≤ the window width under the
chosen approach. State the four numbers in your report.

## Step 2: Implement the Q1 (cutout scaling) fix

Change `src/styles.css:61` per the decision. For Q1(a), the
cutout-derived term must sit outside the `* var(--card-scale)`
multiplication — note this **reverses** plan 063's `:506` "do not
reorder" instruction, which is expected and correct; that instruction
was written without the consequence in view.

**Verify**: `grep -n "notchtap-cutout-width" src/styles.css` and confirm
by reading that the cutout term is no longer multiplied by
`--card-scale` (or is re-clamped, per the chosen option).

## Step 3: Update the `hover.rs` mirror, its doc comment, and its tests

1. `src-tauri/src/hover.rs:126-132` — apply the same rule as the CSS.
   For Q1(a) the `Mode::Notch` arm must **not** be multiplied by
   `scale`, while the HUD arms still are.
2. `src-tauri/src/hover.rs:113-117` — the doc comment currently asserts
   that *every* width is scaled and that this function must match.
   Rewrite it to state the new rule and **why** the notch arm is
   exempt (it mirrors a hardware measurement, not a design width).
   Leaving this comment stale would actively mislead the next reader
   into "fixing" the exemption back out.
3. Update the two tests that encode the old behavior:
   - `:340-343` `notch_clamp_ceils_wide_cutout_at_scale_1_25`
   - `:322-325` `notch_clamp_floors_narrow_cutout_at_scale_1`
   Rename them if the new expectation makes the old names wrong. Add
   one test pinning the new invariant directly — e.g. that a notch-mode
   rect at a realistic cutout is identical at scale 1.0 and 1.25.

**Do NOT** delete these tests to make the suite green. If a test cannot
be updated to a meaningful assertion, STOP and report.

**Verify**: `cd src-tauri && cargo test --locked` passes with the
updated tests present; `grep -n "card_scale\|scale" src-tauri/src/hover.rs`
shows the doc comment describing the new rule.

## Step 4: Full gate

Run all six commands from the table. All must exit 0.

**Verify**: paste the six results into your report.

## Test plan

- **Rust**: update the two `hover.rs` tests above; add one test pinning
  scale-invariance of the notch-mode rect. Follow the existing style in
  `src-tauri/src/hover.rs`'s `mod tests` — plain `assert_eq!` on
  `r.x_max - r.x_min`, one behavior per test, descriptive snake_case
  names.
- **Frontend**: no new tests unless Q2's fix changes rendered class
  behavior. The existing suite must stay green.
- **Manual (operator, notch hardware)**: at "Large", confirm the idle
  rail no longer extends past the cutout, and that expanded cards are
  not clipped. This overlaps plan 063's owed smoke-check — run them in
  one sitting.

## Done criteria

- [ ] Step 0's decisions are recorded in this file under a "Decision" heading, with the operator's answers to Q1/Q2/Q3
- [ ] No card width exceeds the overlay window at `card_scale = 1.4`: for each of `src/styles.css:25,39,44,52`, `width × 1.4 ≤ WINDOW_WIDTH` under the chosen approach
- [ ] `src/styles.css:61`'s cutout term is no longer scaled by `--card-scale` (or is re-clamped to the cutout), per Q1
- [ ] `src-tauri/src/hover.rs`'s `Mode::Notch` arm matches the CSS rule exactly, and its doc comment at `:113-117` describes the new rule rather than the old "EVERY width is multiplied" claim
- [ ] Both previously-encoding tests are updated (not deleted), and at least one new test pins scale-invariance of the notch rect
- [ ] `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .` all exit 0
- [ ] No files outside the Scope list are modified (`git status`), plus `plans/README.md`
- [ ] `plans/README.md` status row for 090 updated

## STOP conditions

- **Step 0 is unanswered.** Do not pick a default and proceed — the
  options have materially different user-visible outcomes.
- Any Step-0 answer requires behavior not among the listed options.
- A `hover.rs` test cannot be rewritten into a meaningful assertion
  under the new rule.
- The drift check is non-empty and the live code no longer matches the
  "Current state" excerpts.
- Implementing Q2(a) (window growth) turns out to require changes
  beyond `tauri.conf.json` + `hover.rs`'s `WINDOW_WIDTH` — e.g. the
  positioning math in `src-tauri/src/lib.rs:772-806` needs rework, or
  the wider transparent window intercepts clicks. Report rather than
  improvising; falling back to Q2(b) is a decision, not a workaround.

## Maintenance notes

- **The CSS ↔ `hover.rs` lockstep is manual.** `hover.rs:101-103` and
  its named-constant tripwire pin the Rust constants but cannot see
  CSS. After this plan there will be *two* rules to keep in sync (which
  widths scale, and the window bound) instead of one. Anyone touching
  card widths must read both files.
- **This plan unblocks plan 079's item 1.** Whoever writes that build
  plan should cite this plan's Decision section as the settled answer
  and must not reintroduce a scale multiplication on cutout geometry.
- **`card_opacity` has a parallel problem**, deliberately not fixed
  here: 079's locked card shape requires pure `#000` matching the
  hardware notch, but `src/styles.css:29` ships
  `rgba(5, 6, 7, var(--card-opacity))` at a 0.9 default. That collision
  is 079's to resolve when item 1 is specified; it is noted here only
  so it is not lost.
- **The Settings preview cannot show this constraint**
  (`src/settings/preview-overlay.css` has no notch-clamp rule), so a
  user adjusting scale sees a preview that structurally cannot display
  the thing the setting affects. Worth a future plan; out of scope here.
