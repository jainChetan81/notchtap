# 169 — Render the Agent Board's primary session through the shared notification template

- **Status**: DONE (2026-08-01) — executed via `/improve execute`, reviewed, approved, and locally merged into `agent-card-ui-unification` (not pushed, not merged to master).
- **Commit**: 1ebd5af, merged via 8184090
- **Severity**: N/A (design direction, not a bug)
- **Category**: UX / architecture — reconsiders a v7 decision (`docs/ARCHITECTURE.md` territory; see Boundaries)
- **Estimated scope**: large — `NotificationBody.tsx`'s existing generic detail rendering, `manifest.css`, `AgentBoard.tsx` rewrite, `agent-board.css` trim, `AgentBoard.test.tsx` rewrite, plus the `.detail-label`/`.detail-value` assertions in `StatusRailCard.test.tsx`
- **Depends on**: plan 168 (the unified template leans on the TTL bar, which should render correctly first)
- **Executable spec**: `prototype/agent-board.html`'s "⚠ proposal — the same states, through the unified shell" section — every state/runtime combination in this plan's Target is built and screenshot-verified there already

## Problem

Today the Agent Board's resting view (`AgentBoard.tsx`) is its own bespoke
layout — `.agent-board-primary` / `.agent-board-primary-head` /
`.agent-board-state-pill` / `.agent-board-rows` — built from scratch,
separate from the generic `NotificationBody.tsx` template every other
origin (manual/CLI, weather, news) renders through:

```tsx
// src/components/AgentBoard.tsx:380-409 (current, resting hero only)
<AnimatePresence initial={false} mode="wait">
  <motion.div key={primary.id} className="agent-board-primary" ...>
    <div className={`agent-board-primary-head ${agentRuntimeClass(primary.runtime)}`}>
      <span key={primary.state} className={`agent-dot large ${primaryPresentation.pulse ? "pulse" : ""}`} aria-hidden="true" />
      <span className="agent-runtime-tick" aria-hidden="true" />
      <span className="agent-board-runtime">{agentRuntimeLabel(primary.runtime)}</span>
      <span className="agent-board-state-pill">{primaryPresentation.label}</span>
    </div>
    {primaryProjectName && <div className="agent-board-project">{primaryProjectName}</div>}
    {primary.summary && <div className="agent-board-summary">{primary.summary}</div>}
    <div className="agent-board-elapsed">{primaryElapsed}</div>
  </motion.div>
</AnimatePresence>
```

