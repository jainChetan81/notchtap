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
> **NOT YET DISPATCHABLE**: filed while 091 was still executing. Before
> dispatch it REQUIRES a review-plan pass to (a) stamp the drift-check and
> Planned-at SHAs against post-091 master, (b) re-verify every "Current
> state" citation against 091's actual shipped class names and file
> layout, and (c) fold in anything 091's review flagged for item 19.
>
> **Drift check (run first, after stamping)**:
> `git diff --stat <stamp at dispatch>..HEAD -- src/styles.css src/settings/preview-overlay.css src/components/StatusRailCard.tsx src/components/Manifest.tsx src/lib/presentation.ts`
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
- **Planned at**: `<stamp at dispatch, post-091>`

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

## Current state (all subject to the 091 re-verify — these are pre-091 facts)

- `src/styles.css` — pre-091 content rules that this plan restyles:
  `.compact::before` accent edge (:356), `.stamp` (:405),
  `.compact-hint` (:415, `kbd` at :428), `.track` (:447-471), the
  manifest rules, news `.pill`s, `.league-chip`/`.live-pill`. **091
  moves these into `.below-block` unmodified; their line numbers and
  possibly class groupings WILL have changed — re-cite at the
  review-plan pass.**
- `src/components/StatusRailCard.tsx` — the compact/expanded branches
  and per-source content; `src/components/Manifest.tsx` — the expanded
  manifest; `src/lib/presentation.ts` — the stamp/signal tables (data
  stays, only presentation CSS changes).
- `prototype/notch-states.html` §4 — the locked reference: `.notif-*`
  rules (~:150-175), `.chip`/`.chip-source` (~:160), the phase-driven
  flank behavior, and the TTL bar placement.
- The TTL bar (`TtlBar.tsx`) and `Track` are wired and tested — reuse,
  don't rebuild; only their container styling moves.

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

## Done criteria (finalize at the review-plan pass)

- [ ] All gates exit 0; §0 matches live
- [ ] `grep -c "\.pill\b" src/styles.css` → 0 (chip language everywhere)
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
  citations assume and the review-plan pass didn't catch it — re-run the
  pass rather than adapting ad hoc.
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
