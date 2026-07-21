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
> **NOT YET DISPATCHABLE**: filed while 091 was executing and before 092.
> Requires a review-plan pass at dispatch: stamp SHAs, re-verify every
> citation against shipped 091+092, and re-check the hover.rs geometry
> (091 rewrote `active_card_rect`'s formulas and constants).
>
> **Drift check (run first, after stamping)**:
> `git diff --stat <stamp at dispatch>..HEAD -- src-tauri/src/hover.rs src-tauri/src/lib.rs src-tauri/src/queue.rs src-tauri/src/engine.rs src/App.tsx src/components/ src/styles.css src/settings/preview-overlay.css src/lib/weatherArt.ts`
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
- **Planned at**: `<stamp at dispatch, post-092>`

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

## Current state (pre-091 facts — re-verify at the review-plan pass)

- `src/App.tsx:86-97` — `hover-changed` listener → `hovered` state →
  prop. Already shipped, tested (087: `.hovered` class toggles).
- `src-tauri/src/hover.rs` — `active_card_rect` (091 REWRITES its
  formulas/constants — cite the post-091 form at the pass), the
  tracking-area wiring, `css_top_down_to_appkit_y` (tested).
- `src-tauri/src/queue.rs` — rotation deadlines/`next_deadline`,
  extension rules (`MIN_REMAINING_ON_SUPERSEDE_SECS`, expand top-ups);
  `src-tauri/src/engine.rs` — the mutate→wake→emit protocol every
  deadline change must follow.
- `src/components/TtlBar.tsx` — deadline-anchored rAF bar; re-anchors on
  `remainingMs`/`ttlMs`/`slotId` change (`:10-14`).
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
  item 17 (both halves) — after this plan plus 092, the 079 ledger holds
  only items 12/13 (app icon) and the item-16 MediaRemote spike.
- The hover interaction model (one below-block at a time; football >
  weather precedence; hold-while-hovered) becomes the contract any
  future hover consumer extends — document it in a comment block in
  hover.rs when building.