This is a real, deliberate divergence from the operator's own stated
preference (this session's whole prototyping pass): a masthead hairline +
stamp pill + accent stripe + title/body/facts reads as "the CMUX card" —
the one visual language the rest of the app commits to — and the Agent
Board currently opts out of it entirely.

**A second, more foundational gap**: the fact-pill treatment this plan
introduces (single colored pill per fact, `.detail-facts`/`.fact-pill`/
`.fp-label`/`.fp-tag`, verified in `prototype/agent-board.html`'s CSS) does
NOT exist anywhere in the shipped app today. What exists instead, and is
LIVE right now for manual/CLI and weather cards, is a stacked label/value
pair:

```tsx
// src/components/NotificationBody.tsx:144-151 (current, generic branch)
{liveVisibleDetails.slice(0, MAX_VISIBLE_DETAIL_PAIRS).map((detail) => (
  <div key={detail.label}>
    <div className="detail-label">{detail.label}</div>
    <div className="detail-value">{detail.value}</div>
  </div>
))}
```

```css
/* src/overlay/manifest.css:50-75 (current) */
.card-root .detail-label {
  margin-bottom: 5px;
  color: var(--accent);
  font: 700 8px/1 var(--font-mono);
  letter-spacing: 0.16em;
  text-transform: uppercase;
}
.card-root .detail-value {
  color: rgba(255, 255, 255, 0.74);
  font-size: 11px;
  line-height: 1.45;
  overflow-wrap: anywhere;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
```

If this plan only adds fact pills for Agent's own new facts and leaves
`NotificationBody.tsx`'s existing generic branch on the old stacked
style, the shared template ends up speaking two different visual
languages for the identical underlying concept (a label/value fact pair)
depending on which origin happens to render it — undermining the entire
"one shared template" premise this plan and 170 are built on. This plan
therefore OWNS converting the existing stacked style to the new pill
style, applied uniformly (manual/CLI and weather's current cards
included), not just adding pills for agent. Plan 170 (football) then
reuses the already-converted system rather than reintroducing it.

Note the existing `.detail-value.message` variant (`manifest.css:79-84`,
"the one manifest field that carries full prose") — check at
implementation time whether any live call site still uses it. Prose
doesn't fit a pill; if something real depends on it, keep that one path
as free-flowing text (not a pill) and say so in the component, rather
than forcing prose into a pill shape to chase uniformity for its own
sake.

## Target

Render the primary (highest-ranked) session through
`NotificationBody.tsx`'s existing template — masthead (dot + `agent`
kicker) + a per-state stamp + the priority accent stripe + title +
subtitle + body + fact pills for state-specific detail (tool/risk,
progress, session duration) — instead of the bespoke header block above.
The "other sessions" list (`.agent-board-rows`, currently
`AgentRow`/`ExpandedAgentRow`) does NOT fit inside a single generic card
and stays as an ADDITIVE flavor appended below the templated hero, in a
restrained mono/dot row language (this part is NOT a regression — it's
already how `prototype/agent-board.html`'s proposal section handles it,
and it's the one piece of Agent Board's information density the generic
template genuinely cannot express on its own).

Concretely, per state (see the prototype for the exact rendered result):

| state | stamp | wash | example fact pills |
|---|---|---|---|
| `waiting_for_permission` | Now | `src-<runtime>` | `Bash` `DESTRUCTIVE` (danger tone) |
| `waiting_for_input` | Now | `src-<runtime>` | — |
| `working` | Done | `src-<runtime>` | `63%` progress |
| `starting` | Done | `src-<runtime>` | `2s` session |
| `completed` | Live | `src-<runtime>` | `12m` duration |
| `failed` | Now | `src-<runtime>` | `1` exit code (danger tone) |
| `stale` | Live | `src-<runtime>` | `14m ago` last seen |

Fact pills: one pill per fact (not a stacked label+value pair), a small
muted mono label prefix only when the value alone would be ambiguous
(omit it entirely when the value is self-explanatory — e.g. a scorer/
assist name never needs a "Scorer:" prefix), an optional colored tag for
a qualifier that deserves a color (risk level → danger/coral). Pills must
never grow the card past its state's fixed width/height — `.compact`
needs `overflow: hidden` as a backstop, `.detail-facts` capped to the
compact's own content width, each pill truncating its own text via
`max-width` + ellipsis rather than wrapping or pushing wider. See
`prototype/agent-board.html`'s CSS for the exact rule set (already
verified empirically to hold at the real card's 400/500px widths with
zero overflow).

The Agent Board's OUTER shell is unaffected — it's already the same
`.card-assembly.expanded.agent-board-shell` every other card's shell
uses; this plan only touches what renders inside `.below-block`.

## Repo conventions to follow

- `NotificationBody.tsx`'s existing branch structure (news vs. everything
  else) is the pattern to extend, not replace — add an agent-aware path
  alongside the existing generic/news branches, following the same
  `GENERIC_MASTHEAD_KICKER`-style lookup convention `lib/presentation.ts`
  already uses elsewhere.
- Reuse `agentStatePresentationFor`/`agentRuntimeLabel`/`agentRuntimeClass`
  (`lib/presentation.ts`) unchanged — this plan changes how the state data
  is RENDERED, not the lookup tables that produce it.
- The priority accent (`--accent`/`--accent-soft`, `.card-assembly.low/
  .medium/.high`) needs a real mapping from agent state to priority tier —
  today Agent Board carries no notion of "priority" at all (it uses its
  own `--agent-accent` system instead). Decide this mapping as part of
  implementation (a reasonable default: waiting/failed → high,
  working/starting → medium, completed/stale → low) and document the
  choice in the component, since it's a new semantic, not a port of an
  existing one.

## Boundaries

