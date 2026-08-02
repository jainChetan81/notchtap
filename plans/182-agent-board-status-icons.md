# Plan 182: Give AgentBoard's flank the same icon glyphs IconStrip already draws

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat f810d58..HEAD -- src/components/IconStrip.tsx src/components/StatusDots.tsx src/components/AgentBoard.tsx`
> If any of those files changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: polish
- **Planned at**: commit `f810d58`, 2026-08-02
- **Spec**: `docs/superpowers/specs/2026-08-02-agent-board-status-icons-design.md`

## Why this matters

Plan 171 replaced `StatusRailCard`'s right-flank `StatusDots` (plain
colored dots) with the new five-glyph `IconStrip`. It deliberately never
touched `AgentBoard.tsx`, which still mounts the old `StatusDots` in its
own right flank. Result: two different visual languages for "what else is
going on" depending on which surface is on screen. The operator confirmed
(2026-08-02 design conversation) they want AgentBoard's dots redrawn with
the same glyph shapes IconStrip already uses — visual consistency only,
no new interactivity, no semantic change to what the dots communicate.

## Current state

`src/components/IconStrip.tsx:119-159` — the three glyphs this plan
reuses (excerpted; there's a fourth, `NewsGlyph`, that takes a `charge`
prop this plan does NOT need):

```tsx
function FootballGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <circle cx="9" cy="9" r="6.5" stroke="currentColor" strokeWidth="1.4" />
      <path
        d="M9 9 L9 3.5 M9 9 L13.7 11.7 M9 9 L4.3 11.7"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}

