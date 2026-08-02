# 171 — Trim live-scorecard.css's dead rules (plan 170's deferred Step 7)

- **Status**: DONE (2026-08-02) — `live-scorecard.css`'s `.notif-block`/`.event-line` removed as planned. Verifying `.event-line` had zero live consumers turned up a second, related dead block in a DIFFERENT file: `idle-peek.css`'s `.ev-ico*` (11 selectors) and `.event-line.tint-goal/yc/rc` — the same deprecated per-event icon+tint system, confirmed dead via the same repo-wide grep — with one correction from the 2026-08-02 Standards review: it is NOT true that "only comments survive"; `src/lib/presentation.ts`'s `EVENT_KIND_PRESENTATION` still mints `"ev-ico goal"`/`"tint-goal"`-style strings as live `iconClass`/`tintClass` FIELDS. The conclusion holds anyway (grep confirms zero readers of either field — only `celebration` is consumed, StatusRailCard.tsx), but those two dead data fields are now a re-adoption trap with their CSS gone; removing them is deferred (their table rows are pinned by presentation tests, so it's a real small plan, not a drive-by) — `src/lib/presentation.ts` itself is untouched by this commit; only the CSS consuming those two fields is gone. `npx vitest run` 625/625 full suite (NotificationBody.test.tsx 13, StatusRailCard.test.tsx 130) — the "768/768" this line originally recorded was a double-count: 625 (full run) + 143 (those two files re-run standalone as evidence) summed by mistake; per-file counts re-verified 2026-08-02, both exact. `npx tsc --noEmit` clean. (625/625 is this commit's own count; a later commit on this branch, 17d3933, adds one more guard test, bringing the branch tip to 626/626 — see plan 174's Status.)
- **Commit**: 01748dd (branched off 0c08e93, this doc's drift baseline)
- **Severity**: LOW
- **Category**: Cleanup (plan 170's own deferred boundary, not a new finding)
- **Estimated scope**: 2 files, 3 rule blocks removed (scope grew from the 1-file/2-block estimate — see Status)

## Problem

Plan 170 (the football unified-card-template migration) deliberately
deferred trimming `live-scorecard.css`'s now-dead rules until the new
`FootballHeroCard` rendering path was confirmed live, per its own
Boundaries: "Do NOT remove `live-scorecard.css`'s current rules in the
same commit that adds the new template path — land new rendering first,
confirm live, THEN trim dead CSS as a follow-up." That confirmation has
since happened (`LiveMatchScorecard.tsx` — the old bespoke component
these rules served — is now deleted; `FootballHeroCard` in
`NotificationBody.tsx` is the only live consumer of `live-scorecard.css`
today).

Verified directly (not assumed) which of the file's 20 top-level rule
blocks are still live, by checking `FootballHeroCard`'s actual JSX
(`src/components/NotificationBody.tsx:469-520`) against every class name
in `src/overlay/live-scorecard.css`:

**Confirmed dead — zero references in any current `.tsx` render** (only
comments/test-comments confirming they're gone):

```css
/* src/overlay/live-scorecard.css:1-9 (current) */
/* ==== football live-match scorecard (plan 084): the operator-locked
   sticky-presence card's compact render, built on 083's structured espn
   meta. Class names are lifted verbatim from prototype/football-card.html
   so cross-referencing stays greppable. `.notif-block` replaces `.compact`
   entirely for this branch (StatusRailCard.tsx) — no Track, no TtlBar, no
   Manifest; see that file's live-match branch comment for why. ==== */
.card-root .notif-block {
  padding: 12px 16px 13px;
}
```

```css
/* src/overlay/live-scorecard.css:214-222 (current) */
.card-root .event-line {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  margin-top: 9px;
  font-size: 11px;
  color: rgba(255, 255, 255, 0.7);
}
```

`NotificationBody.tsx:441-445` explicitly documents WHY `.event-line`
doesn't carry over: "The old component's `.event-line` icon+tint...
does NOT carry over onto `.title.headline`: that was a flex row built
for an icon beside text, `.title.headline` is a line-clamped text block
with no icon slot, and `slot.body`'s own text already names the event."

**Confirmed still live** (do not touch): `.score-block`, `.sc-head`,
`.chip-league`, `.chip-live` and its `.break`/`.final`/`.live-dot`
variants, `.clock-pill`, `.score-row`, `.side`, `.crest`, `.crest img`,
`.score`, `.score-digit`, `.score-digit-roll`, `.score .dash`,
`.cards-line` — all directly referenced in `FootballHeroCard`'s JSX
(`NotificationBody.tsx:469-520`).

## Target

Remove exactly the two dead rule blocks above (`.notif-block` at
`live-scorecard.css:1-9`, including its now-inaccurate header comment
block since it describes a rendering path that no longer exists; and
`.event-line` at `live-scorecard.css:214-222`). Every other rule in the
file stays byte-identical.

**Scope addendum (discovered during execution, see Status above):** the
same dead per-event icon+tint system also had a live block in a
DIFFERENT file — `idle-peek.css`'s `.ev-ico*` (11 selectors) and
`.event-line.tint-goal/yc/rc`. This plan's Target grew to cover that
block too, under the identical dead-consumer criteria; every other rule
in `idle-peek.css` stays byte-identical as well.

## Repo conventions to follow

- Match plan 170's own precedent for this exact kind of cleanup — it's
  named "Trim `live-scorecard.css`'s now-dead masthead-equivalent rules"
  in its own Steps section; this plan simply executes what that one
  deferred.
- Leave a brief comment where `.notif-block`'s old header comment used
  to be, only if useful context would otherwise be lost — check whether
  any surviving rule in the file still benefits from the "class names
  lifted verbatim from prototype/football-card.html" framing before
  deciding whether to keep, trim, or drop that sentence.

## Steps

1. In `src/overlay/live-scorecard.css`, delete the `.card-root
   .notif-block` rule block (currently lines 1-9, including its header
   comment).
2. In the same file, delete the `.card-root .event-line` rule block
   (currently lines 214-222).
2b. In `src/overlay/idle-peek.css`, delete the `.ev-ico*` (11 selectors)
   and `.event-line.tint-goal/yc/rc` blocks — the scope addendum above.
3. Do not touch any other rule in either file.
4. Search the repo once more for `notif-block`/`event-line` usage
   outside this file (`grep -rn "notif-block\|\.event-line" src/`) to
   confirm no other consumer was missed before finalizing — the search
   already run for this plan found none, but re-verify since drift is
   possible between when this plan was written and executed.

## Boundaries

- Do NOT touch any of the confirmed-live rules listed above.
- Do NOT touch `NotificationBody.tsx`, `StatusRailCard.tsx`, or any
  `.tsx` file — this is a CSS-only cleanup, the render path is already
  correct and unaffected.
- Do NOT touch `prototype/football-card.html` or other prototype files —
  out of scope for this plan.
- If `live-scorecard.css`'s current content doesn't match the quoted
  excerpts (drift since commit `0c08e93`), STOP and report instead of
  improvising — in particular, re-verify via the same JSX cross-check
  this plan used (grep every remaining class name against
  `NotificationBody.tsx`) before deleting anything, since a false
  positive here would delete a still-live rule.

## Verification

- **Mechanical**: `npx vitest run` (repo root) clean — in particular
  `NotificationBody.test.tsx`/`StatusRailCard.test.tsx` (both already
  have comments confirming `.notif-block` is gone, so no assertion
  should reference it). `npx biome ci .` N/A (CSS isn't biome-scoped in
  this repo). `npx tsc --noEmit` clean (no TS touched).
- **Feel check**: run the app, trigger a live football match card
  through several event types (goal, yellow/red card, break, final) and
  confirm the score-row/chips/crests/digits still render identically —
  this plan removes zero live styling, so there should be no visible
  change at all.
- **Done when**: all three dead rule blocks (two in `live-scorecard.css`,
  one in `idle-peek.css`) are removed, `npx vitest run` is clean, and a
  diff of both files shows nothing else changed.
