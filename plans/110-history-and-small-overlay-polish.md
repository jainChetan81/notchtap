# Plan 110: History richness + small verified overlay polish (is_day, news time, dot labels)

> **Executor instructions**: Follow step by step; run every verification
> command. On any STOP condition, stop and report. The reviewer
> maintains `plans/README.md` — do not edit it.
>
> **Worktree preflight**: `git log --oneline master ^HEAD`; if it prints
> anything, `git merge --ff-only master`. Then `npm ci`.
>
> **Drift check**: `git diff --stat 870cdeb..HEAD -- src/components src/settings/SettingsApp.tsx src/settings/settings.css src-tauri/src/status.rs src-tauri/src/weather_poller.rs`
> — 107/108/109 land first and overlap; rebase your reading on
> post-109 master. On mismatch beyond their scopes, STOP.

## Status

- **Priority**: P3
- **Effort**: M (four independent small items batched — each is S)
- **Risk**: LOW-MED (item B touches the status wire shape; the
  `dedup_eq` rule applies — see below)
- **Depends on**: 107, 108, 109 merged (file overlap only, no
  semantic dependency). Land before 111.
- **Category**: polish
- **Planned at**: commit `870cdeb`, 2026-07-22

## Why this matters

Four verified low-risk findings from the 2026-07-22 review:

- **A. History discards most of what it stores.** `HistoryEntry`
  carries `event_type`, `priority`, `rotation`, `topic`, `origin`,
  `meta.{source, category, published_at_ms, link, subtitle,
  details[], espn}` — the list renders only time/origin/title/body
  (`SettingsApp.tsx:1357-1365`). And `.history-row`/`.history-body`
  (settings.css:887/:915) have NO `overflow-wrap`, so one long token
  (a URL in a body) breaks the layout.
- **B. Ambient day/night is guessed from the local clock.**
  `IdleHoverPeek.tsx:59-62` `isDaytimeNow()` uses
  `new Date().getHours()` because the ambient `WeatherSummary` wire
  shape is `{tempDisplay, condition}` only (`status.rs:53-58`). The
  poller ALREADY fetches Open-Meteo's `is_day`
  (`weather_poller.rs:267` requests it, `:68` parses it) and already
  ships it on the weather ALERT card via `meta.details` `wx.is_day`
  (plan 082, `:217-250`) — the data exists; only the ambient channel
  drops it. Configured coordinates outside the machine's timezone
  show the wrong scene.
- **C. News compact row shows two timestamps.**
  `StatusRailCard.tsx:456-471` renders both `renderedNewsAge`
  ("5m ago") and `.pub-meta` "published 14:32" in the same compact
  metadata row — redundant at compact size.
- **D. Status dots are opaque to assistive tech and color-only.**
  `StatusDots.tsx:29-36`: bare spans, color classes only, no
  aria-label; the paused `.pause-glyph` is `aria-hidden="true"`
  (`:38`) — a sighted-only signal. (The bigger "enabled vs healthy"
  semantics question is deliberately NOT here — that's a product
  decision parked with the other behavior items.)

## Current state

See the verified citations above; additionally:

- `src/useStatusState.ts:17-20` — frontend
  `WeatherSummary = { tempDisplay; condition }`.
