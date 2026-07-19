# Plan 063: SPIKE — idle status rail (460px) overlaps other apps' menu bar icons in notch mode

> **Executor instructions**: Decision spike. Do not ship a guessed width
> without operator sign-off — the whole point of this plan is that no
> width is safe for every notch size / menu-bar icon count combination.

## Status

- **Priority**: P1 — this actively obscures other apps' functional menu
  bar icons on real hardware, not a cosmetic nit.
- **Effort**: S (decision) / S (build, once decided)
- **Risk**: MED — same notch-mode-needs-hardware-verification caution as
  plan 060, but here the current behavior is already confirmed broken
  (operator screenshot on the MacBook), not just theoretically risky.
- **Depends on**: none (sibling to plan 060 — that one covers the
  notchless/HUD-mode top-spacing collision; this one is the
  notch-mode-specific menu-bar-icon overlap, a different failure mode of
  the same underlying "one CSS width for every screen" gap)
- **Category**: bug / direction
- **Planned at**: commit `40b4b0e`, 2026-07-19 — filed from an operator
  screenshot on the MacBook (real notch hardware): the idle rail's chip
  row (Football/News Paused/Weather off/Clear) extends far enough right
  that it visibly sits on top of — and the trailing "Clear" chip gets
  clipped by — the real macOS menu bar's own icon tray (several
  third-party app icons visible, partially obscured).

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
  Focus/DND rejection already recorded in this README).
- The window itself is fixed at 500x300 (`tauri.conf.json:18-19`,
  `resizable: false`); card width is a CSS-only concern layered inside
  that fixed canvas via `margin: 0 auto`.

## Decision needed (operator)

1. **Cap the rail width in notch mode specifically**, using the real
   `CutoutGeometry.width` (threaded to the frontend — new plumbing, see
   below) to compute a safe max width (e.g. notch width + some fixed
   margin on each side), falling back to today's 460px only in HUD mode
   where there's no notch to respect. This preserves chip richness on
   wider notches (newer MacBook Pro models have wider notches than
   older MacBook Air models) instead of a one-size-fits-none constant.
2. **Never widen past a conservative fixed cap in notch mode**
   (e.g. 300-320px, enough for 2 chips not 4), accepting that some idle
   chips may need to scroll/cycle or simply not all show at once on
   narrow-notch machines. Simpler to build, doesn't need the
   cutout-width plumbing, but caps chip richness on every notch
   Mac regardless of actual clearance.
3. **Don't widen the idle rail in notch mode at all** — keep it at the
   270px plain-clock width always in notch mode, and only show the
   richer status chips in HUD mode (where there's no notch to respect,
   so 460px is genuinely safe) or when a card is actually promoted
   (compact/expanded already don't have this problem — they're
   positioned by the same notch-anchor logic but the operator hasn't
   reported an overlap complaint about those, only the idle rail).

## Recommendation

Option 1 is the most correct long-term fix (uses the information the
app already has, degrades gracefully across different MacBook notch
widths) but requires new rust→frontend plumbing (surface
`CutoutGeometry.width` — or a computed safe-max-width — through the
existing one-shot boot-time eval-splice pattern already used for other
static-at-boot facts). Option 2 is a same-day mitigation if the operator
wants relief before the real fix lands: pick one conservative width,
ship it, revisit properly later. Option 3 is the safest but most
visually regressive (loses the idle-rail feature's whole value in the
one mode — notch — where it's most likely to be seen).

**If the operator wants an immediate stopgap while this plan is
pending**: the single lowest-risk change is narrowing
`.rail-card.idle.status`'s width from 460px to something conservative
(e.g. 320px) universally (both modes) — small, immediate, testable on
this Mac mini in HUD mode, but still needs the MacBook check before
calling it actually safe. Not applied here; this plan intentionally
stops at "decision needed," matching this session's operator-set
scoping instruction to hold off on anything needing hardware
verification only the operator can do.

## Maintenance notes

- Coordinates directly with plan 060 — both are instances of "the
  frontend has zero awareness of notch-vs-HUD mode or actual screen
  geometry beyond a fixed card width." If plan 060's mode-aware
  plumbing gets built, this plan should reuse the same mechanism rather
  than inventing a second one-shot boot fact.
- Whoever builds this needs a real MacBook smoke-check with a realistic
  menu-bar icon load (not a nearly-empty menu bar) before calling it
  done — same discipline plan 060 and the hardware-only checks in
  plans 010/012/018/023/032/033/034 already require.
