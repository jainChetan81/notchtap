# Plan 105: Static glow dots, weather art behind the media row, and a hoverable bare-notch mode

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report. The reviewer
> maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it prints
> anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check**: `git diff --stat 491587b..HEAD -- src/components src/styles.css src/settings/preview-overlay.css`
> — on content mismatch with the excerpts below, STOP.

## Status

- **Priority**: P2 (Fix C is a real bug — a mode that renders nothing and can't be hovered out of)
- **Effort**: M
- **Risk**: MED (Fix C changes plan 085's shipped behavior and its tests, deliberately)
- **Depends on**: plan 104 (merged)
- **Category**: bug + polish
- **Planned at**: commit `491587b`, 2026-07-22

## Why this matters

Operator feedback after live use of the 104 build:

- **A.** The idle status dots pulse continuously — distracting in
  peripheral vision. They should read as a steady *glow*, not a blink.
- **B.** The media row replaced the weather scene entirely; the
  operator liked the weather background art and wants it kept behind
  the media row.
- **C. (the bug)** "Minimal mode" (`resting_state = "notch"`,
  Settings' bare-notch option) makes the overlay vanish completely AND
  become un-hoverable — the peek can never be revealed, so the mode is
  a dead end until you find Settings again. Expected: it should look
  like the native notch and still reveal the peek on hover. (Cards DO
  still appear in that mode — the gate is `!renderedShowing` — but the
  operator couldn't confirm that because nothing was hoverable.)

## Current state

- `src/styles.css:862-895` — the pulse to remove:
  ```css
  .status-dot.active {
    box-shadow: 0 0 7px 1px currentColor;
    animation: dot-pulse 1.8s ease-in-out infinite;
  }
  ...
  .status-dot.dim { opacity: 0.22; box-shadow: none; animation: none; }
  @keyframes dot-pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.55; } }
  @media (prefers-reduced-motion: reduce) { .status-dot.active { animation: none; } }
  ```
  Mirrored in `src/settings/preview-overlay.css` (grep `dot-pulse`).
  **`.chip-live .live-dot`'s `live-dot-pulse` (styles.css ~:1222) is a
  DIFFERENT dot and must NOT change** — it only appears on live-match
  chips and its pulse encodes liveness (advisor decision).
- `src/components/IdleHoverPeek.tsx`:
  - `:64-80` `WeatherPeekScene` — one element carrying BOTH the art
    (`wx-card` + `art.moodClass` + `art.textureClass` on
    `.wx-peek-scene`) and the readout (`.wx-peek-readout`: temp +
    condition chip).
  - `:239-258` the render body:
    ```tsx
    const live = status?.football.live ?? null;
    const media = status?.media.current ?? null;
    const weather = status?.weather.current ?? null;
    return (
      <div className={`below-block idle-peek${closing ? " closing" : " open"}`}>
        {live !== null ? <ScorecardRevealContent live={live} />
          : media !== null ? <MediaPeekRow media={media} />
          : weather !== null ? <WeatherPeekScene weather={weather} /> : null}
        <PeekTimeline />
      </div>
    );
    ```
  - `styles.css:1399-1406` `.wx-peek-scene` is `position: relative;
    min-height: 78px; border-radius: 10px; overflow: hidden; display:
    flex; align-items: flex-end;`; `.wx-peek-readout` is `position:
    relative; z-index: 1`. The mood/texture classes paint via
    background layers on `.wx-card`.
- `src/components/StatusRailCard.tsx:278-289` — Fix C's target:
  ```tsx
  // plan 085: idle + resting_state "notch" → zero app-drawn pixels, not a
  // narrower/emptied shell ...
  if (!renderedShowing && !exiting && restingState === "notch") {
    return null;
  }
  ```
  The component owns the whole `.card-assembly` INCLUDING
  `<IdleHoverPeek>`, so this early return also removes the only path
  by which hovering can reveal anything.
- `styles.css:147-155` — `.synthetic-cutout` is `display: none` except
  under `:root[data-notchtap-mode="hud"]`, where it paints `#000`.
  That is what will give the mac mini its "looks like a real notch"
  appearance in bare mode.
- Rust hover detection is independent of `resting_state`
  (`grep -n resting src-tauri/src/hover.rs` → no hits), so hover events
  already fire in bare mode — **no rust change is needed or wanted**.

## Commands you will need

`npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` /
`npx vite build` from the worktree root. Rust is untouched by this plan
(if you find yourself editing `src-tauri/`, that's a STOP).

## Scope

**In scope**: `src/styles.css`, `src/settings/preview-overlay.css`
(mirror law), `src/components/IdleHoverPeek.tsx`,
`src/components/StatusRailCard.tsx`, their `.test.tsx` files,
`docs/TESTING_STRATEGY.md` §0 (counts, last).

**Out of scope**: all of `src-tauri/` (including hover.rs);
`.chip-live .live-dot`; the football scorecard's internals; every
showing-card path (`renderedShowing === true` behavior must be
byte-identical).

## Steps

### Step A: static glow dots
In both CSS files: delete the `animation:` line from
`.status-dot.active`, strengthen the resting glow slightly
(`box-shadow: 0 0 8px 1px currentColor;`), delete the now-unused
`@keyframes dot-pulse` and the reduced-motion block that only silenced
it (if that block contains other selectors, remove only the
`.status-dot.active` rule from it). Keep `.status-dot.dim` as is
(its `animation: none` becomes redundant but harmless — leave it).
**Verify**: `grep -rn "dot-pulse" src/` → no hits.
`grep -rn "live-dot-pulse" src/styles.css` → still present (untouched).

### Step B: weather art as a backdrop layer
In `IdleHoverPeek.tsx`, split `WeatherPeekScene` into two pieces:
- `WeatherPeekBackdrop({ weather })` — renders ONLY the art element:
  `<div className={["wx-peek-backdrop","wx-card",art.moodClass,art.textureClass].filter(Boolean).join(" ")} aria-hidden="true" />`
  (plus the `<img className="wx-icon" .../>` if the glyph belongs to
  the art rather than the readout — decide by reading which element
  currently positions it, and keep the glyph with the art).
- `WeatherPeekReadout({ weather })` — the existing `.wx-peek-readout`
  (temp + `.chip` condition), unchanged markup.

New render body:
```tsx
const showBackdrop = weather !== null && live === null; // scorecard keeps its own visual
return (
  <div className={`below-block idle-peek${closing ? " closing" : " open"}`}>
    {showBackdrop ? <WeatherPeekBackdrop weather={weather} /> : null}
    <div className="peek-content">
      {live !== null ? <ScorecardRevealContent live={live} />
        : media !== null ? <MediaPeekRow media={media} />
        : weather !== null ? <WeatherPeekReadout weather={weather} /> : null}
      <PeekTimeline />
    </div>
  </div>
);
```
CSS (both files): `.wx-peek-backdrop { position: absolute; inset: 0;
z-index: 0; border-radius: 10px; overflow: hidden; }` and
`.peek-content { position: relative; z-index: 1; }`. Port whatever
`.wx-peek-scene` contributed (min-height, art layering) so the
weather-only case looks the same as before; keep `.wx-peek-scene` only
if something still uses it, else delete it. If the art is light enough
to hurt media-row legibility, add a scrim
(`.wx-peek-backdrop::after { content:""; position:absolute; inset:0;
background: rgba(0,0,0,0.35); }`) — the same technique
`.news-shade` already uses; say in your report whether you added it.
**Verify**: `npx vitest run` — update/extend `IdleHoverPeek.test.tsx`:
backdrop present with media+weather, backdrop ABSENT with a live match,
weather-only case still renders temp + condition chip, media row still
outranks the weather readout.

### Step C: bare notch mode that is still hoverable
Replace the `return null` in `StatusRailCard.tsx` with a bare render.
Behavior contract:
- Not showing, `restingState === "notch"`, **not hovered** → the
  assembly mounts but paints NOTHING app-drawn: no below-block, no
  clock/dots content in the flanks, flanks transparent, no shadow, no
  priority accent. In HUD mode the `.synthetic-cutout` still paints
  `#000` (that IS the "looks like a native notch" appearance); in notch
  mode nothing paints over the hardware notch.
- Same state, **hovered** → `<IdleHoverPeek>` mounts and reveals
  exactly as it does in rail mode (that is the whole point of the fix).
- Any showing/exiting state → unchanged from today.

Implementation: add a `bare` modifier class on `.card-assembly` for
this state, keep the width at the cutout width alone while not hovered
(reuse the `.idle` width formula only when the peek is open, so the
shell can't be a wide invisible blocker), and in both CSS files:
```css
.card-assembly.bare .flank-left,
.card-assembly.bare .flank-right { background: transparent; box-shadow: none; }
.card-assembly.bare { --cw: min(var(--notchtap-cutout-width, 200px), 100%); }
.card-assembly.bare:has(.idle-peek) { /* peek open: normal idle width */ }
```
(exact selectors are yours; the contract above is what the tests pin).
Do NOT render the clock or status dots in bare mode.
**Verify** — `StatusRailCard.test.tsx`, replacing the plan-085
null-return test (this is an authorized behavior change; keep a
regression test for what 085 actually protects: *no painted card
chrome*):
- bare + not hovered → no `.below-block`, no clock text, no
  `.status-dots` in the DOM
- bare + hovered → an `.idle-peek` below-block IS present
- bare + showing → identical DOM to rail + showing (assert the card
  content renders as usual)
- rail mode unaffected (existing tests stay green)

### Step D: gates + §0
`npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`,
`npx vite build` → clean. Update `docs/TESTING_STRATEGY.md` §0 to the
observed frontend count (rust unchanged at 481+3) with attribution.

## Done criteria

- [ ] `grep -rn "dot-pulse" src/` → no hits; `live-dot-pulse` still present
- [ ] Backdrop tests from Step B pass; weather-only visual path preserved
- [ ] All four Step C behavior tests pass; the old null-return test is
      replaced (not merely deleted) by the no-painted-chrome test
- [ ] `git diff master -- src-tauri/` → empty
- [ ] All gates clean; §0 matches observed counts; only in-scope files modified

## STOP conditions

- Making bare mode hoverable would require a rust/hover.rs change
  (it should not — hover is `resting_state`-agnostic).
- The weather art's layering can't be split without changing how
  `weatherArtFor`'s mood/texture classes paint (report what you found).
- Any showing-card DOM change is unavoidable.
- Content mismatch against the "Current state" excerpts.

## Maintenance notes

- Fix C narrows plan 085's original "zero app-drawn pixels" promise to
  "zero app-drawn pixels *until hovered*" — that is the intended new
  contract; 085's file in `plans/done/` is historical and stays as is.
- The `.chip-live .live-dot` pulse was deliberately kept (liveness
  signal, rare, only on live-match chips). If the operator later wants
  it static too, it is a one-line change in the same place.
- If the operator ever asks for the bare notch to also hide during
  *showing* (true do-not-disturb), that is a different feature — the
  pause hotkey (⌃⇧P) already covers the "stop rotating" half.
