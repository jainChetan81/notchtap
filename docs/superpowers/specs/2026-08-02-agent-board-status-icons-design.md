# AgentBoard status icons (visual consistency with IconStrip)

**Date:** 2026-08-02
**Status:** approved, pending implementation plan

## Problem

Plan 171 replaced `StatusRailCard`'s right-flank `StatusDots` with the new
five-glyph `IconStrip` (`src/components/IconStrip.tsx`). It deliberately did
not touch `AgentBoard` — `AgentBoard.tsx` still mounts the pre-171
`StatusDots` component (plain colored dots) in its own right flank
(`AgentBoard.tsx:521-524`). The result: two different visual languages for
"what else is going on" depending on which surface is on screen (rail vs.
board), which reads as inconsistent.

## Non-goals

- Making the icons on `AgentBoard` clickable / a tab switcher. `mode`
  (board vs. rail) is computed purely in the frontend from
  `slot`/`agentState.sessions.length`/`status.paused`
  (`src/lib/presentation.ts:300-312`) and nothing today lets a tab
  selection affect it. `CONTEXT.md` documents the Agent Board as
  automatic-only (shows whenever sessions are present, returns
  automatically once a Notification finishes) — this spec does not reopen
  that decision.
- Adding new sources. `StatusDots` has always tracked exactly three:
  Football, News, Weather (`StatusDots.tsx:3-4`, plan 091). No Agent icon
  (redundant — you're already on the Agent surface) and no Music icon
  (never part of this component's scope).
- Changing `StatusDots`' existing semantics. It encodes *configuration*
  state (enabled/disabled/status-unavailable per source, `StatusDots.tsx:
  45-57`) plus a paused indicator — a different meaning than `IconStrip`'s
  hidden/present/live tiers (which encode *live content* presence). This
  spec does not change what the indicator communicates, only how each dot
  is drawn.

## Approach

Extract the `FootballGlyph`, `NewsGlyph` (charge-free — `StatusDots` has no
concept of the news charge fill), and `WeatherGlyph` SVG-drawing functions
out of `IconStrip.tsx` into small shared exports (e.g.
`src/components/icon-glyphs.tsx`, or exported directly from
`IconStrip.tsx` if that's simpler — implementation plan's call). Swap them
into `StatusDots`' three existing `<span>` elements in place of the plain
shape divs, keeping every other line of `StatusDots.tsx` (the
enabled/disabled/unavailable shape-fallback classing, the pause glyph, the
`aria-label`s) untouched.

`IconStrip`'s `NewsGlyph` currently takes a `charge: number` prop for its
fill animation — `StatusDots` has no charge concept, so `StatusDots` calls
it with `charge={0}` (fully empty, matching its existing "no fill" visual)
or the glyph gets an optional prop. Implementation plan's call which reads
cleaner.

## Data flow

Unchanged. `StatusDots` still receives the same `status?: StatusState`
prop and computes the same `football`/`news`/`weather`/`paused` booleans
it does today (`StatusDots.tsx:60-66`). Only the JSX inside each `<span>`
changes from an empty shape div to an inline glyph SVG.

## Testing

- All existing `StatusDots` tests continue to pass unchanged — this is a
  rendering-only swap, not a behavior change.
- Add a pin test confirming the SVG glyph markup now appears inside
  `.status-dot.football`/`.news`/`.weather` (mirroring the existing
  `IconStrip.test.tsx` geometry-pin pattern used for the news charge
  rect), so a future edit can't silently regress to the old shape divs.
- Visual check on hardware (Mac Mini/MacBook): confirm the icons render
  at a legible size in `AgentBoard`'s flank and the enabled/disabled/
  unavailable states remain visually distinguishable (this was the
  original point of the plan-110 shape fallback — a pure glyph swap must
  not lose that signal, e.g. via opacity/stroke changes alongside the
  shape change if needed).

## Escape hatch

If swapping in the full glyph SVGs makes the flank visually cramped or
the enabled/disabled/unavailable distinction becomes hard to read at
`AgentBoard`'s flank size, STOP and report back rather than improvising a
new visual treatment — that's a design question, not an implementation
detail.
