# Plan 094: App icon — the notch-cutout glyph (079 items 12+13, decided 2026-07-21)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to
> the next step. If anything in the "STOP conditions" section occurs,
> stop and report. When done, update the status row for this plan in
> `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat <stamp at dispatch>..HEAD -- src-tauri/icons/ src-tauri/tauri.conf.json`
> Expected: empty (nobody touches icons). On a diff, re-read Current
> state before proceeding.

## Status

- **Priority**: P3 — cosmetic, independent of every other plan; can run
  any time, in parallel with anything (zero file overlap with the
  091/092/093 chain).
- **Effort**: S
- **Risk**: LOW — asset-only; no code paths change.
- **Depends on**: none.
- **Category**: direction
- **Planned at**: `<stamp at dispatch>` — a review pass is NOT required
  beyond stamping; nothing here cites volatile code.

## Decisions of record (operator, 2026-07-21)

- **Item 12 — direction**: a minimal geometric mark of the product's own
  identity: a dark rounded square, the notch bite taken out of its top
  edge in pure `#000`, one small accent dot in the overlay's live-green
  (`#7fe08d` — the reserved "something is live" color; using it here is
  branding, not a status claim, and is the operator's call). Must read
  clearly at 16px. Explicit constraint from CLAUDE.md's naming rule: the
  mark must not echo any third-party notch-app's branding.
- **Item 13-remainder — sourcing**: hand-written SVG, committed in-repo,
  rendered to the full icon set via the standard tauri pipeline. No AI
  generation, no stock, no licensing surface.

## Current state

- `src-tauri/icons/` — the default Tauri scaffold set (32x32.png,
  128x128.png, 128x128@2x.png, icon.icns, icon.ico, icon.png, plus
  Square*Logo.png Windows sizes).
- `src-tauri/tauri.conf.json:46-52` — the bundle icon list naming five
  of those files. Keep the list as-is; replace file contents, not paths.
- No SVG source exists anywhere; the scaffold has no vector master.

## Steps

1. **Write the SVG master** at `src-tauri/icons/icon-master.svg` (new,
   committed): 1024×1024 viewBox; dark rounded square (macOS icon-grid
   proportions — content within the ~824px safe area, corner radius
   ~185px to match Big Sur+ squircle feel; a plain rounded rect is
   acceptable, macOS masks the dock rendering anyway); the notch bite
   centered on the top edge in pure `#000` (a rectangle with square
   bottom corners — the same sharp-bite language as the overlay);
   one `#7fe08d` dot, ~56px, placed below the bite. No text, no
   gradients beyond a subtle card-tone fill (`#0b0c10`-family) for the
   square against the black bite.
   **Verify**: the SVG opens (e.g. `qlmanage -p` or any browser) and
   remains legible scaled to 16px.
2. **Generate the icon set**: `npx tauri icon src-tauri/icons/icon-master.svg`
   (tauri v2's icon generator writes the full set into `src-tauri/icons/`).
   If the CLI subcommand is unavailable in this repo's tauri version,
   STOP and report — do not hand-resize PNGs.
   **Verify**: the five files named in `tauri.conf.json:46-52` all have
   fresh mtimes and non-scaffold contents; `git status` shows them
   modified plus the new SVG.
3. **Build check**: `cd src-tauri && cargo build --locked` → exit 0 (the
   bundler config still resolves every icon path).
4. **Gates**: `cargo test --locked`, `npx vitest run` → unchanged counts
   (asset-only change; any count movement is a STOP).

## Scope

**In scope**: `src-tauri/icons/**` (regenerated set + new SVG master),
nothing else.
**Out of scope**: `tauri.conf.json` (paths unchanged), all source code,
all CSS, the overlay, README/docs (the icon needs no documentation
beyond this plan).

## Done criteria

- [ ] `src-tauri/icons/icon-master.svg` exists, hand-written, committed
- [ ] All five `tauri.conf.json`-listed icon files regenerated from it
- [ ] `cargo build --locked` exit 0; test counts unchanged
- [ ] No file outside `src-tauri/icons/` modified
- [ ] `plans/README.md` status row updated

## STOP conditions

- `npx tauri icon` is unavailable or errors — report the tauri CLI
  version rather than hand-producing PNGs.
- Any gate's test count moves (nothing here should touch code).
- The glyph cannot be made legible at 16px without abandoning the
  decided direction — report with a rendering, don't redesign solo.

## Maintenance notes

- The dock-icon eyeball check (does it look right against a real dock,
  light and dark wallpaper?) joins the batched MacBook/mini smoke
  sitting.
- This closes 079 items 12+13 — with 091/092/093 landed, the 079 ledger
  is fully resolved except the item-16 MediaRemote spike filing.
- Future icon tweaks edit the SVG master and re-run the generator —
  never the PNGs directly.
