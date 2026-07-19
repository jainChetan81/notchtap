# Plan 056: SPIKE — richer live-match scorecard visual (flags, bigger score, icon events)

> **Executor instructions**: Decision spike. The reference image the
> operator provided (a broadcast-style scoreboard graphic: team flags,
> large centered score, icon-annotated event ticker) does not fit this
> app's card dimensions as-is — Step 0 below is a hard size-budget
> constraint check, not optional context.

## Status

- **Priority**: P3
- **Effort**: M (design) / M-L (build)
- **Risk**: LOW-MED — visual-only, but touches a card every football fan
  sees constantly if shipped wrong.
- **Depends on**: 042 (done — current collapsed scorecard: Clock + per-side
  Cards detail cells)
- **Category**: direction
- **Planned at**: commit `f58ced2`, 2026-07-19 — filed from operator
  feedback: "I don't like this design at all" on the current
  text/label scorecard, reference image attached showing a TV-style
  MEXICO 2-0 SOUTH AFRICA graphic (flags, giant score, icon+name event
  list).

## Why this matters

The current collapsed live-match card (plan 042, `StatusRailCard.tsx:148-155`,
reusing `detail-label`/`detail-value` — "HOME CARDS 🟨", "AWAY CARDS
🟨🟨🟥", "CLOCK 58'") is plain, monospace-label-driven, and reads more
like a debug panel than a sports score. The operator's reference image is
a genuinely different visual language: team flags instead of 3-letter
codes, a large centered score instead of a title-line score, and an
icon+player-name event ticker instead of aggregate card counts.

## Current state — the hard constraint

The overlay window is fixed at 500x300px (`tauri.conf.json:18-19`); the
compact card is 400px wide (`.rail-card`, `src/styles.css:25`), 500px
expanded. The reference graphic is built for a full-width broadcast lower
third — easily 1200px+ with large flag art and multi-line event history.
**A literal copy will not fit.** Any redesign has to be re-derived for a
~400px card, not scaled down from the reference 1:1 (scaling down loses
legibility on flags/text at that size anyway).

What *does* translate directly from the reference, scaled to this card's
budget:

- **Flags instead of 3-letter team codes** — small (16-20px) flag icons
  are legible at that size and more scannable than "ARS"/"CHE" text.
  Needs a flag-icon source (a small SVG/emoji flag set keyed by
  league/team — ESPN's scoreboard payload includes team country/league
  data already fetched by `poller.rs`, worth checking what's already
  available before sourcing new assets).
- **A bigger, more prominent score** — currently folded into the card
  title text ("ARS 2–0 CHE"). Could become its own visually distinct
  element (larger font, centered) without needing the card to grow.
- **An icon for the scoring event** — plan 041 already extracts
  structural event data (goal/penalty/own-goal/card,
  `labeled_detail_line`, `poller.rs`) that a small icon set could key
  off, replacing or supplementing the current text label.

What does **not** translate at this size: a multi-row event history
ticker (the reference shows 4+ events with timestamps) — the card has
room for the *current* moment, not a scrolling log, without a genuinely
different interaction (e.g. the event list living only in the expanded
manifest, not the collapsed view).

## Decision needed (operator)

1. Confirm the scope: collapsed-card visual refresh only (flags + bigger
   score, still Clock/Cards detail cells or a replacement), or does the
   expanded manifest also get an event-history redesign?
2. Flag source: which asset set, and is per-team flag art acceptable
   scope for this app (adds a new asset dependency) vs. staying
   text-only but more visually weighted?
3. Confirm Option B (never-pin, plan 042's decision) still holds — a
   bigger/richer card design shouldn't quietly reopen "should this pin
   open during a match," which was already decided against.

## Recommendation

Scope to the collapsed card first (flags + prominent score), reusing the
existing `detail-label`/`detail-value` cells for Cards/Clock rather than
inventing a new layout primitive — keeps this additive to plan 042
instead of a rewrite. Defer the event-ticker idea; it's a bigger,
separate interaction question (arguably overlaps with plan 043's
richer-event-coverage work, which is itself gated on an unresolved ESPN
data-availability question).

## Maintenance notes

- Coordinates with plan 043 (richer match events) — if 043's Step 0
  confirms ESPN's play-by-play data exists, an event-icon ticker becomes
  much more buildable; if not, this stays limited to what `poller.rs`
  already extracts (goal/card/penalty/own-goal).
