# Plan 093: The hover consumers — idle weather peek (+timeline), TTL hover-pause, scorecard reveal (079 items 9/17/18 hover halves + 081/084 deferred halves)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **HARD PREREQUISITES — 091 AND 092 must be DONE and merged.** The idle
> peek grows out of 091's assembly; the reveal/pause layer onto surfaces
> 092 styles. If `grep -c "below-block" src/styles.css` is 0 → STOP (091
> missing); if `grep -c "notif-track" src/styles.css` is 0 → STOP (092
> missing).
>
> **FULLY REVIEWED AND DISPATCHABLE.** The post-091 rust/hover pass ran
> 2026-07-21, and the owed 092-dependent re-check completed after 092
> merged (both notes at the end of this file). Prerequisite gates both
> confirmed live at `147a996`: `grep -c "below-block" src/styles.css` > 0
> (091 shipped) and `grep -c "notif-track\|notif-body" src/styles.css`
> → 4 (092 shipped).
>
> **Drift check (run first, after stamping)**:
> `git diff --stat 147a996..HEAD -- src-tauri/src/hover.rs src-tauri/src/lib.rs src-tauri/src/queue.rs src-tauri/src/engine.rs src/App.tsx src/components/ src/styles.css src/settings/preview-overlay.css src/lib/weatherArt.ts`
> Expected: empty. On a mismatch with "Current state", STOP.

## Status

- **Priority**: P2
- **Effort**: M — three features, but the hard part (the primitive)
  shipped in 087 and the `hovered` plumbing already reaches the
  component.
