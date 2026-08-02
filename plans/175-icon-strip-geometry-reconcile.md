# Plan 175: Reconcile the icon strip's rust hit-test geometry with the shipped CSS

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat 7ca82d5..HEAD -- src-tauri/src/hover.rs src/overlay/card-chrome.css src/overlay/icon-strip.css src/components/StatusRailCard.tsx`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: none (plan 176 edits the same `card-chrome.css:243` rule — whoever lands second reconciles by reading; recommended order is 175 then 176)
- **Category**: bug
- **Planned at**: commit `7ca82d5`, 2026-08-02

## Why this matters

Plan 171 shipped the tab-notch icon strip: while the shell is hovered and
idle, up to five source icons appear in the right flank, and a rust-side
`NSEvent` click monitor hit-tests clicks against per-icon rects computed in
`src-tauri/src/hover.rs`. The rust math and the shipped CSS were paired
against a design mock (`prototypes/tab-notch-rest-and-morph.html`), and
`hover.rs`'s own comment says "re-pair these against the real stylesheet
once it exists". That re-pair never happened. The CSS paints a **flat 85px
flank** with a **16px** right inset; rust computes an **icon-count-driven
flank** (`max(85·scale, 26·n + 14)`) with a **14px** inset. At 3+ present
icons the rust rects drift right of the painted glyphs (≈9px at 3 icons,
≈35px at 4, ≈61px at 5 — the icon pitch is only 26px), so a click on a
visible icon selects the wrong tab or nothing, and because the flank has
`overflow: hidden`, the leftmost glyphs are clipped out of view entirely.
This is the feature's headline gesture, broken exactly in the
"everything live" state that makes the strip worth clicking.

The fix direction is **decided — do not re-decide it**: the CSS adopts
rust's icon-count-driven growth (the mock's design intent; a flat 85px
flank cannot ever fit 4-5 icons), and the inset is unified at the CSS's
shipped **16px** (zero visual change; rust moves 14→16).

## Current state

Files and their roles:

- `src-tauri/src/hover.rs` — pure geometry: `hovered_right_flank_width` and
  `icon_strip_rects` (plus their `#[cfg(test)]` coverage lower in the file).
- `src/overlay/card-chrome.css` — the shell width (`--cw`) formulas and the
  flank rules.
- `src/overlay/icon-strip.css` — the strip's own icon sizing.
- `src/components/StatusRailCard.tsx` — renders the `.card-assembly` shell
  the CSS classes land on (new custom property gets threaded here).
- `src-tauri/src/click.rs` — consumes `icon_strip_rects` (do not change it
  in this plan; it inherits the fix through `hover.rs`).

`src-tauri/src/hover.rs:92-105` (constants above at `:92-94`):

```rust
const ICON_BOX: f64 = 18.0;
const ICON_GAP: f64 = 8.0;
const FLANK_INSET: f64 = 14.0;
...
fn hovered_right_flank_width(present_count: usize, scale: f64) -> f64 {
    let strip_w = (ICON_BOX + ICON_GAP) * present_count as f64 + FLANK_INSET;
    (FLANK_IDLE * scale).max(strip_w)
}
```

`icon_strip_rects` (`hover.rs`, directly below) derives
`total_width = effective_cutout_width + 2.0 * flank_w` and lays icons
right-to-left from `card_x_max - FLANK_INSET`. Note the **symmetric**
`2.0 * flank_w`: the CSS grid is a symmetric `1fr auto 1fr` (flank,
cutout, flank), so both flanks share leftover width equally — the CSS-side
growth must stay symmetric too.

`src/overlay/card-chrome.css:112-114` — the idle shell width (flat 85px
flanks, no icon-count term):

```css
.card-root .card-assembly.idle {
  --cw: min(calc(var(--notchtap-cutout-width, 200px) + (2 * 85px * var(--card-scale))), 100%);
}
```

`src/overlay/card-chrome.css:242-245` — the bare-with-peek variant, same
flat 85px:

```css
.card-root .card-assembly.bare:has(.idle-peek) {
  --cw: min(calc(var(--notchtap-cutout-width, 200px) + (2 * 85px * var(--card-scale))), 100%);
  transition: width var(--reveal-ms, 260ms) var(--ease-notchtap);
}
```

These two are the only `--cw` rules that can match while the strip is
visible (verified at planning time by grepping every `--cw:` declaration in
`src/` — they all live in `card-chrome.css` at lines 41, 113, 138, 193,
243, 457, 509).

`src/overlay/card-chrome.css:333` (and `:274` for the `.bare.hovered`
restore) — the shipped inset:

```css
padding-right: 16px;
```

