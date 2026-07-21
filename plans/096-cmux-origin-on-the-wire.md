# Plan 096: Cmux origin on the wire + the cmux card accent (079 item 8, the half plan 092 could not build)

> **Executor instructions**: **Step 0 is an operator decision — do NOT
> proceed past it on your own judgment.** If the operator chooses to drop
> the feature, this plan is retired, not executed. Otherwise follow the
> steps, run every verification command, and stop on any STOP condition.
> When done, update this plan's row in `plans/README.md` — unless a
> reviewer dispatched you and told you they maintain the index.
>
> **Drift check (run first)**:
> `git diff --stat 18a3e8d..HEAD -- src-tauri/src/event.rs src-tauri/src/queue.rs src-tauri/src/http.rs src/useSlotState.ts src/components/StatusRailCard.tsx src/styles.css`
> Expected: empty. On a mismatch with "Current state", STOP.

## Status

- **Priority**: P3 — cosmetic; the only unshipped piece of 079 item 8.
- **Effort**: S–M (a small wire addition + a small frontend accent)
- **Risk**: MED — touches `SlotState`, which is governed by the
  `dedup_eq` rule; a careless addition there causes emission storms.
- **Depends on**: 092 (merged). Independent of 093.
- **Category**: direction
- **Planned at**: commit `18a3e8d`, 2026-07-21

## Why this exists

The operator chose a **cmux-specific card accent** (079 item 8) over the
advisor's "general template only" recommendation. Plan 092 was written
frontend-only and specced that accent — but the executor traced the wire
and **correctly stopped**: there is no frontend-safe way to know a card
came from cmux. The reviewer independently confirmed every claim:

- `SlotState::Showing` (`src-tauri/src/event.rs`, the `Showing` variant)
  carries 18 fields — `id`, `title`, `body`, `event_type`, `priority`,
  `signal`, `expanded`, `source`, `category`, `published_at_ms`, `link`,
  `subtitle`, `details`, `espn`, `queue_total`, `queue_done`, `ttl_ms`,
  `remaining_ms` — and **`origin` is not among them**.
- `queue.rs::current_slot_state` (the sole constructor of
  `SlotState::Showing`) never reads `item.event.origin` (grep: 0 hits).
- `Event.origin: SourceKind` is therefore rust-internal only.
- `meta.source` cannot substitute: `http.rs` documents
  "source/category/published/link stay poller-only" and builds generic
  events as `EventMeta { subtitle, details, ..EventMeta::default() }`,
  so a cmux push and a plain CLI push are byte-identical on the wire.

The executor also explicitly refused a proxy heuristic (keying off
`eventType === "generic"`), because that matches manual/CLI pushes too
and would mislabel them as agent-originated — worse than not shipping.
That judgment was correct.

So the accent needs `origin` on the wire. That is a real (if small)
architectural addition, and it is the operator's call whether a cosmetic
accent justifies it.

## Step 0: OPERATOR DECISION — do not skip, do not guess

**Should `origin` be added to the `SlotState::Showing` wire payload so
the frontend can style cards by their source?**

- **(a) Yes — add it.** Cost: one new wire field, its `dedup_eq`
  treatment, TS validator updates, and the tests that pin the payload
  shape. Benefit: unlocks the decided cmux accent AND any future
  per-origin presentation (the frontend currently cannot distinguish
  sources at all for generic cards). Arguably `origin` is legitimate
  presentation input, not just accent plumbing.
- **(b) No — drop the cmux accent.** Item 8 reverts to the advisor's
  original recommendation: cmux renders through the general template like
  every other generic card. Zero code. Record it in `plans/README.md`'s
  "Findings considered and rejected" and retire this plan.

*Advisor's recommendation: **(a)**, narrowly.* Not for the accent itself
— that remains marginal — but because "the overlay cannot tell which
source produced a generic card" is a real expressiveness gap that will
resurface (any future per-source treatment hits the same wall), and the
addition is small and well-understood. If the operator disagrees, (b) is
entirely reasonable and costs nothing.

**STOP** and report if the answer is anything other than (a) or (b).

**Verify**: the decision is written into this file under a "Decision"
heading before any file is edited.

## Current state

- `src-tauri/src/event.rs` — the `SlotState` enum with
  `#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]`;
  the `Showing` variant's 18 fields listed above; `SlotState::dedup_eq`
  (added by plan 081) which normalizes continuously-varying fields
  before comparison.
- `src-tauri/src/queue.rs::current_slot_state` — builds
  `SlotState::Showing` from the visible `QueuedItem`; `item.event.origin`
  is in scope there and simply not read today.
