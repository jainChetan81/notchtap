# Plan 092: The general card + expanded manifest in the cutout language (079 item 19, + items 8/10/11/14)

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **HARD PREREQUISITE — plan 091 must be DONE and merged.** This plan
> restyles content INSIDE 091's `.below-block`; if
> `grep -c "below-block" src/styles.css` is 0, 091 has not landed — STOP.
>
> **DISPATCHABLE** — the required review-plan pass ran 2026-07-21 against
> shipped 091 (see the note at the end of this file); SHAs stamped and all
> citations re-verified against what 091 actually built.
>
> **Drift check (run first)**:
> `git diff --stat 0ea2a96..HEAD -- src/styles.css src/settings/preview-overlay.css src/components/StatusRailCard.tsx src/components/Manifest.tsx src/lib/presentation.ts src/components/StatusDots.tsx`
> Expected: empty. On a mismatch with "Current state", STOP.

## Status

- **Priority**: P2
- **Effort**: M-L — content restyle of the most-used card path, plus the
  chip-language unification across all card types.
- **Risk**: MED — every non-live notification renders through this path.
  Mitigation: the wire contract and component props don't change; this is
  presentation-layer only.
- **Depends on**: **091 (hard)**. 080 (news card, DONE) — its `.pub-meta`
  and manifest-block content carries into the new language per Step 4.
- **Category**: direction
- **Planned at**: commit `0ea2a96`, 2026-07-21 (stamped post-091 merge;
  every citation below re-read at this SHA)
- **Baselines at `0ea2a96`, verified live**: 441 rust + 3 doc-tests,
  189 frontend. Re-derive rather than trusting these.

## Decisions of record (operator, 2026-07-21 — do not re-litigate)