`src/overlay/icon-strip.css:93-98` — the per-icon footprint (18px box +
8px gap = the 26px pitch rust assumes):

```css
.card-root .icon.is-present {
  width: 18px;
  margin-left: 8px;
  ...
}
```

`src/overlay/card-chrome.css` flank rules give the flanks
`overflow: hidden` and `min-width: 0` (around `:277-283` and `:317-334`),
which is why an oversized strip clips instead of growing the track.

Repo conventions that apply:

- Rust↔CSS constant pairs are "lockstep pairs": each side carries a comment
  naming its twin (see `hover.rs`'s `IDLE_PEEK_BELOW_BLOCK_H` comment and
  `src-tauri/src/agents/expand.rs`'s `HEADER_HEIGHT` ↔ `agent-board.css`
  pairing). Match that idiom.
- `CONTEXT.md` is the glossary; the strip/tab vocabulary ("present",
  "pulled") comes from plan 171 — reuse those words in comments.
- Cosmetic widths scale with `--card-scale`; the cutout width never does
  (comment at `card-chrome.css:108-111`). Rust mirrors this: only the 85px
  rail floor is multiplied by `scale`, the `26n+16` strip term is unscaled.
  Preserve that split on both sides.

## Commands you will need

Run rust commands from `src-tauri/`, web commands from the repo root.

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `PATH="$HOME/.cargo/bin:$PATH" cargo test --locked` | all pass |
| Rust lints | `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| Frontend tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint gate | `npx biome ci .` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src-tauri/src/hover.rs`
- `src/overlay/card-chrome.css`
- `src/overlay/icon-strip.css` (comments/custom-property adoption only)
- `src/components/StatusRailCard.tsx` (threading one custom property)
- One new frontend test file (see Test plan) or additions to an existing one
- `docs/TESTING_STRATEGY.md` §0 (test-count row, if counts change)

**Out of scope** (do NOT touch, even though they look related):
- `src-tauri/src/click.rs` — inherits the fix via `hover.rs`; no edit needed.
- `prototypes/tab-notch-rest-and-morph.html` — a frozen design snapshot,
  never synced (CLAUDE.md).
- Any other `--cw` formula in `card-chrome.css` (lines 41, 138, 193, 457,
  509) — they govern non-strip states.
- The `.hovered` reveal/stagger transitions in `icon-strip.css` — motion
  was reviewed separately; only geometry changes here.

## Git workflow

- Branch: `advisor/175-icon-strip-geometry`
- Commit style: conventional, e.g. `fix(tabs): grow the hovered flank with icon count so clicks land on their glyphs` (match `git log --oneline -10`).
- Do NOT push or open a PR unless the operator instructed it.

## Steps

### Step 1: Unify the inset at 16 in rust

In `src-tauri/src/hover.rs`, change `FLANK_INSET` from `14.0` to `16.0`.
Rewrite the constant block's comment (currently `hover.rs:76-94`): it must
stop citing `prototypes/tab-notch-rest-and-morph.html` as the source of
truth and instead name the shipped lockstep twins:
`src/overlay/icon-strip.css`'s `.icon.is-present` (`width: 18px`,
`margin-left: 8px`) and `src/overlay/card-chrome.css`'s flank
`padding-right: 16px`.

Update every worked example in `hover.rs`'s `#[cfg(test)]` block that pins
`hovered_right_flank_width`/`icon_strip_rects` outputs (locate with
`grep -n "icon_strip_rects\|hovered_right_flank_width" src-tauri/src/hover.rs`)
to the inset-16 values. Add (or extend) a test pinning the flank width for
`present_count` 1 through 5 at `scale = 1.0`: expected
`max(85.0, 26.0 * n + 16.0)` → `85, 85, 94, 120, 146`.

**Verify**: from `src-tauri/`,
`PATH="$HOME/.cargo/bin:$PATH" cargo test --locked hover` → all pass.

### Step 2: Thread the present-icon count onto the shell as a custom property

In `src/components/StatusRailCard.tsx`, compute the present-icon count from
the SAME source `IconStrip` uses for its `is-present` tiers (the
`src/lib/iconPresence.ts` presence derivation — do not write a second
presence predicate), and set it on the `.card-assembly` element as an
inline custom property:

```tsx
style={{ ...existingStyle, ["--present-icons" as string]: presentCount }}
```

The count must match what rust's `tabs::present_tabs` would return for the
same status (weather and news always present; agent/football/music only
when live) — `iconPresenceFor` already mirrors that rule; count its
non-hidden entries.

**Verify**: `npx tsc --noEmit` → exit 0.

### Step 3: Adopt the growth formula in the two CSS width rules