function WeatherGlyph(): ReactNode {
  return (
    <svg viewBox="0 0 18 18" fill="currentColor" aria-hidden="true">
      <path d="M5.5 13.5 a3.2 3.2 0 0 1 -0.4 -6.38 a3.6 3.6 0 0 1 6.9 -1.5 a2.9 2.9 0 0 1 -0.3 7.88 z" />
    </svg>
  );
}
```

`NewsGlyph` (`IconStrip.tsx:161-208`) draws a page outline plus a
`useId()`-scoped clipPath and a `charge`-driven fill rect:

```tsx
function NewsGlyph({ charge }: { charge: number }): ReactNode {
  const clamped = Math.max(0, Math.min(1, charge));
  const clipId = useId();
  return (
    <svg viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <rect x="3" y="2.5" width="12" height="13" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
      <clipPath id={clipId}>
        <rect x="3" y="2.5" width="12" height="13" rx="1.5" />
      </clipPath>
      <rect
        className="charge"
        x="3"
        y="2.5"
        width="12"
        height="13"
        clipPath={`url(#${clipId})`}
        fill="currentColor"
        opacity="0.55"
        style={{ transform: `scaleY(${clamped})`, transformOrigin: "9px 15.5px" }}
      />
      <path d="M5.5 6 H12.5 M5.5 8.6 H10" stroke="currentColor" strokeWidth="1.1" strokeLinecap="round" />
    </svg>
  );
}
```

`src/components/StatusDots.tsx` (full file, 93 lines) — the component
this plan modifies. Its three dots today (excerpt, lines 59-92):

```tsx
export function StatusDots({ status }: { status?: StatusState }) {
  const paused = status?.paused ?? false;
  const footballConfigured = status ? status.football.enabled : undefined;
  const newsConfigured = status ? status.news.enabled : undefined;
  const weatherConfigured = status ? status.weather.enabled : undefined;
  const football = !paused && (footballConfigured ?? false);
  const news = !paused && (newsConfigured ?? false);
  const weather = !paused && (weatherConfigured ?? false);
  return (
    <span className="status-dots">
      <span
        className={`status-dot football ${shapeClass(footballConfigured)}${football ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("Football", footballConfigured)}
      />
      <span
        className={`status-dot news ${shapeClass(newsConfigured)}${news ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("News", newsConfigured)}
      />
      <span
        className={`status-dot weather ${shapeClass(weatherConfigured)}${weather ? " active" : " dim"}`}
        role="img"
        aria-label={configuredLabel("Weather", weatherConfigured)}
      />
      {paused && (
        <span className="pause-glyph" role="img" aria-label="Notifications paused">
          <span />
          <span />
        </span>
      )}
    </span>
  );
}
```

`shapeClass`/`configuredLabel` (`StatusDots.tsx:45-57`) stay exactly as
they are — this plan does not touch them.

`src/components/AgentBoard.tsx:521-524` — the mount site (unchanged by
this plan):

```tsx
<div className="flank-right">
  <div className="card-content idle">
    <StatusDots status={status} />
  </div>
</div>
```

CSS: `.status-dot` rules live in `styles.css` (grep `\.status-dot` to
find them — the enabled/disabled/unavailable shape classes and
`.active`/`.dim` opacity rules this plan must not break).

## Commands you will need

Run from repo root.

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass |
| Type check | `npx tsc --noEmit` | exit 0 |
| Lint | `npx biome ci .` | exit 0 |

## Scope

**In scope** (the only files you should modify):
- `src/components/IconStrip.tsx` (export the three glyph functions;
  no behavior change to their bodies)
- `src/components/StatusDots.tsx` (swap shape spans for glyphs)
- `src/components/StatusDots.test.tsx` (new pin test)
- `styles.css` (only if the glyph swap needs new/adjusted `.status-dot`
  sizing rules — see Step 3's escape hatch)

**Out of scope** (do NOT touch):
- `IconStrip.tsx`'s own rendering/props/tests beyond adding the exports
  — the rail's tab strip behavior must not change.
- `AgentBoard.tsx`'s mount site — already correct, needs no edit.
- Any rust file — this is a frontend-only, presentation-only change.
- `NewsGlyph`'s `charge`-fill mechanics — `StatusDots` has no charge
  concept; call it with a fixed `charge={0}` (empty), never wire a real
  value.

## Git workflow

- Branch: `fix/agent-board-status-icons`
- Commit style: conventional, e.g. `feat(agent-board): reuse IconStrip's glyphs for StatusDots`
- Open a PR when done (this repo's established practice — see recent
  history for the pattern: small focused PRs, CodeRabbit + PR-Agent
  review before merge).

## Steps

### Step 1: Export the three glyph functions from IconStrip.tsx

Change `function FootballGlyph()`, `function WeatherGlyph()`, and
`function NewsGlyph({ charge }: { charge: number })` to `export function`.
No other change to their bodies.

**Verify**: `npx tsc --noEmit` → exit 0 (IconStrip.tsx's own render calls
these unqualified — exporting doesn't break local use).

### Step 2: Write the failing pin test

Create `src/components/StatusDots.test.tsx` if it doesn't already exist
(check first — `find src -iname "StatusDots.test.tsx"`; if it exists,
add to it instead of overwriting). Add:

```tsx
import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusDots } from "./StatusDots";

describe("StatusDots glyphs", () => {
  it("renders the football, news, and weather glyphs as SVGs, not plain shape divs", () => {
    const { container } = render(<StatusDots />);
    expect(container.querySelector(".status-dot.football svg")).not.toBeNull();
    expect(container.querySelector(".status-dot.news svg")).not.toBeNull();
    expect(container.querySelector(".status-dot.weather svg")).not.toBeNull();
  });
});
```

**Verify**: `npx vitest run StatusDots` → FAILS (no `svg` inside
`.status-dot` yet — the current markup is an empty span).

### Step 3: Swap the shape spans for glyphs

In `StatusDots.tsx`, import the three glyphs and render them inside each
`<span className="status-dot ...">` instead of leaving it empty:

```tsx
import { FootballGlyph, NewsGlyph, WeatherGlyph } from "./IconStrip";

// ...inside the JSX, each span gains its glyph as a child:
<span
  className={`status-dot football ${shapeClass(footballConfigured)}${football ? " active" : " dim"}`}
  role="img"
  aria-label={configuredLabel("Football", footballConfigured)}
>
  <FootballGlyph />
</span>
<span
  className={`status-dot news ${shapeClass(newsConfigured)}${news ? " active" : " dim"}`}
  role="img"
  aria-label={configuredLabel("News", newsConfigured)}
>
  <NewsGlyph charge={0} />
</span>
<span
  className={`status-dot weather ${shapeClass(weatherConfigured)}${weather ? " active" : " dim"}`}
  role="img"
  aria-label={configuredLabel("Weather", weatherConfigured)}
>
  <WeatherGlyph />
</span>
```

Do not change `shapeClass`, `configuredLabel`, the `paused`/pause-glyph
block, or any prop/logic above the return statement.

**Verify**: `npx vitest run StatusDots` → PASSES. Then `npx vitest run`
(full suite) → all pass, confirming no other test asserted on the old
empty-span markup.

**Escape hatch**: if the existing `.status-dot` CSS sizes the element
assuming an empty shape (e.g. a fixed small `width`/`height` on the span
itself with no room for an 18x18 viewBox SVG to read clearly), a minimal
CSS addition constraining the SVG's rendered size (e.g. `.status-dot svg
{ width: 100%; height: 100%; }`) is in scope. Do not redesign the flank
layout — if a bigger visual change seems needed, STOP and report instead
of guessing at new dimensions.

### Step 4: Full verification

Run all three commands from "Commands you will need". All must pass.

### Step 5: Commit and open PR

```bash
git add src/components/IconStrip.tsx src/components/StatusDots.tsx src/components/StatusDots.test.tsx
git commit -m "feat(agent-board): reuse IconStrip's glyphs for StatusDots"
git push -u origin fix/agent-board-status-icons
gh pr create --title "feat(agent-board): reuse IconStrip's glyphs for StatusDots" --body "Implements docs/superpowers/specs/2026-08-02-agent-board-status-icons-design.md. Visual-only: StatusDots now draws the same football/news/weather glyphs IconStrip uses instead of plain shape divs, no change to enabled/disabled/unavailable/paused semantics."
```

## Test plan

- `StatusDots.test.tsx`'s new pin test (Step 2) — confirms SVGs render.
- Every existing `StatusDots`-adjacent test (if any exist under
  `AgentBoard.test.tsx` asserting on `.status-dot` markup) must still
  pass unchanged — this is a rendering-only swap.

## Done criteria

- [ ] `npx vitest run` exits 0
- [ ] `npx tsc --noEmit` exits 0
- [ ] `npx biome ci .` exits 0
- [ ] `.status-dot.football/.news/.weather` each contain an `svg` (Step 2's test)
- [ ] No files outside the in-scope list modified (`git status`)
- [ ] `plans/README.md` status row updated

## STOP conditions

- `IconStrip.tsx`'s glyph functions no longer match the excerpts above
  (a concurrent change landed) — re-diff before proceeding.
- The enabled/disabled/unavailable visual distinction (the plan-110
  accessibility shape fallback) becomes hard to read once the glyph
  swap lands — this is a design question (see spec's own escape hatch),
  not an implementation detail to improvise past.
- `StatusDots.tsx` already imports from `IconStrip.tsx` under a
  different name/shape than described here (unlikely, but check) —
  reconcile rather than creating a duplicate import.

## Maintenance notes

- If `IconStrip.tsx`'s glyphs are redrawn in the future (the file's own
  header comment calls them "a first pass, not a locked asset"),
  `StatusDots` picks up the change automatically via the shared export —
  no separate update needed, which is the point of this plan.
- On-hardware visual check (Mac Mini/MacBook) is operator-owed, same
  class as other physical-hardware verification in this repo: confirm
  the icons read clearly at AgentBoard's flank size in both light/dark
  system appearance if applicable.