- `IdleHoverPeek.tsx:50-58` — the comment explicitly says the wire
  shape lacks the flag ("there is no real sunrise/sunset value on
  this channel").
- `presentation.ts:253-282` — `ageLabel` (relative) and
  `publishedLabel` (HH:MM) helpers; both tested.
- **`dedup_eq` rule (CLAUDE.md)**: continuously-varying wire fields
  must extend `dedup_eq` explicitly. `is_day` is NOT continuously
  varying (flips twice/day) — it belongs in normal content equality
  so a day→night flip IS a content change and repaints the art.
  **(Cold-read resolved this, 2026-07-22 — no investigation needed)**:
  the ambient status channel does NOT use `dedup_eq` (that is
  SlotState-only, `event.rs:333`); it dedups via
  `status_state_if_changed` (`status.rs:179`) comparing `last` vs
  `next` through DERIVED `PartialEq` on `StatusState` (`:19`) and
  `WeatherSummary` (`:53`). Adding `is_day: bool` therefore
  auto-participates in equality — a flip is a content change and
  repaints, for free. Verify this stays true (the derive is still
  present) rather than re-deriving the analysis.

## Commands you will need

Frontend gates from root (`npx vitest run` / `npx tsc --noEmit` /
`npx biome ci .` / `npx vite build`); rust gates from `src-tauri/`
(`cargo test --locked`, clippy `-D warnings`, `fmt --check`) — item B
touches rust.

## Scope

**In scope**: `src-tauri/src/status.rs`, `src-tauri/src/
weather_poller.rs`, `src-tauri/src/engine.rs` (only if
`update_weather`'s signature carries the summary), their tests;
`src/useStatusState.ts`, `src/components/IdleHoverPeek.tsx`,
`src/components/StatusRailCard.tsx`, `src/components/StatusDots.tsx`,
their tests; `src/settings/SettingsApp.tsx` (history section),
`src/settings/settings.css` (history styles); `src/styles.css` +
`src/settings/preview-overlay.css` ONLY if C/D need CSS (mirror law);
`docs/TESTING_STRATEGY.md` §0 last.

**Out of scope**: history filtering UI (review's "optionally support
filtering" — defer; add only the metadata row + expandable details);
now-playing artwork (review itself says don't block on it); any
change to what history STORES (display only); `build.rs`/
`capabilities/` (byte-untouched is a done criterion — `get_history`
already returns the full entries).

## Steps

### Step A: history display
1. Metadata row per entry: source chip (when present), category,
   priority, event_type — compact, muted, using 109's type tokens
   (the `--fs-*` custom properties 109 defines in settings.css's
   `:root` — read that file post-109-merge for the exact names;
   `--fs-caption` is the intended tier for this row).
2. Expandable details: a native `<details>`/`<summary>` (semantics
   free of charge, no JS state) revealing subtitle, `details[]`
   label/value pairs, and the link (rendered as a plain anchor —
   settings window MAY open links; confirm existing precedent for
   links in settings before adding `target`; if none exists, render
   the URL as selectable text instead — STOP-lite: note the choice).
3. `overflow-wrap: anywhere;` on `.history-body` (and the new details
   values).
**Verify**: tests — an entry with full meta renders chips + details;
an entry with empty meta renders no empty chrome; a 300-char
unbroken-token body doesn't widen the row (assert the CSS class
carries the rule; jsdom can't measure — pin the class).

### Step B: `is_day` on the ambient weather channel
1. `status.rs` `WeatherSummary` += `is_day: bool`; populate it in the
   poller's summary-construction path from the already-parsed `:68`
   value.
2. Thread through whatever constructs the frontend payload
   (`engine.update_weather` / `StatusState::snapshot` — follow the
   existing field pattern).
3. Frontend: `useStatusState.ts` type += `isDay: boolean`
   (serde-case: match the existing casing convention on this wire —
   check how `tempDisplay` is serialized). **(Added at plan review,
   2026-07-22)**: extend the RUNTIME guard too —
   `isValidWeatherSummary` (`useStatusState.ts:79`) checks every
   field by contract (the comment at `:87` says so explicitly);
   adding `isDay` to only the TS type would let a payload missing or
   mistyping it pass validation and reach `weatherArtFor` as
   `undefined` (silently wrong night scene instead of a rejected
   payload). Require `typeof v.isDay === "boolean"`; add an
   invalid-payload test (missing isDay → summary rejected).
4. `IdleHoverPeek.tsx`: `weatherArtFor(weather.condition,
   weather.isDay)`; DELETE `isDaytimeNow()` and its comment block;
   keep a clock **fallback only if** the field can genuinely be
   absent mid-upgrade (old rust + new frontend can't happen in a
   Tauri app — same binary; no fallback needed, say so).
5. Dedup: per Current state, place `is_day` in normal equality on the
   status channel so a flip repaints; verify no tick-storm results
   (it changes twice a day).
**Verify**: rust test — summary built from a fixture with `is_day: 0`
carries `false`; frontend test — art class matches `isDay` from the
payload, not the wall clock (freeze/mock Date to prove the clock is
no longer consulted).

### Step C: news compact row — one timestamp
**(Corrected at review round 2)** — the expanded side needs NO
addition: `src/components/Manifest.tsx:33-38` ALREADY renders
`published {HH:MM}` (via `publishedLabel`) in the expanded manifest's
meta segments. The original step's "add it to expanded metadata"
would have duplicated it. The work is subtraction only:
- Compact: keep relative age, drop the `.pub-meta published HH:MM`
  node from the compact row (`StatusRailCard.tsx:456-471`).
- Expanded: change NOTHING — and PIN the existing Manifest
  presentation with a test if one doesn't already cover it, so this
  plan's compact deletion can never be "simplified" into deleting
  the expanded rendering too.
- Delete the compact row's `.pub-meta` CSS from BOTH mirror files —
  `src/styles.css` (`:1028`) and `src/settings/preview-overlay.css`
  (`:736`) — ONLY if no RENDERING COMPONENT still uses the class:
  grep `src/**/*.tsx` EXCLUDING test files. **(Cold-read correction,
  2026-07-22 — the old "no other consumer, grep first" gate
  false-blocked)**: a bare grep hits
  `StatusRailCard.test.tsx:505-520`, an existing test that ASSERTS
  `.pub-meta` present in the compact card — that test is not a
  consumer to preserve, it is the OLD contract this step flips:
  rewrite it to assert age present / published absent. Manifest
  renders its published span with NO class (`Manifest.tsx:37`), so
  once the compact node goes the class is genuinely orphaned.
**Verify**: compact-row test asserts age present, published absent
(the rewritten `:505-520` test); Manifest test asserts
`published HH:MM` still renders in expanded meta segments;
`grep -rn "pub-meta" src/ --include="*.tsx"` (tests included) → only
the rewritten assertions, no rendering site.

### Step D: dots minimally legible
1. Each dot: `role="img"` + `aria-label` — "Football — enabled" /
   "…— disabled". **(Corrected at plan review, 2026-07-22)**: derive
   the label from the RAW config flag
   (`status?.football.enabled ?? false`), NOT from the component's
   display booleans — those are `!paused && enabled`
   (`StatusDots.tsx:28-31`), so while paused every dot would
   announce "disabled" even for sources that remain configured-on: a
   false statement about configuration, doubly confusing next to the
   pause glyph's own announcement. Rationale for not compounding the
   labels instead (e.g. "Football — enabled, paused"): one fact per
   element — the dot announces configuration, the pause glyph
   announces the pause; AT users hear both, sighted users see dim +
   glyph, and neither channel lies. The VISUAL dim-under-pause
   behavior is untouched.
2. Paused: remove `aria-hidden` from `.pause-glyph`, give it
   `role="img"` + `aria-label="Notifications paused"`.
3. NO layout/visual change in this plan (labels are for AT; the
   review's fuller "labels in hover reveal + health semantics" is
   parked with the product decisions).
**Verify**: tests query by accessible name for all three dots and the
pause glyph; one test pins the paused case — paused + football
enabled → dot still labeled "Football — enabled" (dim visual class
asserted unchanged) + pause glyph accessible.

### Step E: gates + §0
Full frontend + rust gates → clean. §0 updated with attribution.

## Done criteria

- [ ] History: metadata row + `<details>` expansion + overflow-wrap;
      empty-meta entries render clean
- [ ] `WeatherSummary` carries `is_day` end-to-end; the local-clock
      guess and its comment are gone; art keyed off the wire
- [ ] News compact row has exactly one time expression
- [ ] Dots + pause glyph have accessible names
- [ ] `build.rs`/`capabilities/` byte-untouched; mirror law held where
      CSS changed
- [ ] All gates clean; §0 matches observed counts

## STOP conditions

- The ambient status channel's dedup semantics turn out NOT to be
  the derived-`PartialEq` `status_state_if_changed` comparison
  described in Current state (i.e. the cold-read analysis no longer
  holds at execution time) — report what you found instead. (The
  081 tripwire guards SlotState `dedup_eq`, a DIFFERENT channel —
  do not cite it for this.)
- History link rendering has no safe precedent and plain-text URL
  feels wrong (report; don't invent a new open-URL path — that's an
  IPC-surface question).
- Any of the four items balloons past S on contact (report and land
  the other three).

## Maintenance notes

- Parked deliberately: history filtering; dot health-vs-enabled
  semantics; hover-reveal dot labels; now-playing artwork. All are
  product decisions or bigger features, listed in the README's
  open-items line.
- After B, `wx.is_day` on the alert card's `meta.details` (plan 082)
  and `WeatherSummary.is_day` coexist — different channels, same
  source value. If a third consumer appears, unify then, not now.
