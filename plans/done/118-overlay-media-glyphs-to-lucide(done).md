# Plan 118: Overlay media glyphs → lucide icons (UI polish)

> **Executor instructions**: Follow step by step; run every verification. STOP on
> any STOP condition. Do NOT push/merge/PR. Do NOT edit `plans/README.md`.
>
> **Drift check (run first)**: `git diff --stat f439f20..HEAD -- src/components/IdleHoverPeek.tsx`

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx / ui-polish
- **Planned at**: commit `f439f20`, 2026-07-22

## Why this matters

The overlay's idle-hover media row uses bare emoji/text glyphs (`♪ 📺 🌐 ▶ ⏸`)
that render at font metrics and look inconsistent next to the rest of the UI. The
operator wants these as crisp lucide icons — a deliberate UI improvement they
value. This is UI-only: swap the glyphs for the equivalent lucide components,
sized/colored to match, no behavior change. **Accepted tradeoff**: this
introduces `lucide-react` into the overlay bundle (settings already uses it; the
overlay had none). The operator has decided the visual gain is worth it. Scope is
deliberately narrow — only the media affordance glyphs, NOT wire-driven content.

## Current state

`src/components/IdleHoverPeek.tsx`:
- `glyphForBundleId(bundleId)` (≈line 100–116) returns emoji STRINGS: `"♪"` (music),
  `"📺"` (tv), `"🌐"` (browser: safari/zen/chrome/firefox), `"▶"` (default/null).
  Rendered at ≈line 161 inside `<span className="media-art" aria-hidden="true">`.
- Media play-state indicator (≈line 174): `<span className="media-state" aria-hidden="true">{media.playing ? "▶" : "⏸"}</span>`
  — a STATE INDICATOR (not a control): playing shows `▶`, paused shows `⏸`. Preserve that exact mapping.

Tests: `src/components/IdleHoverPeek.test.tsx:231-250` — 8 assertions on
`glyphForBundleId` returning the specific emoji strings (`.toBe("♪")` etc.).

lucide is already a dependency (`lucide-react ^1.24`), used in `src/settings/SettingsApp.tsx`.
lucide exports `Music`, `Tv`, `Globe`, `Play`, `Pause`.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Install | `npm ci` | exit 0 |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Tests | `npx vitest run` | 305 pass |
| Lint | `npx biome ci .` | exit 0 |
| Build | `npx vite build` | exit 0 |

## Scope

**In scope**: `src/components/IdleHoverPeek.tsx`, `src/components/IdleHoverPeek.test.tsx` ONLY.

**Out of scope — leave untouched**:
- The weather condition text (`weather.condition`, `.wx-peek-condition`) — wire content.
- Football scorecard event marks, celebration art — wire content / CSS art.
- `StatusDots.tsx` pause glyph (CSS-drawn bars tuned to the 9×9 dot footprint) — not an emoji; converting risks shifting the status row for no emoji-cleanup gain.
- Keyboard keycap glyphs (`⌃⇧…`) anywhere — content.
- The cmux `⌁` chip prefix — decorative CSS pseudo-element.
- `src-tauri/**`, `src/overlay-card.css` (no CSS changes needed unless sizing demands one small rule — see Step 2).

## Git workflow

- Worktree from stale base: **FIRST** `git reset --hard f439f20`, verify Step 0.
- One or two commits, conventional-commit style (e.g. `feat(overlay): media glyphs to lucide icons (plan 118)`).

## Steps

### Step 0: Base-sync + baseline
`git reset --hard f439f20`; confirm the glyph sites above with grep; baseline gates green (`npm ci`, vitest 305, tsc 0, biome 0, vite build 0). Mismatch → STOP.

### Step 1: Convert `glyphForBundleId` to return a lucide icon
Refactor `glyphForBundleId` → rename to `iconForBundleId(bundleId)` returning a
lucide component (the `LucideIcon` type): `Music` (music), `Tv` (tv), `Globe`
(browser list unchanged), `Play` (default/null). Keep the same match order and
the same branch conditions — only the RETURN changes from string to component.
Import `Music, Tv, Globe, Play` from `lucide-react`.
Update the render site (`.media-art`): render the returned component, e.g.
`const MediaIcon = iconForBundleId(media.appBundleId);` then
`<MediaIcon className="media-art-icon" aria-hidden="true" />` (keep the
`aria-hidden`; the accessible name still comes from `.media-title`). If you keep
the wrapping `<span className="media-art">`, render the icon inside it.
Update the 8 tests in `IdleHoverPeek.test.tsx:231-250` to assert the returned
COMPONENT (`expect(iconForBundleId("com.apple.Music")).toBe(Music)` etc.,
importing the icons) — this is an allowed, required focused test edit for the
changed contract. Keep all 8 cases (music / tv / 4 browsers / 2 defaults).