- `src-tauri/src/event.rs` — `SourceKind` is a closed serde enum
  (`Football`/`News`/`Manual`/`Cmux`/`Weather`, snake_case on the wire,
  unknown values rejected at deserialization).
- `src/useSlotState.ts` — the TS validator for the `slot-state` payload
  (camelCase wire; reject-on-malformed, same rigor as the rust side).
- `src/components/StatusRailCard.tsx` — the compact/expanded branches
  now rendering plan 092's `.notif-*`/`.chip` language inside
  `.below-block`.

## Commands you will need

| Purpose | Command | Expected |
|---|---|---|
| Rust tests | `cd src-tauri && cargo test --locked` | all pass (baseline 441 + 3 doc-tests) |
| Rust lint/fmt | `cargo clippy --locked --all-targets -- -D warnings` / `cargo fmt --check` | exit 0 |
| Frontend | `npx vitest run` / `npx tsc --noEmit` / `npx biome ci .` / `npx vite build` | all pass (baseline 192) |

## Scope (if Step 0 answers (a))

**In scope**: `src-tauri/src/event.rs` (the `origin` field + its
`dedup_eq` treatment), `src-tauri/src/queue.rs` (`current_slot_state`
populates it), `src/useSlotState.ts` (validator + type),
`src/components/StatusRailCard.tsx` (the accent), `src/styles.css` +
`src/settings/preview-overlay.css` (mirror), the affected test files,
`docs/TESTING_STRATEGY.md` §0.

**Out of scope**: `Event`'s own shape (origin already exists there);
queue/rotation/priority semantics; the priority accent channel — the
cmux accent must never touch it (priority encodes tier, never origin);
plan 091's shell geometry and rounding law; anything under
`capabilities/`.

## Steps (if (a))

1. **Wire**: add `origin: SourceKind` to `SlotState::Showing`; populate
   it in `current_slot_state`. **`dedup_eq` treatment: `origin` is
   time-invariant, so it stays IN the comparison** (unlike
   `remaining_ms`, which plan 081 had to exclude). Do NOT touch the
   derived `PartialEq`.
   **Verify**: `cargo test --locked` passes; add a serialization test
   pinning `"origin":"cmux"` on the wire.
2. **TS**: extend the `useSlotState` validator and type to accept
   `origin` as the five-value union, rejecting unknown values as the
   validator does for other closed enums.
   **Verify**: `npx tsc --noEmit`; a validator test for accept + reject.
3. **Accent** (Decision 3's minimal spec): cmux cards get the
   cmux-yellow chip tint plus a small agent glyph inside the source chip
   (CSS shape or a unicode mark — no new asset, no dependency), and a
   1px cmux-tinted hairline on `.below-block`. **It must not touch the
   priority accent edge.** If the hairline collides visually with the
   priority edge, drop the hairline and keep the chip treatment; say so
   in the report. Mirror all CSS into `preview-overlay.css` same commit.
   **Verify**: a test asserting the accent renders for `origin: "cmux"`
   and is byte-absent for every other origin.
4. **Gates + §0**.

## Done criteria (if (a))

- [ ] All seven gates exit 0; §0 matches live counts
- [ ] `"origin"` appears in a pinned serialization test
- [ ] `dedup_eq` still excludes only the continuously-varying fields —
      one `accept()` still emits exactly one `slot-state` (plan 081's
      regression test stays green)
- [ ] The accent renders only for cmux; a test pins its absence for the
      other four origins
- [ ] Priority accent edge byte-identical for every origin
- [ ] `git diff -- src-tauri/capabilities/` empty

## STOP conditions

- Step 0 unanswered.
- Adding `origin` to `dedup_eq`'s comparison changes emission volume
  (plan 081's single-emit regression test fails) — that would mean the
  field is not actually time-invariant; report rather than excluding it.
- The accent cannot be made visible without touching the priority accent
  channel.
- Any change to plan 091's shell geometry or rounding law.

## Maintenance notes

- If (b) is chosen: retire this file, record item 8 in the rejected
  findings, and note in `plans/README.md` that the frontend deliberately
  cannot distinguish generic-card sources.
- If (a) ships, `origin` becomes available for any future per-source
  presentation — note that in the field's doc comment so the next person
  doesn't re-derive this analysis.
- The `dedup_eq` rule (CLAUDE.md) is the sharp edge here: any wire field
  that varies continuously must extend `dedup_eq`, never rely on derived
  `PartialEq`. `origin` is constant per item, so it is safe in the
  comparison — but state that reasoning in the code.