- **Risk**: MED — the TTL lifecycle pause touches queue timing (the
  repo's most-tested invariant surface); the y-span fix touches the
  CSS↔hover.rs lockstep.
- **Depends on**: **091 + 092 (hard)**; 087 (primitive, DONE); 082
  (weatherArt moods, DONE); 084 (scorecard, DONE); item 18 decision
  (timeline folds into the hover-expanded idle).
- **Category**: direction
- **Planned at**: commit `147a996`, 2026-07-21 (stamped post-091 AND post-092 merges; all citations re-verified)

## Why this matters

Plan 087 shipped the hover primitive — tracking area, rust-derived rect,
a `hover-changed` event already flowing into `StatusRailCard`'s `hovered`
prop (`src/App.tsx:86-97`, `StatusRailCard.tsx:92`) — with exactly one
diagnostic consumer. Three real features have been explicitly deferred
onto it across four plans (081's hover-pause half, 082's peek prep,
084's reveal gate, 079 item 17's expanded-on-hover): this plan builds all
three at once so the hover semantics land as one coherent interaction
model instead of three ad-hoc patches.

**The primitive's carried-forward limitation is this plan's first job**:
087's rect is x-constrained only — its vertical span is the full 300px
window, so ~240px of empty space below the card registers as hovered
(`docs/design/hover-cursor-tracking.md`; 079 item 17's bracketed note).
Every consumer below depends on fixing that first.

## The three features

1. **Idle weather peek = the hover-expanded idle state (items 9+17+18
   in one surface)**: hovering the idle assembly grows a `.wx-peek`
   below-block — the flat weather-mood scene (082's `weatherArt.ts`
   moods + temp/condition overlay) plus the relocated day-progress
   timeline (item 18's decided home). Mechanics per the prototype
   (`prototype/notch-states.html:112-124`): height transition (NOT
   max-height — the prototype documents why), flanks un-round while the
   peek is open (the double-curve rule), 260ms/200ms curves as mocked.
   Driven by the `hovered` prop, NOT CSS `:hover` — the overlay window
   is click-through; only the tracking area knows the cursor.
2. **TTL hover-pause (081's deferred half)**: while a showing card is
   hovered, the TTL bar freezes AND the rotation deadline holds. Hover
   state originates rust-side (the tracking-area handler), so the hold
   belongs in rust: on hover-enter over a showing card, the Engine
   extends/parks the rotation deadline (same wake-protocol discipline as
   every deadline mutation — plan 036/037 rules); on hover-exit, the
   deadline re-anchors with the remaining time it had at entry. The
   frontend freezes the bar via a `hoverPaused` flag alongside the
   existing `remainingMs` re-anchor machinery (`TtlBar.tsx:10-14`).
   **Design constraint**: a card must never rotate out while under the
   cursor, and must never gain MORE total time than `ttl + extension`
   rules allow from repeated hover cycles — pin both in tests.
3. **Scorecard reveal (084's deferred gate)**: with a live match active
   and the overlay idle, hovering reveals the compact scorecard (the
   recurring live card) as a below-block for the hover duration; on exit
   it collapses back to time+dots. Priority/rotation semantics are
   untouched — this is a render-layer reveal of already-present ambient
   state (`useStatusState`'s live summary), not a queue promotion. When
   a live match exists, the weather peek yields to the scorecard reveal
   (one below-block at a time; football outranks ambient weather — same
   precedence 084 established for the paused-football fallback).

Plus the shared groundwork: **y-span constraint** — `active_card_rect`
gains a height derived from the actual assembly state (cutout height for
idle; cutout height + measured/constant below-block height for showing),
using the already-tested `css_top_down_to_appkit_y` flip. Hover must
stop firing over empty window space below the card.

## Current state — rust/hover re-verified against shipped 091 at `0ea2a96`

- `src/App.tsx:86-97` — `hover-changed` listener → `hovered` state →
  prop. Already shipped, tested (087: `.hovered` class toggles).
- **`src-tauri/src/hover.rs::active_card_rect` (post-091 form — this is
  what you extend)**:
  - Signature `(mode, cutout_width, scale, visible, expanded,
    _has_status_chips) -> Rect`. The last parameter is **deliberately
    unused** (underscore-prefixed) — 091 kept it only so the sole call
    site, `lib.rs`'s `hover_point_is_over_card`, needed no edit. If this
    plan gives it a real use again, un-prefix it; otherwise leave it.
  - Width no longer branches on `mode` at all; `mode` only selects
    `effective_cutout_width` (measured vs `HUD_CUTOUT_W`). Constants at
    `:37-50`: `FLANK_IDLE 85.0`, `MIN_FLANK_SHOWING 60.0`,
    `BASE_SHOWING 400.0`, `BASE_EXPANDED 500.0`, `HUD_CUTOUT_W 200.0`,
    `HUD_CUTOUT_H 32.0`; window `500.0 × 300.0` at `:23-24`. The cutout
    term is never multiplied by `scale` (plan 090) — preserve that.
  - **The y-span this plan must fix is at `hover.rs:201`**:
    `let (y_min, y_max) = css_top_down_to_appkit_y(WINDOW_HEIGHT, 0.0, WINDOW_HEIGHT);`
    — the full 300px window, with 091's own comment at `:147-148`
    acknowledging it. Replace the `0.0, WINDOW_HEIGHT` span with the
    real assembly height: `HUD_CUTOUT_H`-or-measured-cutout-height for
    idle, plus the below-block's height when showing. `css_top_down_to_appkit_y`
    (`:84`) is already correct and tested (`:545-568`) — reuse it, do
    not reimplement the flip.
  - Two existing tests pin the full-window span (`:566-577`,
    asserting `y_min == 0.0` / `y_max == WINDOW_HEIGHT`) — they encode
    the limitation, so UPDATE them, never delete them.
- `src-tauri/src/queue.rs` — deadline machinery: `promoted_at`
  (`:10`, `Option<Instant>`, set at `:176`) and `extension_secs`
  (`:11`); the auto-retract read at `:247-253` is the closest existing
  example of reasoning from `promoted_at`. `src-tauri/src/engine.rs` —
  the mutate→wake→emit protocol every deadline change must follow.
- `src/components/TtlBar.tsx:18-40` — props `{ slotId, ttlMs,
  remainingMs }`; re-anchors in a `useEffect` keyed on all three
  (the biome-ignore there explains why `slotId` is a deliberate
  re-anchor trigger). A `hoverPaused` flag joins these props; note the
  rAF loop is already gated under `prefers-reduced-motion`.
- **092's shipped content language (verified live at `147a996`)**, which
  the peek and reveal render beside inside `.below-block`:
  `.notif-header-row` (`styles.css:518`), `.notif-title` (:525),
  `.notif-subtitle-row` (:537), `.notif-subtitle` (:545),
  `.notif-time-inline` (:558), `.notif-body` (:564), `.notif-meta-row`
  (:952), and the `.chip` family (:941, `.chip-category` :959). **The
  weather peek should reuse `.chip` for its condition label rather than
  inventing a pill** — 092 retired `.pill` entirely (`grep -c "\.pill\b"`
  → 0); reintroducing one would undo item 10.
- `src/lib/weatherArt.ts` — `weatherArtFor(condition, isDay)` (`:120`)
  returning `WeatherArt` (`:40`). The peek's mood/glyph come from here;
  do not duplicate the table.
- **Note the pause indicator now exists** (092, Decision 4): `StatusDots`
  dims every dot while `status.paused` and renders a `.pause-glyph`. The
  hover peek must not fight it — a paused engine still shows the idle
  furniture, so hovering while paused should still peek.
- `src/lib/weatherArt.ts` — mood/glyph tables (082), built to be reused
  by this peek ("preps but does not build the 086-gated hover
  weather-peek" — its own plan text).
- `useStatusState` — live-match + weather ambient summaries (the reveal
  and peek render from these, not from the queue).
- `prototype/notch-states.html:112-124` — the locked peek mechanics.

## Scope (headline — expand at the review-plan pass)

**In scope**: `src-tauri/src/hover.rs` (y-span + any state-dependent rect
height), `src-tauri/src/lib.rs` (hover→Engine notification path if the
TTL hold needs it), `src-tauri/src/engine.rs` + `src-tauri/src/queue.rs`
(the hover-hold deadline mechanics + tests), `src/components/`
(peek/reveal/pause rendering), `src/styles.css` +
`src/settings/preview-overlay.css` (peek/reveal rules, mirror law),
`src/lib/` only if the peek needs a small helper, test files,
`docs/TESTING_STRATEGY.md` §0.

**Out of scope**: `capabilities/**`, `build.rs` (no new commands — hover
stays event-push, receive-only); `set_ignore_cursor_events` call sites
(byte-identical, the standing 087 rule); the weather-art assets and mood
tables (reuse only); priority/supersession semantics; 092's chip/content
styling.

## Key STOP conditions (finalize at the pass)

- Either prerequisite grep fails (091/092 not merged).
- The TTL hold cannot be expressed through the existing
  mutate→wake→emit protocol without a new side channel into the queue —
  report the shape you'd need; do not invent a second wake path.
- Repeated hover cycles can extend a card's life unboundedly and the fix
  isn't obvious from the extension rules already in `queue.rs` — STOP
  with the failing property sketch.
- The y-span fix requires `set_ignore_cursor_events` changes or a
  capabilities change (it must not).
- The peek and reveal fight over the below-block in a state the
  precedence rule (football > weather) doesn't cover.

## Test plan (headline)

- Rust: hover-hold property tests (never rotates while held; total
  lifetime bounded across N hover cycles); y-span rect tests per state
  at multiple scales; wake-protocol single-emit regression stays green.
- Frontend: peek opens/closes on `hovered` (not CSS :hover); flanks
  un-round while open (double-curve pin); timeline present in the peek;
  reveal shows scorecard only when live match active; precedence pin;
  TtlBar freeze on `hoverPaused`; reduced-motion: transitions off.

## Maintenance notes

- This closes the last deferred halves of 081/082/084 and completes 079
  item 17 (both halves). Items 12/13 shipped via plan 094 and item 16 is
  filed as plan 095, so **this plan is the last build the 079 ledger is
  waiting on** — when it lands, 079 retires to `done/`.

**Review-plan pass (2026-07-21, at `0ea2a96`, after 091 merged)**: the
rust/hover half of this plan is now verified against shipped code rather
than the pre-091 assumptions it was filed with. Materially: 091 rewrote
`active_card_rect` — its width formula no longer branches on `mode`
(only `effective_cutout_width` does), its constants are an entirely new
set (`FLANK_IDLE`/`MIN_FLANK_SHOWING`/`BASE_SHOWING`/`BASE_EXPANDED`/
`HUD_CUTOUT_W`/`HUD_CUTOUT_H`), and the old width constants this plan
would have cited are gone. The y-span fix now has an exact target
(`hover.rs:201`) plus the two existing tests that pin the limitation
(`:566-577`) flagged as update-not-delete. `_has_status_chips` is
documented as deliberately-unused so this plan doesn't "fix" it by
accident. `TtlBar`'s prop shape and re-anchor effect confirmed at
`:18-40`; `promoted_at`/`extension_secs` confirmed at `queue.rs:10-11`
with the auto-retract read (`:247-253`) named as the closest existing
precedent for reasoning from `promoted_at`. The 092-dependent citations
remain unverified by construction — a short re-check at dispatch, noted
in the header.
- The hover interaction model (one below-block at a time; football >
  weather precedence; hold-while-hovered) becomes the contract any
  future hover consumer extends — document it in a comment block in
  hover.rs when building.

**Owed 092-dependent re-check (2026-07-21, at `147a996`, after 092
merged)**: discharged. Both prerequisite greps confirmed live. 092's
content classes are cited exactly above, and one substantive addition
came out of it: 092 retired `.pill` completely, so the peek must reuse
`.chip` rather than reintroducing a pill class — otherwise this plan
would silently undo item 10. Also flagged the new pause indicator so the
peek's interaction doesn't fight it. This plan is now fully stamped and
dispatchable; nothing further is owed before execution.