**Verify**: `grep -cE '[♪📺🌐]' src/components/IdleHoverPeek.tsx` → 0; tsc 0; the 8 (updated) glyph tests pass.

### Step 2: Convert the media play-state indicator
Replace `{media.playing ? "▶" : "⏸"}` in `.media-state` with
`{media.playing ? <Play className="media-state-icon" aria-hidden="true" /> : <Pause className="media-state-icon" aria-hidden="true" />}`
— preserving the exact semantic (playing→Play, paused→Pause). Import `Play, Pause`
(Play already imported from Step 1). Keep the `.media-state` span (now `aria-hidden`
wrapper) or move `aria-hidden` onto the icon.

**Sizing/color**: lucide icons default to 24px and `currentColor`. The glyphs
currently render at the CSS font-size of `.media-art` / `.media-state`. Size the
icons to MATCH that visual footprint so the row doesn't shift — set an explicit
`size` prop or a `width/height` via a small className. Read the current
`.media-art` / `.media-state` font-size in `overlay-card.css` and match it (e.g.
`size={<that px>}`). Color inherits via `currentColor` — no color change needed.
If a tiny CSS rule for `.media-art-icon`/`.media-state-icon` sizing is cleaner
than inline props, adding it to `overlay-card.css` is permitted (that one small
addition is the only allowed overlay-card.css change).

**Verify**: `grep -cE '[▶⏸]' src/components/IdleHoverPeek.tsx` → 0; build clean; the media-row tests still pass (they assert on title/subtitle/outranking, not the glyph).

### Step 3: Final gates
`npx vitest run` (305, with the 8 glyph tests updated — count unchanged), `npx tsc --noEmit` (0), `npx biome ci .` (0), `npx vite build` (0). `git diff --name-only f439f20..HEAD` → only `IdleHoverPeek.tsx`, `IdleHoverPeek.test.tsx` (and optionally `overlay-card.css` if you added the one sizing rule).

## Test plan

- The 8 `glyphForBundleId`/`iconForBundleId` tests: updated to assert the lucide
  component per case (required — the contract changed). Count stays 8.
- Existing media-row render tests (media outranks weather, paused renders, null →
  no media row, `iconForBundleId`'s 4 cases via the render) stay green — they key
  off `.media-title`/structure, not the glyph character. If any asserted on the
  literal `▶`/`⏸`/emoji, repoint it to assert the icon component/SVG presence.
- No new product behavior → no new test files needed.

## Done criteria

- [ ] `iconForBundleId` returns lucide components (`Music`/`Tv`/`Globe`/`Play`); no `♪`/`📺`/`🌐` emoji remain in the file
- [ ] media-state indicator uses `Play`/`Pause` (playing→Play, paused→Pause preserved); no `▶`/`⏸` remain
- [ ] icons sized to match the prior glyph footprint (no row layout shift)
- [ ] all `aria-hidden`/accessible-name semantics preserved (title still names the row)
- [ ] 8 glyph tests updated + passing; all other IdleHoverPeek tests green; vitest 305 total
- [ ] tsc 0 / biome 0 / vite build 0; scope diff limited to the 2 (or 3) in-scope files

## STOP conditions

- Base-sync/Step-0 evidence doesn't match.
- Sizing the icons to match the glyph footprint proves impossible without shifting the media row — report for a sizing decision rather than shipping a layout shift.
- A gate fails twice after a reasonable fix.
- Any out-of-scope file (weather/football/StatusDots/keycaps) would need to change.

## Operator-owned acceptance (PENDING)

WKWebView screenshot of the idle-hover media row: confirm the `Music`/`Tv`/`Globe`/`Play`
source icon and the `Play`/`Pause` state icon render crisply, aligned, at the
right size, matching the app's icon language. Report PENDING; never fake.

## Maintenance notes

- This introduces `lucide-react` into the overlay bundle (was settings-only).
  Future overlay icon needs can now reuse the same set. Keep it to genuine
  affordances — wire-driven content (weather/football emoji) stays as content.
- Reviewer: confirm the play/pause SEMANTIC wasn't flipped, icon sizing matches
  the old glyph footprint, and no wire-content glyph was swept up.
