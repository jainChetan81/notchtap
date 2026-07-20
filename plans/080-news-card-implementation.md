# Plan 080: Implement the approved news card — compact published-time meta + full-width expanded summary

> **Executor instructions**: This is a build plan for the news card
> exactly as mocked and operator-approved in `prototype/news-card.html`
> (plan 079 item 7, locked 2026-07-20 — see
> `plans/frontend-ui-consolidated.html`'s "Locked decisions" list). The
> compact card keeps today's shipped render verbatim and gains ONE
> element (published-time meta in the pills row); the expanded news
> manifest drops its 3-column grid for a full-width summary + one inline
> meta row + footer hints. Follow the steps in order. The
> preview-overlay.css mirror law is absolute here: every `styles.css`
> change in this plan lands in `src/settings/preview-overlay.css` in the
> SAME commit, or the Appearance preview silently diverges. When done,
> update the status row for this plan in `plans/README.md` — unless a
> reviewer dispatched you and told you they maintain the index.
>
> **Drift check (run first)**: `git diff --stat 71e54a7..HEAD -- src/components/StatusRailCard.tsx src/components/Manifest.tsx src/styles.css src/lib/presentation.ts src/settings/preview-overlay.css prototype/news-card.html`
> Any diff in the five source files means line refs below have shifted —
> re-read before editing. Any diff in `prototype/news-card.html` is a
> STOP condition (see below): the prototype is the approved design of
> record, and if it changed, what was approved may have changed too.

## Status

- **Priority**: P1 (first buildable slice of the locked 079 redesign;
  the news card is the template every other card's compact/expanded
  layout follows)
- **Effort**: M
- **Risk**: LOW-MED — frontend-only, but touches the card users see most
  often after the idle rail
- **Depends on**: 063 (idle-rail wrap bug) ships first — locked
  sequencing; the news card lands on top of 063's shipped width/wrap
  mechanics, not against the pre-063 rail. Plan 079 item 7 is the locked
  decision this implements.
- **Category**: direction (locked) → build
- **Planned at**: commit `71e54a7`, 2026-07-20

## Why this matters

Plan 079 item 7 is locked and the mockup is complete
(`prototype/news-card.html`, embedded live in `prototype/notch-states.html`
§5). Today's news card hides one piece of wire metadata it already
carries — `publishedAtMs` arrives in every news payload and
`publishedLabel` already formats it, but only the *expanded* 3-column
manifest shows it, squeezed into a side column. The approved design
puts `published 18:34` directly on the compact card and gives the
expanded summary its full width back (the 3-column grid's two side
columns are mostly empty today). This is also the smallest, most
self-contained slice of the 079 redesign — landing it first proves out
the prototype→implementation pipeline (prototype classes → `styles.css`
→ `preview-overlay.css` mirror → vitest) that 081/082/084 then reuse.

## Current state

- `src/components/StatusRailCard.tsx:107-123` — the compact news branch:
  `.masthead` (dot + source), `.title.headline`, and the `.pills` row
  rendering category + age pills only. `renderedNewsAge` is computed at
  `StatusRailCard.tsx:81` via `ageLabel`; there is no published-time
  render anywhere in the compact card.
- `src/lib/presentation.ts:126-134` — `publishedLabel(publishedAtMs,
  _nowMs)` already exists and returns `"HH:MM"` (or `null`); it is
  currently consumed ONLY by `Manifest.tsx:33`. No new formatting logic
  is needed — the compact card just has to call it.
- `src/components/Manifest.tsx:46-84` — the expanded news branch: a
  3-column `.manifest-inner.news` grid (Summary / Source·Published /
  Category·Control). `src/styles.css:685-686` sets
  `.manifest-inner.news { grid-template-columns: 1.5fr 1fr 1fr; }` and
  tints its `.detail-label` with `var(--cat)`.
- `src/styles.css` news tokens (all carry over untouched): `.cat-*`
  custom properties 611-617, `.news-shade` 619-634, `.masthead`
  636-643, `.title.headline` 645-649, `.pills`/`.pill.category`/
  `.pill.age` 651-658, `pill-enter` animation 660-676.
- The approved design, `prototype/news-card.html`:
  - Compact (lines 57-90, example at 202-209): identical to shipped,
    plus `.pub-meta` — `published 18:34` tucked at the RIGHT end of the
    pills row (`margin-left: auto`, quiet 8px mono, `rgba(255,255,255,0.32)`),
    styled as a text meta, NOT a third pill (no background/border).
  - Expanded (lines 92-106, example at 302-309): `.manifest-block` with
    a full-width `.manifest-label` SUMMARY + `.manifest-text`, then ONE
    `.manifest-meta` inline row —
    `<b>NDTV</b> · published 18:34 · Politics` — then a
    `.manifest-footer` right-aligned hint: `⌃⇧O read · ⌃⇧N collapse`
    when a link exists, `⌃⇧N collapse` otherwise (same conditional as
    today, `Manifest.tsx:73-81`).
  - The prototype's lifecycle demo (its §3) and TTL bar are NOT this
    plan's scope — lifecycle already ships (plan 033), the TTL bar is
    plan 081.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Frontend unit tests | `npx vitest run` | all pass |
| Typecheck | `npx tsc --noEmit` | exit 0 |
| Lint + format gate | `npx biome ci .` | exit 0 |
| Frontend build | `npx vite build` | exit 0 |

## Scope

**In scope**:
- `src/components/StatusRailCard.tsx` — news compact branch only: add
  the published-time meta element to the pills row.
- `src/components/Manifest.tsx` — news expanded branch only: replace
  the 3-column grid with the full-width summary + inline meta row +
  footer hints.
- `src/lib/presentation.ts` — reuse `publishedLabel`; add at most a
  tiny composition helper if the `published 18:34` prefix wants one
  (a template literal at the call site is also fine — match surrounding
  style).
- `src/styles.css` — add `.pub-meta`, `.manifest-meta`, and any classes
  the new expanded layout needs; remove or repurpose the
  `.manifest-inner.news` grid rule (styles.css:685) once nothing renders
  it.
- `src/settings/preview-overlay.css` — mirror every one of those CSS
  changes, SAME commit (DESIGN.html Law #7, restated in
  `plans/frontend-ui-consolidated.html`'s constraints).
- `src/components/StatusRailCard.test.tsx` / `Manifest`-adjacent vitest
  assertions — update minimally, add the new coverage.

**Out of scope**:
- The generic (non-news) card branch and the generic manifest layout —
  untouched. The cmux/generic restyle is a separate future plan.
- The TTL progress bar (plan 081), hover-expand of a compact card
  (gated on plan 086), the new 3-block notch shape itself (a later
  079-slice plan), idle-rail content changes.
- Any rust change — the wire payload already carries everything this
  design needs (`publishedAtMs`, `source`, `category`, `link`).
- Any change to `ageLabel`'s format or the shipped age-pill styling
  (079 item 14 is still open — keep them exactly as-is).

## Steps

### Step 1: Compact card — published-time meta

In `StatusRailCard.tsx`'s news branch, compute `renderedNewsPublished`
alongside `renderedNewsAge` (line 81) using
`publishedLabel(renderedSlot.publishedAtMs, Date.now())`, and render it
as the last child of the `.pills` row:

```tsx
{renderedNewsPublished !== null && (
  <span className="pub-meta">published {renderedNewsPublished}</span>
)}
```

Match the prototype exactly: the meta sits at the right end of the row
(`margin-left: auto` in CSS), quiet uppercase mono, no pill chrome. If
both category and age are null the pills row doesn't render today
(`StatusRailCard.tsx:114`); publishedAtMs is set on every rss_poller
item, but keep the same null-guard discipline — the meta renders only
when the row renders, and the row's condition stays as-is (category or
age present) OR gains `|| renderedNewsPublished !== null` if you find a
real payload where it matters; note which you chose and why in the
completion report.

**Verify**: `npx vitest run` → all pass (no new test yet — Step 4 adds
it; this step must not break existing assertions).

### Step 2: Expanded news manifest — full-width summary + inline meta

Rewrite `Manifest.tsx`'s `eventType === "news_item"` branch
(`Manifest.tsx:46-84`) to the prototype's structure: a
`.manifest-label` SUMMARY, the full-width `.manifest-text` body (plain
text, NOT markdown — news bodies stay un-marked-down exactly as today,
`Manifest.tsx:50`), one `.manifest-meta` row —
`<b>{source ?? "RSS"}</b> · published {publishedLabel(...)} · {categoryLabel(...)}`
with each segment guarded the way the current columns are (published
and category render only when non-null, `Manifest.tsx:56-71`), and a
`.manifest-footer` with the same `⌃⇧O read · ⌃⇧N collapse` /
`⌃⇧N collapse` conditional on `hasLink` that exists today. The
`.manifest-inner.news` wrapper and its three `detail-label`/
`detail-value` cells go away entirely.

**Verify**: `npx vitest run` → all pass; `npx tsc --noEmit` → exit 0
(the branch drops its `newsPublished`/`newsCategory` column usage —
leave no unused locals).

### Step 3: styles.css + preview-overlay.css mirror (same commit)

Add to `src/styles.css`, next to the existing news block (611-686):
`.pub-meta` (prototype line 84 verbatim), `.manifest-meta` +
`.manifest-meta b` + `.manifest-meta .sep`, `.manifest-footer` +
`.manifest-hint` (prototype lines 99-106), and `.manifest-text` if the
generic manifest's `.detail-value.message` styling doesn't already
cover it — check before adding a duplicate. Remove the
`.manifest-inner.news` rules at styles.css:685-686 (nothing renders
that class after Step 2) — but keep `.manifest-inner.news
.detail-label`'s `var(--cat)` tint idea alive on the new
`.manifest-label` (the prototype colors it `var(--cat)`, prototype
line 97). Then mirror EVERY added/removed rule in
`src/settings/preview-overlay.css` under its `.appearance-preview`
scoping, in the same commit. Add `prefers-reduced-motion` handling only
if a new animation is introduced (none should be — this plan adds no
animations).

**Verify**: `npx vite build` → exit 0; eyeball-grep that every new
class name in `styles.css` also appears in `preview-overlay.css`
(`grep -o '\.pub-meta\|\.manifest-meta\|\.manifest-footer\|\.manifest-hint\|\.manifest-label\|\.manifest-text' src/settings/preview-overlay.css | sort -u`).

### Step 4: Tests + full gate

**Verify**:
- `npx vitest run` → all pass, including the new/updated assertions
  from the Test plan below.
- `npx tsc --noEmit` → exit 0.
- `npx biome ci .` → exit 0.
- `npx vite build` → exit 0.

## Test plan

Following `docs/TESTING_STRATEGY.md` — render states are vitest's job;
visual look stays manual:

- **New vitest**: compact news card renders `published HH:MM` in the
  pills row when `publishedAtMs` is set, and omits it when null.
- **New vitest**: expanded news manifest renders the summary full-width
  (assert the `.manifest-meta` row exists with source · published ·
  category content) and does NOT render `.manifest-inner.news` / the
  old 3-column labels (`Source / Published`, `Category / Control` gone).
- **Update minimally**: existing StatusRailCard/Manifest assertions that
  query the old column labels or the `.manifest-inner.news` structure —
  repoint them at the new structure without weakening their intent
  (plan 078's precedent: narrowing query scope is fine, dropping
  coverage is not). The `⌃⇧O read` conditional-on-link assertions must
  survive, repointed at `.manifest-footer`.
- **Manual-only** (operator, per TESTING_STRATEGY §5): the visual match
  to `prototype/news-card.html` — meta alignment at the row's right
  end, meta row separators, footer hint placement. Check the Appearance
  preview in the Settings window shows the same (that's the mirror-law
  smoke).

## Done criteria

- [ ] Compact news card shows `published HH:MM` at the right end of the pills row, absent when `publishedAtMs` is null
- [ ] Expanded news manifest is full-width summary + one inline meta row + footer hints; `.manifest-inner.news` no longer exists in component or CSS
- [ ] `src/settings/preview-overlay.css` carries every CSS change, same commit (`git show --stat` shows both CSS files)
- [ ] `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`, `npx vite build` all exit 0
- [ ] No diff in any rust file, and no diff in the generic (non-news) card/manifest branches (`git diff src/components` shows only the news branch + shared imports)
- [ ] `plans/079-checklist.html` and `plans/frontend-ui-consolidated.html` statuses updated (news card → implemented); `plans/README.md` row for 080 updated

## STOP conditions

- **Prototype drift**: `prototype/news-card.html` no longer matches what
  the operator approved (the drift-check diff is non-empty, or its
  structure disagrees with this plan's description) — stop and confirm
  which is current rather than guessing.
- **Mirror-law risk**: you find yourself unable to mirror a CSS change
  into `preview-overlay.css` (e.g. a rule that can't be scoped under
  `.appearance-preview`) — stop and surface it; do NOT land an
  unmirrored `styles.css` change.
- Plan 063 has not shipped yet — its width/wrap mechanics are this
  card's baseline; landing the news card first risks a rebase against
  moving width rules. Confirm 063's status row before starting.
- A payload-shape surprise: a real news item where `publishedAtMs` is
  null but the compact card still needs the meta (or vice versa) —
  don't invent fallback formatting; present the case to the operator.

## Maintenance notes

- This plan implements 079 item 7 only. When it lands, mark the news
  card row in `plans/079-checklist.html` and the "News card (079 item
  7)" entry in `plans/frontend-ui-consolidated.html`'s Locked-decisions
  list as implemented, and move it in that page's "Next moves" list.
- The new `.manifest-label`/`.manifest-meta`/`.manifest-footer` classes
  are written to be reused by the generic card's future restyle (079
  item 19) — name them generically (as above), not `news-*`.
- Plan 081 (TTL bar) lands its bar element at the bottom of this same
  card; leave no `.pills`-row margin/padding hack that a 2px bar at the
  card's bottom edge would fight.