1. **Item 19 (this plan's core)**: the general compact card + expanded
   manifest adopt the prototype §4 language — title+chip header row,
   subtitle+inline-time row, full-width body (2-line clamp compact),
   `.notif-track` queue slider, TTL bar at the block's bottom edge.
   Reference: `prototype/notch-states.html` §4 (`.notif-*` rules) and its
   interactive flow — this was mocked as the cmux worked example and
   operator-approved as "the standard template for every other card type."
2. **Item 10 — one chip language everywhere**: the prototype's `.chip`
   system (rounded-full, mono uppercase 8-8.5px, tinted background +
   1px tinted border) REPLACES today's per-surface pills during this
   build: news `.pill`s, the live `.league-chip`/`.live-pill`, and the
   source chips all converge on `.chip` + modifier classes. Existing
   *semantics* (what each pill says) are unchanged — only the shared
   visual vocabulary.
3. **Item 8 — cmux gets a specific accent** (operator chose this OVER the
   advisor's general-template-only recommendation; build it minimal and
   non-colliding): cards with `origin === "cmux"` get (a) the cmux-yellow
   source chip tint (already exists), (b) a small agent glyph prefix
   INSIDE the source chip (a CSS shape or unicode ⌁-class mark — no new
   asset, no new dependency), and (c) a 1px cmux-tinted top hairline on
   `.below-block`. It must NOT touch the priority accent channel — the
   accent edge still encodes priority, never origin. If (c) visually
   collides with the priority edge in practice, drop (c) and keep (a)+(b);
   note the drop in your report.
4. **Item 11 — pause indicator via the idle dots**: when the engine is
   paused, all three status dots render the `dim` treatment plus one
   small static pause glyph (two 2px bars, CSS-drawn) beside the dot
   row. Receive-only law: an indicator, never a button. The paused flag
   already crosses the wire (`useStatusState` — verify the exact field
   at dispatch). The old "News paused" idle text remnant is retired by
   091; this replaces it globally.
5. **Item 14 — staleness carries as-is**: plan 032's `ageLabel`
   ("2m ago" + dimming thresholds) moves into the new header's
   inline-time position unchanged — same computation, same thresholds,
   new location. Zero redesign; item 14 closes with this plan.

## Current state — re-verified against shipped 091 at `0ea2a96`

**The shell 091 built (consume it; do NOT modify it):**

- `.card-assembly` (`styles.css:56`) — the outer shell, a 3-column ×
  2-row CSS grid (`1fr | var(--notchtap-cutout-width) | 1fr`). It owns
  **geometry + shell-level state only**: `.idle` (:74), `.expanded`
  (:78), the priority accents `.low`/`.medium`/`.high` (:180/:185/:190),
  `.hovered` (:204), and the celebration classes `.pulse-goal` (:253)
  / `.pulse-red` (:379) with their `::after`/`::before` rings.
- `.flank-left` / `.flank-right` (:82-83, :99, :105) — the cutout-height
  row. Left renders `<FlankClock />`, right renders `<StatusDots>` **in
  idle only** (`StatusRailCard.tsx:309-330`).
- `.below-block` (:157) — `grid-column: 1 / -1; grid-row: 2`, opaque
  `#000`, `overflow: hidden`, bottom-corner rounding via
  `var(--card-radius, 14px)`. **All card content lives here** — this is
  the element your restyle targets.
- Rounding law: `.card-assembly:not(:has(.below-block)) .flank-*`
  (:131/:135) rounds the flanks only when no below-block exists. Do not
  add radius anywhere else; do not key rounding off `.idle`.
- Mood/shade classes (`news-shade`, `cat-*`, `wx-*`) sit on
  **`.below-block`**, not the shell — 091 moved them there because a
  `.card-assembly::before` paints behind below-block's opaque fill.
  Keep them there.

**The content rules this plan restyles** (all now inside `.below-block`,
moved unmodified by 091 — line numbers verified at `0ea2a96`):

- `.compact` (`styles.css:453`), `.compact::before` accent edge (:471)
- `.stamp` (:520)
- `.compact-hint` (:530), `.compact-hint kbd` (:543)
- `.track` (:562), `.track span` (:574), `.done` (:582), `.cur` (:586)
- `.manifest-wrap` (:625), `.manifest-wrap.expanded` (:633) and the
  manifest rules below them
- **The pills to converge (Decision 2)**: `.pills` (:821), `.pill`
  (:822), `.pill.category` (:827), `.pill.age` (:828), plus the
  grouped rules at :843-847; and the football chips
  `.league-chip` / `.live-pill` (rendered at
  `StatusRailCard.tsx:359-361`; CSS near :767).
- `ageLabel` (Decision 5) is `src/lib/presentation.ts:253`, called at
  `StatusRailCard.tsx:225` (`renderedNewsAge`). **Data unchanged** —
  only where it renders moves.
- The paused flag for Decision 4 is `status.paused`
  (`src/useStatusState.ts:23`, a plain boolean) — already validated and
  reaching the component.

**Reference and reuse:**
- `prototype/notch-states.html` §4 — the locked `.notif-*` / `.chip`
  language.
- `TtlBar.tsx`, `Track`, `Manifest.tsx`, `weatherArt.ts`,
  `presentation.ts` tables — reuse, never rebuild; only container
  styling and classNames move.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Frontend tests | `npx vitest run` | all pass (baseline: re-derive at dispatch) |
| Typecheck / lint / build | `npx tsc --noEmit` / `npx biome ci .` / `npx vite build` | exit 0 |
| Rust (no changes expected) | `cd src-tauri && cargo test --locked` | unchanged from baseline |

## Scope

**In scope**: `src/styles.css`, `src/settings/preview-overlay.css`
(mirror, same commit), `src/components/StatusRailCard.tsx` (content
markup within `.below-block` + the dots-pause treatment),
`src/components/Manifest.tsx` (markup/classes only),
`src/components/IdleView.tsx`-successor (only the pause-glyph addition),
their test files, `docs/TESTING_STRATEGY.md` §0.

**Out of scope**: the shape machinery (091's blocks — consume, don't
touch); `src/lib/presentation.ts`'s *data* (tables stay; only classNames
they emit may be renamed with their consumers); all rust; all hover
behavior (093); `TtlBar.tsx`/`useDelayedSwap.ts` internals; wire types.

## Steps (headline level — the review-plan pass expands against shipped 091)

1. Port the §4 `.notif-*` + `.chip` CSS into `styles.css` under the
   `.below-block` context; mirror to preview-overlay.css.
2. Restructure the compact branch: header row (title + chip-row),
   subtitle row (subtitle + `ageLabel` inline time), clamped body,
   `.notif-track`, TTL bar at bottom edge.
3. Expanded/manifest: full-width body + the 080 manifest-block content
   re-skinned into the same vocabulary (its full-width/meta/footer
   structure survives — it was designed post-redesign).
