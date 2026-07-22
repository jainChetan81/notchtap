# Plan 109: Settings legibility & semantics — gentle type bump, compliant contrast, real HTML

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report. The reviewer
> maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it prints
> anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check**: `git diff --stat 870cdeb..HEAD -- src/settings/`
> — expect plan 108's diff here (it lands first); rebase your reading
> on post-108 master. On mismatch beyond 108's scope, STOP.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED (every settings test that queries emulated ARIA roles
  must migrate with the markup; visual layout shifts at +1–2.5px per
  size)
- **Depends on**: 108 (merged — same file). Land before 111.
- **Category**: accessibility + polish
- **Planned at**: commit `870cdeb`, 2026-07-22

## Why this matters

Verified 2026-07-22: `settings.css` has 29 `font-size` declarations
(cold-read recount; the earlier "28" was a miscount — the per-size
distribution below was always right and sums to 29) distributed
6.5px×2, 7px×5, 8px×7, 8.5px×3, 9px×3, 10px×7, 11px×1,
19px×1 — statuses at 6.5px are below any legibility floor. Dim text
colors are hard-coded down to `#474b53` on `--surface: #0d0f12`
(≈2.19:1, far under WCAG AA 4.5:1); `--text-muted: #70757d` sits at
4.14:1 (just under). Separately, 13 `biome-ignore lint/a11y/*`
suppressions emulate groups/lists/a table with ARIA roles instead of
semantic elements — acknowledged debt ("migrating to `<fieldset>` is a
separate a11y-markup task" — this is that task).

**Operator decision (2026-07-22): gentle bump, not the review's
12–13px.** Body lands at 10–11px, floor at 9px. Window stays 480×600
default (it's resizable; CSS min 400×480 unchanged).

## Current state (verified 2026-07-22 at `870cdeb`)

- `src/settings/settings.css:3-17` tokens: `--surface: #0d0f12`,
  `--window: #050607`, `--text: #f1f2f4`, `--text-secondary: #a2a6ad`,
  `--text-muted: #70757d`.
- Hard-coded dim grays bypassing tokens: `#63676f` (`:515`, `:900`),
  `#646972` (`:397`), `#666b73` (`:587`, `:787`), `#555a62` (`:578`,
  `:637`), `#52565e` (`:173`), `#474b53` (`:203`). **This list is
  ILLUSTRATIVE, not exhaustive** — hexes may appear at more sites
  than cited (cold-read already found two extra); Step B's script
  sweep is the authority, so extra grep hits during your drift check
  are expected, not drift.
- Smallest text: `.shortcut-status` etc. at 6.5px (`:398`, `:788`).
- `src/settings/SettingsApp.tsx` suppressions: `role="group"` ×3
  (`:473` PriorityToggle, `:516`, `:1440` SegmentedControl);
  `role="list"`/`listitem"` (`:557`, `:560` rotation order); a full
  `role="table"`/`row`/`cell` shortcut cheatsheet (`:1203-1216`,
  five suppressions plus `useFocusableInteractive` and
  `noInteractiveElementToNoninteractiveRole`); `noLabelWithoutControl`
  (`:1437`). Suppression comments cite "tests query the ARIA roles"
  and visual-regression risk — both addressed below.
- Settings window: `src-tauri/src/lib.rs:1047` `.inner_size(480.0,
  600.0)`, resizable (Tauri default), CSS min 400×480
  (settings.css:35, :64). **(Amended at review round 2)**: a CSS
  min-width/min-height does NOT establish a native window minimum —
  the user can drag the Tauri window smaller than 400×480 and the
  CSS minimum then forces clipping/scrolling of the whole panel.
  One authorized single-line rust exception (below) closes this.

## Commands you will need

`npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` /
`npx vite build` from the worktree root; `cargo build` from
`src-tauri/` (Step D's one authorized line must compile).
**(Corrected at cold-read — the old sentence here said "STOP if you
find yourself in src-tauri/", which contradicted Step D and would
halt an obedient executor.)** Rust is out of scope EXCEPT the single
authorized `.min_inner_size(400.0, 480.0)` line in Step D (see
Scope); STOP if you find yourself editing any OTHER rust.

## Scope

**In scope**: `src/settings/settings.css`,
`src/settings/SettingsApp.tsx`, settings test files,
`docs/TESTING_STRATEGY.md` §0 (counts, last).

**Out of scope**: `src-tauri/` — with ONE authorized exception
(review round 2): adding `.min_inner_size(400.0, 480.0)` to the
settings `WebviewWindowBuilder` chain at `lib.rs:1041-1048`, making
the native minimum match the CSS minimum. That single line is the
entire permitted rust diff (`git diff master -- src-tauri/` shows
exactly it); the 480×600 default and resizability stay. Everything
else in `src-tauri/` untouched;
`preview-overlay.css` and everything the preview MIRRORS from the
overlay (the preview's own chrome text is in settings.css and IS in
scope; the mirrored card internals are NOT — they must render the
overlay faithfully, not legibly-for-settings); the overlay entry
point.

## Steps

### Step A: type scale
Replace the ad-hoc sizes with a token scale in `:root`:
`--fs-body: 11px; --fs-secondary: 10px; --fs-caption: 9px;
--fs-title: 19px;` and remap every declaration:
6.5→9, 7→9, 8→10, 8.5→10, 9→10, 10→11, 11→11, 19→`--fs-title`.
Judgment calls allowed one step either way where layout demands, but
NOTHING below 9px survives. Convert declarations to the tokens (so
the next pass is a token edit, not 28 greps).

**(Widened at plan review, 2026-07-22)** — the 29 `font-size`
declarations are NOT the whole surface: settings.css also sets sizes
through `font:` SHORTHAND, verified down to 8px — e.g. `:478`
(`font: 620 8.5px/1 …`), `:804` (`font: 700 8px/1 …`), `:846`
(`font: 620 8.5px/1 …`), plus more 9–10px shorthands. Remap these on
the same scale. Shorthand can't take `var()` for the size portion
cleanly with the weight/line-height packed in — either split those
declarations into longhand (`font-family`/`font-size: var(--fs-*)`/
`font-weight`/`line-height`) or keep the shorthand with the literal
remapped px and a `/* --fs-secondary */` comment; prefer longhand
where it doesn't balloon the diff.
**Verify**: `grep -nE "font-size|font:" src/settings/settings.css` →
every size is a `var(--fs-*)` or a literal ≥9px; zero
6.5/7/8/8.5px hits in EITHER form. (The grep also matches size-less
lines — `font: inherit` at `:48` and any `font-family` — these carry
no px size; skip them, they are not violations. Baseline cold-read:
29 `font-size` + 9 sized `font:` shorthands, sub-9px present in both
forms, so this gate genuinely fails before the fix.)

### Step B: contrast tokens
1. Retune tokens to AA on `--surface`: `--text-muted` ≥ 4.5:1
   (e.g. `#7d838c`≈4.6:1 — verify computationally, don't trust this
   plan's arithmetic). Use the exact WCAG formula — it is easy to
   botch the sRGB linearization step:
   `lin(c) = c/12.92 if c ≤ 0.03928 else ((c+0.055)/1.055)^2.4`
   (c = channel/255); `L = 0.2126·lin(R) + 0.7152·lin(G) +
   0.0722·lin(B)`; `contrast = (L₁+0.05)/(L₂+0.05)` with L₁ the
   lighter. Sanity-check the script: `#70757d` on `#0d0f12` must
   compute to ≈4.14 before you trust any other number it prints.
2. **(Widened at plan review, 2026-07-22)** — do NOT work from this
   plan's six-hex list; it was incomplete (`.status-chip`'s `#6d727a`
   at `:610`, ≈3.96:1 on `--surface`, was already missing from it).
   The audit is script-driven: extract EVERY color literal and
   `var()` that lands on a text-rendering `color:` declaration in
   settings.css, compute contrast against both `--surface` and
   `--window`, and tokenize every failure. The six named grays
   (`#474b53 #52565e #555a62 #63676f #646972 #666b73`) plus
   `#6d727a` are confirmed members, not the boundary.
3. Replacements go to `var(--text-muted)` or a new `--text-faint` —
   but `--text-faint` must ALSO be ≥4.5:1 for text; the ≥3:1
   exemption applies only to genuinely decorative or disabled
   elements. Disabled-state text may stay dimmer; mark each such
   site with a comment.
**Verify**: the script (scratch, not committed) runs AFTER the edit
and reports the full table: every text color ≥4.5:1 on both
surfaces except comment-marked disabled/decorative sites (each
listed in the report). Belt-and-suspenders grep for the seven known
hexes → 0, but the SCRIPT SWEEP is the gate — the grep alone proves
nothing about colors the list missed.

### Step C: semantic markup
Replace the emulations, preserving visual style via existing classes:
1. `role="group"` ×3 → `<fieldset>` + `<legend>` (legend can be
   visually styled as the current label; `fieldset` reset:
   `border: 0; padding: 0; margin: 0; min-inline-size: min-content;`).
2. Rotation order `role="list"`/`listitem` → `<ul>`/`<li>`
   (`list-style: none; padding: 0;`).
3. Shortcut cheatsheet `role="table"` cluster → `<table>`/`<tr>`/
   `<td>`/`<th scope="row">` semantic table.
4. `noLabelWithoutControl` (`:1437`) → wire the `<label>` to its
   control (`htmlFor`) or restructure.
5. Migrate the tests that query these roles: `getByRole("group")`
   still matches `<fieldset>`, `getByRole("list")` matches `<ul>`,
   `getByRole("table")` matches `<table>` — most queries should
   survive VERBATIM (that's the point of roles); update only what
   breaks and report which.
6. Delete every now-unneeded `biome-ignore lint/a11y` suppression.
**Verify**: `grep -c "biome-ignore lint/a11y" src/settings/SettingsApp.tsx`
→ 0 (if any legitimately must remain, report each with its reason —
target is 0). `npx biome ci .` clean WITHOUT them.

### Step D: minimum-size fitness check (added at review round 2)
The type bump grows every section; the floor it must survive is the
400×480 minimum, not the 480×600 default:
1. Add the one authorized rust line: `.min_inner_size(400.0, 480.0)`
   in the settings window builder (see Scope).
2. At 400×480 (drive via the CSS min in a browser/jsdom is NOT
   enough — use a real `npm run tauri dev` window resized to minimum
   if GUI available; otherwise vite preview in a 400×480 browser
   viewport and note the Tauri-window check as operator-owed):
   every section must scroll to reach ALL controls (no dead
   clipping), no control paints outside its container, keyboard
   traversal (Tab order) reaches every interactive element with a
   visible focus ring, and the section nav remains operable.
3. Fix what fails (usually: a fixed-height container needing
   `overflow-y: auto`, or a flex row needing wrap).
**Verify**: report the per-section pass/fail table at 400×480; any
jsdom-untestable rows explicitly join the operator smoke list.

### Step E: gates + §0
All frontend gates → clean; `cargo build` (or `cargo test --locked`)
confirms the one-line rust change compiles. Update §0 with
attribution. Take a before/after screenshot pair if running with a
GUI is possible; otherwise note the visual check joins the operator
smoke list.

## Done criteria

- [ ] No text size below 9px in `font-size` OR `font:` shorthand
      form; sizes tokenized (or comment-tagged where shorthand kept)
- [ ] Script sweep of ALL text colors ≥4.5:1 on both surfaces
      (full table pasted into the report; disabled/decorative
      exemptions enumerated); zero raw dim hexes from the known-seven
      list
- [ ] 0 a11y suppressions (or each survivor justified in the report)
- [ ] Role-based test queries still pass against real elements
- [ ] `git diff master -- src-tauri/` → exactly the one
      `.min_inner_size(400.0, 480.0)` line; mirrored card internals
      in preview-overlay.css byte-untouched
- [ ] 400×480 fitness table in the report (scroll reach, no
      clipping, keyboard traversal, nav operable) — failures fixed
      or explicitly operator-owed
- [ ] All gates clean; §0 matches observed counts

## STOP conditions

- A fieldset/table swap breaks layout in a way that needs a
  restructure beyond CSS resets (report with a screenshot/DOM dump).
- Any role-query test cannot be satisfied by the semantic element
  (would indicate a wrong element choice — reconsider before forcing).
- The +1–2.5px reflow makes a section unusable at 400×480 even
  WITH scroll/wrap fixes (report; raising the minimum beyond
  400×480 is an operator decision — the authorized rust line only
  pins the existing CSS minimum natively, it does not license a
  bigger one).

## Maintenance notes

- The review recommended 12–13px body and a 540×680 window; the
  operator chose this gentler scale deliberately. If real-machine use
  still strains, the NEXT bump is a token edit (Step A's point).
- 111's preview/gallery work assumes settings chrome text is
  tokenized — done here.