- **This reopens a real v7 decision.** `docs/ARCHITECTURE.md`/the v7 spec
  established the Agent Board as its own dedicated resting-state component
  specifically so multi-session aggregation had room to exist — do not
  start this plan without the operator's explicit go-ahead beyond "make
  the plans" (i.e. confirm they want it EXECUTED, not just written up).
- Do NOT touch the hover-expanded view (`AgentBoard.tsx`'s
  `agent-board-expanded-list`/`ExpandedAgentRow`) — out of scope for this
  plan. It already renders full per-session detail in a scrollable list;
  unifying that too is a separate, later decision if this lands well.
- Do NOT touch `HERO_SWAP_TRANSITION`/`ROW_TRANSITION`/`DISCLOSURE_SPRING`
  or any of the bounded agent-dot pulse motion — this plan changes
  markup/CSS classes, not the animation contracts around them.
- Do NOT remove `agent-board.css`'s `.agent-board-primary-head` family
  outright in the same commit that adds the new template path — land the
  new rendering first, confirm it in a real build, THEN remove the dead
  CSS in a small follow-up so a revert is cheap if the operator changes
  their mind after seeing it live.

## Steps

1. Read `src/components/AgentBoard.test.tsx` in full first. There is no
   standalone `NotificationBody.test.tsx` — its generic-branch detail
   rendering is covered inside `src/components/StatusRailCard.test.tsx`
   (confirmed: `grep -rl "detail-label" src --include="*.test.tsx"` returns
   only that file). Read `StatusRailCard.test.tsx` in full too, specifically
   every assertion touching `detail-label`/`detail-value`. Know exactly what
   breaks — on BOTH files — before touching either component.