4. Converge news/live pills onto `.chip` modifiers; delete the orphaned
   pill rules (grep-verified).
5. Cmux accent (Decision 3) + pause indicator (Decision 4).
6. Tests: update selectors/classes; content SEMANTICS assertions stay
   (title/body/meta text, TTL wiring, track counts); add: cmux-accent
   present only for cmux origin; pause renders dim-dots+glyph; ageLabel
   in the new position with old thresholds.
7. Full gates + §0.

## Done criteria

- [ ] All seven gates exit 0 (`cargo test --locked`, clippy, fmt,
      `npx vitest run`, `npx tsc --noEmit`, `npx biome ci .`,
      `npx vite build`); `docs/TESTING_STRATEGY.md` §0 matches live
- [ ] `grep -c "\.pill\b" src/styles.css` → 0 (chip language everywhere)
- [ ] `git diff --stat -- src-tauri/` is **empty** — this plan is
      frontend-only; any rust diff is a scope violation
- [ ] `.card-assembly`'s own rules are untouched apart from any class
      you ADD: `git diff src/styles.css` shows no edit to the geometry
      block (`:56-160`), the rounding law (`:131-135`), or the priority
      accents (`:180-190`)
- [ ] Mood/shade classes still resolve on `.below-block`, not the shell
- [ ] `.stamp`/`.compact-hint`/`.track` content survives in the new
      vocabulary (assertions updated, semantics identical)
- [ ] Cmux accent renders ONLY for `origin === "cmux"`; priority edge
      unchanged for all origins
- [ ] Paused state: dim dots + glyph, pinned by a test
- [ ] Mirror-law grep ≥1 per new class family in both CSS files
- [ ] No rust diff; no wire-type diff

## STOP conditions

- 091 not merged (the `below-block` grep gate).
- 091 shipped class names/structure differing from what this plan's
  citations assume — re-run the review pass rather than adapting ad hoc.
- **You need to change `.card-assembly`'s geometry, its rounding law, or
  the `:has()` selector at `styles.css:131-135`.** Those encode 091's
  two hardest-won fixes (zero-height clipping; the exit-window
  double-curve race). Restyling content must never require touching
  them — if it seems to, STOP and report.
- A change would move mood/shade classes back onto the outer shell
  (that makes the gradient invisible — 091 proved it).
- The cmux hairline collides with the priority edge AND dropping it
  (per Decision 3) still leaves ambiguity about what to build.
- Any change would alter `presentation.ts` table *data* or a wire type.

## Maintenance notes

- This closes 079 items 8/10/11/14/19. After it: only 12/13 (app icon)
  and 16 (MediaRemote spike) remain in the ledger, plus 093's hover
  consumers.
- The chip system becomes the single pill vocabulary — future card types
  extend `.chip` modifiers, never invent new pill CSS.
- 093 (hover consumers) layers hover reveals onto surfaces this plan
  styles — land 092 before 093 to avoid styling the same regions twice.

**Review-plan pass (2026-07-21, at `0ea2a96`, after 091 merged)**: the
filing-time note requiring this pass is discharged. Every "Current
state" citation was re-read against shipped 091, not against the
pre-091 guesses this plan was filed with — the whole section was
rewritten. What changed materially: the shell is a **CSS grid**
(`.card-assembly`, 3×2), not the prototype's absolutely-positioned
blocks, so content sits in `.below-block` at `grid-row: 2`; mood/shade
classes live on `.below-block` (091 moved them for stacking-order
reasons — moving them back makes gradients invisible, now a STOP
condition); rounding is keyed off `:not(:has(.below-block))` rather than
`.idle`, and that selector plus the geometry block are now explicitly
STOP-protected since they encode 091's two hardest fixes. All content
rule line numbers re-verified live (`.compact` :453, `.stamp` :520,
`.compact-hint` :530, `.track` :562, `.manifest-wrap` :625, the `.pill`
family :821-847). Decision 4's paused flag confirmed as
`status.paused` (`useStatusState.ts:23`); Decision 5's `ageLabel`
confirmed at `presentation.ts:253` / `StatusRailCard.tsx:225`. Done
criteria gained a rust-diff-empty gate and a shell-untouched gate.
Baselines recorded (441+3 / 189).