In `src/overlay/card-chrome.css`, change the `--cw` value in exactly the
two rules quoted in "Current state" (`.card-assembly.idle` at ~113 and
`.card-assembly.bare:has(.idle-peek)` at ~243) to:

```css
--cw: min(
  calc(
    var(--notchtap-cutout-width, 200px)
    + 2 * max(85px * var(--card-scale), calc((26 * var(--present-icons, 0) + 16) * 1px))
  ),
  100%
);
```

Keep each rule's other declarations (the `:243` rule's `transition` line)
untouched. Add a lockstep comment on each naming
`hover.rs::hovered_right_flank_width` as the twin, and note the symmetric
`2 *` matches rust's `total_width` and the grid's `1fr auto 1fr` symmetry.
`--present-icons` defaults to `0`, which reduces both rules to today's
exact formula whenever the property is missing.

In `src/overlay/icon-strip.css`, add a one-line comment above
`.icon.is-present` naming `hover.rs`'s `ICON_BOX`/`ICON_GAP` as the
lockstep twins.

**Verify**: `npx vite build` → exit 0, then
`grep -c "present-icons" dist/assets/main-*.css` → at least 2 (the
formulas survived the build un-stripped).

### Step 4: Pin the pair with a frontend test

Add a text-level parity test (new file `src/lib/stripGeometryParity.test.ts`,
modelled on `src/settings/hookEventParity.test.ts` — read that file first;
it reads sources as text with `readFileSync` and compares extracted
regions). Assert:

- `card-chrome.css` contains the `(26 * var(--present-icons, 0) + 16)` term
  in exactly 2 rules;
- `hover.rs` contains `ICON_BOX: f64 = 18.0`, `ICON_GAP: f64 = 8.0`,
  `FLANK_INSET: f64 = 16.0`;
- `icon-strip.css` contains `width: 18px` and `margin-left: 8px` under
  `.icon.is-present`.

This is deliberately a text pin, not a computed one — same trade-off
`hookEventParity.test.ts`'s header comment explains.

**Verify**: `npx vitest run stripGeometryParity` → passes.

### Step 5: Full gates + test-count bookkeeping

Run all five commands from the table. If test counts changed, update the
counts in `docs/TESTING_STRATEGY.md` §0 (counts live there and ONLY there —
CLAUDE.md rule; recount from the live runs, do not add deltas by hand).

**Verify**: all five commands green.

## Test plan

- Rust: extended/updated pins in `hover.rs`'s existing `#[cfg(test)]`
  block — flank width for n=1..5 at scale 1.0, plus the updated
  `icon_strip_rects` worked examples at inset 16.
- Frontend: `src/lib/stripGeometryParity.test.ts` (Step 4), following
  `src/settings/hookEventParity.test.ts` as the structural pattern.
- Existing suites stay green throughout.

## Done criteria

- [ ] `cargo test --locked` (from `src-tauri/`) exits 0
- [ ] `cargo clippy --locked --all-targets -- -D warnings` exits 0
- [ ] `npx vitest run` exits 0, including the new parity test
- [ ] `npx tsc --noEmit` and `npx biome ci .` exit 0
- [ ] `grep -n "FLANK_INSET: f64 = 16.0" src-tauri/src/hover.rs` → 1 match
- [ ] `grep -c "present-icons" src/overlay/card-chrome.css` → ≥ 2
- [ ] `grep -rn "prototypes/tab-notch-rest-and-morph" src-tauri/src/hover.rs` → no match in the constants comment (the stale pairing claim is gone)
- [ ] No files outside the in-scope list are modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back (do not improvise) if:

- The two `--cw` rules quoted in "Current state" are not the ones matching
  the hovered-idle-with-strip state (e.g. a third rule or a `.hovered`
  variant appeared since `7ca82d5`).
- The `.card-assembly` grid template is no longer symmetric `1fr auto 1fr`
  (the symmetric `2 *` growth assumption would be wrong).
- `iconPresenceFor`'s presence rule and `tabs.rs::present_tabs` disagree
  for any tab (that is a separate bug — report it, do not paper over it).
- CSS `max()` inside the `--cw` `calc` fails to build or is stripped by
  the toolchain.

## Maintenance notes

- Any future change to icon size, gap, or inset must touch all three
  lockstep sites (`hover.rs` constants, `icon-strip.css`, the two `--cw`
  rules) — the Step 4 parity test will fail loudly if one is missed.
- The visual result (flank growing on hover as icons accumulate) needs an
  on-hardware feel-check by the operator — same manual-verification class
  as plan 171's outstanding notch checks. Note it in the PR.
- Plan 176 edits the `:243` rule's **selector**; this plan edits its
  **value**. Land 175 first; 176 then reconciles by reading.