2. Convert `NotificationBody.tsx`'s existing generic-branch detail
   rendering (quoted in Problem above) from stacked `.detail-label`/
   `.detail-value` divs to the fact-pill markup (`.detail-facts` wrapper,
   one `.fact-pill` per fact, `.fp-label`/`.fp-tag` inner spans as shown
   in `prototype/agent-board.html`'s CSS). This lands BEFORE the
   agent-aware branch below, so agent's own facts have a system to plug
   into rather than inventing a parallel one.
3. In `src/overlay/manifest.css`, replace the `.detail-label`/
   `.detail-value` rules (lines 50-75) with the following — copied
   verbatim from `prototype/agent-board.html`'s `<style>` block (note:
   that prototype file is UNTRACKED as of this writing; do not rely on
   reading it from a worktree, it may not be there — the rules below are
   the full, already-empirically-verified-at-400/500px-with-zero-overflow
   source, inlined so this step never depends on that file's presence):

   ```css
   .card-root .detail-facts { display: flex; flex-direction: column; align-items: flex-end; gap: 3px; margin-top: 7px; margin-bottom: 14px; max-width: 100%; }
   .card-root .fact-pill { display: inline-flex; align-items: center; gap: 4px; max-width: 100%; padding: 2px 7px; border-radius: 999px; font-size: 10px; line-height: 1.4; color: rgba(255,255,255,0.82); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; border: 1px solid rgba(255,255,255,0.12); background: rgba(255,255,255,0.05); }
   .card-root .fact-pill .fp-label { flex: none; font: 700 7px/1 var(--font-mono); letter-spacing: 0.07em; text-transform: uppercase; opacity: 0.68; }
   .card-root .fact-pill .fp-tag { flex: none; font: 700 7px/1 var(--font-mono); letter-spacing: 0.07em; text-transform: uppercase; }
   .card-root .fact-pill.tone-accent { background: color-mix(in srgb, var(--accent) 14%, transparent); border-color: color-mix(in srgb, var(--accent) 38%, transparent); }
   .card-root .fact-pill.tone-accent .fp-tag { color: var(--accent); }
   .card-root .fact-pill.tone-danger { background: color-mix(in srgb, var(--overlay-coral) 14%, transparent); border-color: color-mix(in srgb, var(--overlay-coral) 42%, transparent); }
   .card-root .fact-pill.tone-danger .fp-tag { color: var(--overlay-coral); }
   .card-root .fact-pill.tone-safe { background: color-mix(in srgb, var(--overlay-green) 14%, transparent); border-color: color-mix(in srgb, var(--overlay-green) 42%, transparent); }
   .card-root .fact-pill.tone-safe .fp-tag { color: var(--overlay-green); }
   ```

   Also add `overflow: hidden;` to the existing `.card-root .compact` rule
   (`src/overlay/masthead-content.css` — search for `.card-root .compact {`)
   as the fixed-size backstop; do not otherwise change that rule.

   The expected markup shape per fact (mirror this exactly, adjust element
   types to match `NotificationBody.tsx`'s existing JSX conventions):
   ```html
   <div class="detail-facts">
     <span class="fact-pill tone-danger">
       <span class="fp-label">Tool</span>Bash<span class="fp-tag">destructive</span>
     </span>
   </div>
   ```
   `fp-label` is omitted entirely (not rendered as an empty span) when a
   fact's label would be redundant with its value (e.g. a scorer's name
   needs no "Scorer:" prefix) — this is a per-fact judgment call the
   calling code makes, not something the pill component decides on its own.
   `fp-tag` is omitted entirely when a fact has no qualifier worth
   coloring. `tone-*` on the outer `.fact-pill` defaults to no tone class
   (plain neutral pill) when neither accent/danger/safe applies.

   Resolve the `.detail-value.message` question from Problem above while
   in this file.
4. Add the agent-aware branch to `NotificationBody.tsx` (or a sibling
   component `AgentBoard.tsx` calls into directly — implementer's choice,
   whichever keeps `NotificationBody.tsx` from needing agent-specific
   imports it doesn't otherwise need), using the fact-pill system from
   steps 2-3.
5. Update `AgentBoard.tsx`'s primary-session JSX to call the new path
   instead of hand-rolling `.agent-board-primary-head`.
6. Add the priority-tier mapping decided in "Repo conventions" above.
7. Keep `.agent-board-rows`/`AgentRow` as the additive flavor below the
   templated hero — restyle only if the new hero's spacing needs it to
   match visually (mono row language, same as the prototype).
8. Update `AgentBoard.test.tsx` AND `StatusRailCard.test.tsx` (the
   `detail-label`/`detail-value` assertions found in step 1) for the new
   markup — this is real regression-test rewrite work on two files, not a
   quick assertion tweak; budget for it.
9. Trim `agent-board.css`'s now-dead `.agent-board-primary-head`/
   `.agent-board-runtime`/`.agent-board-state-pill` rules — as a SEPARATE
   follow-up commit per the Boundaries note above, not part of this one.

## Verification

- **Mechanical**: `npx vitest run` clean (including the rewritten
  `AgentBoard.test.tsx` and the `detail-label`/`detail-value` assertions
  in `StatusRailCard.test.tsx`). `npx tsc --noEmit` clean. `npx biome ci .`
  clean.
- **Manual/weather regression check**: trigger a manual/CLI notification
  and a weather alert that both carry `details` (e.g. via `./notchtap
  --title "t" --body "b"` with detail fields, or the Settings → Appearance
  → "Live check" test button) and confirm THEIR fact pills render
  correctly too — this is the step most likely to get silently skipped,
  since it's easy to verify only the new agent path and assume the
  existing origins still work.
- **Visual, real app**: trigger at least one session in each of the seven
  states (`./notchtap-agent` test-event tooling, or the Settings → Agents
  → "Send test event" button) and compare against
  `prototype/agent-board.html`'s proposal section for each — masthead,
  stamp, accent stripe, fact pills, and (where present) the queue-rows
  flavor should all match.
- **Overflow check**: with the longest realistic values (a long tool name,
  a long risk word, a long project name), confirm in real DevTools that
  the card's rendered width never exceeds its state's formula (400px
  showing / 500px expanded, scale 1) and `.compact`'s rendered height
  never grows unbounded — pills must truncate, not push.
- **Done when**: all seven states render through the shared template in a
  real build, `AgentBoard.test.tsx` is green against the new markup, and
  a side-by-side against the prototype shows no visual drift beyond
  intentional differences (live data vs. the mock's fixture data).
