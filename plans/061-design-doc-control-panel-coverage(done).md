# Plan 061: add the Settings/control-panel window to DESIGN.html

> **Executor instructions**: Documentation-only, no product decision
> needed — unlike its neighbors in this batch (054-060), this is
> ready to execute directly.
>
> **Drift check (run first)**: `grep -n "^<h2>" DESIGN.html` — if a
> "Control panel" or "Settings" section already exists, this plan is
> stale.

## Status

- **Priority**: P3
- **Effort**: S-M
- **Risk**: LOW — docs-only, no source changes
- **Depends on**: none
- **Category**: docs
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from operator
  feedback: "in the document control panel is missing, all components
  should be there for real."

## Why this matters

`DESIGN.html` documents the overlay card exhaustively (color, type,
components, motion, laws) but has zero coverage of the **Settings
window** — the only other UI surface this app has. A design-system
reference that only covers half the app's UI isn't a complete reference;
anyone using it to judge "does a new Settings control match the system"
has nothing to check against.

## Current state

- `src/settings/SettingsApp.tsx` — the real component, 8 sidebar
  sections (`SectionId`, `SettingsApp.tsx:82-90`): General, Football,
  News, Cmux, Weather, Connectors & Keys, Shortcuts, Appearance.
- `src/settings/settings.css` — the real stylesheet, already has a
  properly-built keycap style (`.shortcut-row kbd`, `settings.css:759-773`)
  worth documenting as its own pattern (it's a nicer treatment than
  anything currently in the overlay's own `DESIGN.html` "03 Components"
  section had before plan 054/060's hotkey-styling work landed there).
- `DESIGN.html`'s existing structure (`<h2>01 Color</h2>` through
  `<h2>05 Laws</h2>`) is entirely overlay-scoped — confirmed by reading
  the full file, no `<h2>` section references Settings at all.

## Scope

**In scope**:
- A new numbered section (`<h2>06 Control panel</h2>` or similar,
  renumbering "Laws" to 07 if section order matters) covering: the
  sidebar nav pattern, the rotation-order reorder list (the exact widget
  involved in the plan 045-adjacent config-healing bug fixed this
  session — worth a callout that it's fixed-permutation, not
  add/remove), the shortcuts cheatsheet's kbd styling, the Save/Reset/
  Reset-to-defaults button row, and at least one representative form
  section (e.g. Weather's threshold inputs) to show the input-field
  visual language.
- Live-rendered examples where practical (matching the existing pattern
  of `renderCard`/`renderIdle` JS functions that build real DOM from
  the real CSS classes), or static screenshots if live-rendering the
  Settings React app inside a static HTML file isn't practical — note
  in the plan which approach was used and why.

**Out of scope**:
- Rebuilding Settings' React component tree inside the static HTML file
  — reuse `settings.css`'s classes directly (same approach `DESIGN.html`
  already takes for the overlay: raw HTML/CSS mirroring the real
  classes, not a live React mount).
- Any change to the actual Settings app itself — this is documentation
  only.

## Done criteria

- [ ] `DESIGN.html` has a Control Panel section covering at minimum:
      sidebar nav, rotation-order list, shortcuts kbd styling, form
      inputs, button row
- [ ] Every new example uses `settings.css`'s real class names (no
      invented classes)
- [ ] File still opens cleanly in a browser with no console errors
      (same check used earlier this session: extract `<script>` blocks,
      `node --check` each)
- [ ] `plans/README.md` status row updated

## Maintenance notes

- This should be done in the same pass as plan 054/060 if either lands
  first and changes overlay-side kbd/spacing patterns — keep the new
  Control Panel section's kbd example consistent with whatever the
  overlay section ends up showing, since they're meant to be the same
  visual language documented twice (once per surface).
